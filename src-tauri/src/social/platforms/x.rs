//! X (Twitter) gerçek entegrasyonu.
//!
//! Bu modül, X API v2 yayınlama akışını uygular:
//! - OAuth 1.0a üçlü bacaklı yetkilendirme (consumer key/secret + loopback
//!   callback `127.0.0.1` üzerinde).
//! - v1.1 `media/upload.json` ile video/görsel yükleme (doğrudan multipart veya
//!   parçalı INIT + APPEND + FINALIZE).
//! - v2 `/2/tweets` ile yayın (tweet.create).
//!
//! Hiçbir adımda sahte/yer tutucu yayın üretilmez; gerçek API isteği yapılır ve
//! gerçek yayın id / media id döndürülür.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha1::Sha1;
use tauri::Manager;

use super::super::credential_store;
use super::super::media_validation;
use super::super::metadata_store;
use super::super::models::{
    ConnectionRecord, ConnectionStatus, SocialAccountConnection, SocialError, TokenType,
};

/// X platform kimliği (katalogdaki değer).
pub const PLATFORM_ID: &str = "x";

const REQUEST_TOKEN_ENDPOINT: &str = "https://api.twitter.com/oauth/request_token";
const AUTHORIZE_ENDPOINT: &str = "https://api.twitter.com/oauth/authorize";
const ACCESS_TOKEN_ENDPOINT: &str = "https://api.twitter.com/oauth/access_token";
const TWEETS_ENDPOINT: &str = "https://api.x.com/2/tweets";
const MEDIA_UPLOAD_ENDPOINT: &str = "https://upload.twitter.com/1.1/media/upload.json";

/// Doğrudan (multipart) gönderim için üst sınır: üzerindeki her boyut parçalı yüklenir.
const DIRECT_MEDIA_LIMIT: usize = 4 * 1024 * 1024;
const CHUNK_SIZE: usize = 1 * 1024 * 1024;
const OAUTH_TIMEOUT_SECS: u64 = 300;

type HmacSha1 = Hmac<Sha1>;

/// Tek parça gönderimde doğrudan multipart iletilebilir dosya boyutu üst sınırı.
fn generate_nonce() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| {
            let b = rng.gen::<u8>();
            let chars =
                b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
            chars[b as usize % chars.len()] as char
        })
        .collect()
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

/// RFC 3986 percent-encode (X OAuth 1.0a signature kuralına uygun).
fn pct(input: &str) -> String {
    let mut out = String::new();
    for b in input.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Sıralı parametre dizesi (signature base string için).
fn collect_params_encoded(map: &BTreeMap<String, String>) -> String {
    map.iter()
        .map(|(k, v)| format!("{}={}", pct(k), pct(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// HMAC-SHA1 imza (OAuth 1.0a).
fn oauth_signature(method: &str, base_url: &str, params: &BTreeMap<String, String>, key: &str) -> String {
    let param_str = collect_params_encoded(params);
    let signature_base = format!("{}&{}&{}", method, pct(base_url), pct(&param_str));
    let mut mac = HmacSha1::new_from_slice(key.as_bytes()).expect("hmac key");
    mac.update(signature_base.as_bytes());
    BASE64.encode(mac.finalize().into_bytes())
}

/// İstemci/hesap kimlikleriyle ortak OAuth parametrelerini üretir.
fn oauth_params(consumer_key: &str, token: &str) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("oauth_consumer_key".to_string(), consumer_key.to_string());
    m.insert("oauth_nonce".to_string(), generate_nonce());
    m.insert("oauth_signature_method".to_string(), "HMAC-SHA1".to_string());
    m.insert("oauth_timestamp".to_string(), timestamp());
    m.insert("oauth_token".to_string(), token.to_string());
    m.insert("oauth_version".to_string(), "1.0".to_string());
    m
}

// ---------------------------------------------------------------------------
// Veri dizini ve güvenli config (Consumer Key / Secret) depolama
// ---------------------------------------------------------------------------

fn data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, SocialError> {
    app.path()
        .app_data_dir()
        .map_err(|_| SocialError::ConnectionStoreError)
}

const X_CONFIG_CONN: &str = "_x_app_config";

/// X Consumer Key, derleme zamanında (varsa) güvenli biçimde gömülür.
/// Değer tanımlı değilse `None` döner; derleme bu yüzden başarısız olmaz.
fn x_consumer_key_compiled() -> Option<&'static str> {
    option_env!("ES_OPS_X_CONSUMER_KEY")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// X Consumer Secret, derleme zamanında (varsa) güvenli biçimde gömülür.
/// Değer tanımlı değilse `None` döner; derleme bu yüzden başarısız olmaz.
fn x_consumer_secret_compiled() -> Option<&'static str> {
    option_env!("ES_OPS_X_CONSUMER_SECRET")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// Consumer Key / Secret yapılandırılmış mı? (ham değer dönmez)
/// Build-time gömülü veya güvenli depodaki kayıt kullanılır.
pub fn config_status() -> Result<(bool, bool), SocialError> {
    let has_key = credential_store::token_exists(PLATFORM_ID, X_CONFIG_CONN, TokenType::RefreshToken)
        .unwrap_or(false)
        || x_consumer_key_compiled().is_some();
    let has_secret = credential_store::token_exists(PLATFORM_ID, X_CONFIG_CONN, TokenType::AccessToken)
        .unwrap_or(false)
        || x_consumer_secret_compiled().is_some();
    Ok((has_key, has_secret))
}

pub fn store_consumer_key(value: &str) -> Result<(), SocialError> {
    if value.trim().is_empty() {
        return Err(SocialError::XNotConfigured);
    }
    credential_store::store_token(
        PLATFORM_ID,
        X_CONFIG_CONN,
        TokenType::RefreshToken,
        value.trim(),
    )
}

pub fn store_consumer_secret(value: &str) -> Result<(), SocialError> {
    if value.trim().is_empty() {
        return Err(SocialError::XNotConfigured);
    }
    credential_store::store_token(PLATFORM_ID, X_CONFIG_CONN, TokenType::AccessToken, value.trim())
}

fn read_consumer_key() -> Result<Option<String>, SocialError> {
    credential_store::get_token(PLATFORM_ID, X_CONFIG_CONN, TokenType::RefreshToken)
}

fn read_consumer_secret() -> Result<Option<String>, SocialError> {
    credential_store::get_token(PLATFORM_ID, X_CONFIG_CONN, TokenType::AccessToken)
}

fn resolved_consumer_key() -> Option<String> {
    x_consumer_key_compiled()
        .map(str::to_string)
        .or_else(|| read_consumer_key().ok().flatten())
}

fn resolved_consumer_secret() -> Result<String, SocialError> {
    x_consumer_secret_compiled()
        .map(str::to_string)
        .map(Ok)
        .unwrap_or_else(|| read_consumer_secret()?.ok_or(SocialError::XNotConfigured))
}

pub fn clear_config() -> Result<(), SocialError> {
    credential_store::delete_all_tokens(PLATFORM_ID, X_CONFIG_CONN)
}

pub fn load_consumer_key() -> Result<String, SocialError> {
    resolved_consumer_key().ok_or(SocialError::XNotConfigured)
}

pub fn load_consumer_secret() -> Result<String, SocialError> {
    resolved_consumer_secret()
}

// ---------------------------------------------------------------------------
// HTTP istemci
// ---------------------------------------------------------------------------

fn http_client() -> Result<reqwest::blocking::Client, SocialError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| SocialError::OperationFailed)
}

// ---------------------------------------------------------------------------
// Loopback callback + OAuth 1.0a üçlü bacaklı akış
// ---------------------------------------------------------------------------

fn generate_connection_id() -> Result<String, SocialError> {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    let mut b = [0u8; 8];
    rng.fill_bytes(&mut b);
    Ok(format!("x-{}{}", BASE64.encode(b), std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)))
}

fn bind_loopback() -> Result<TcpListener, SocialError> {
    TcpListener::bind(("127.0.0.1", 8080)).map_err(|_| SocialError::OperationFailed)
}

fn extract_param(query: &str, key: &str) -> String {
    let q = query.split('?').nth(1).unwrap_or("");
    for pair in q.split('&') {
        let mut it = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (it.next(), it.next()) {
            if k == key {
                return pct_dec(v);
            }
        }
    }
    String::new()
}

fn pct_dec(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Loopback'ta OAuth 1.0a callback'ini bekler; `oauth_verifier` döndürür.
fn wait_for_callback(listener: &TcpListener) -> Result<String, SocialError> {
    listener.set_nonblocking(true).ok();
    let deadline = std::time::Instant::now() + Duration::from_secs(OAUTH_TIMEOUT_SECS);
    loop {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            if let Ok(n) = stream.read(&mut buf) {
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/");
                let verifier = extract_param(path, "oauth_verifier");
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nConnection: close\r\n\r\n<html><body><h2>X baglantisi tamamlandi</h2><p>Bu pencereyi kapatabilirsiniz.</p><script>window.close();</script></body></html>");
                if !verifier.is_empty() {
                    return Ok(verifier);
                }
            }
        }
        if std::time::Instant::now() > deadline {
            return Err(SocialError::OauthTimeout);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn open_browser(app: &tauri::AppHandle, url: &str) -> Result<(), SocialError> {
    use tauri_plugin_shell::ShellExt;
    app.shell()
        .open(url, None)
        .map_err(|_| SocialError::OperationFailed)
}

/// query response (application/x-www-form-urlencoded) içinden anahtar/değer ayıklar.
fn parse_form(body: &str, key: &str) -> Option<String> {
    for pair in body.split('&') {
        let mut it = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (it.next(), it.next()) {
            if k == key {
                return Some(pct_dec(v));
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// OAuth 1.0a istek tokeni ve access token
// ---------------------------------------------------------------------------

fn request_token(listener: &TcpListener) -> Result<(String, String), SocialError> {
    let xlog = |msg: &str| {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(
            std::env::temp_dir().join("esops_x_debug.log"),
        ) {
            let _ = writeln!(f, "[REQ] {}", msg);
        }
    };

    let consumer_key = load_consumer_key()?;
    let consumer_secret = load_consumer_secret()?;
    let _port = listener.local_addr().map_err(|_| SocialError::OperationFailed)?.port();
    let callback = "http://127.0.0.1:8080/callback".to_string();

    let mut params = BTreeMap::new();
    params.insert("oauth_callback".to_string(), callback.clone());
    params.insert("oauth_consumer_key".to_string(), consumer_key.clone());
    params.insert("oauth_nonce".to_string(), generate_nonce());
    params.insert("oauth_signature_method".to_string(), "HMAC-SHA1".to_string());
    params.insert("oauth_timestamp".to_string(), timestamp());
    params.insert("oauth_version".to_string(), "1.0".to_string());

    let signing_key = format!("{}&", pct(&consumer_secret));
    let signature = oauth_signature("POST", REQUEST_TOKEN_ENDPOINT, &params, &signing_key);

    let mut header = params.clone();
    header.insert("oauth_signature".to_string(), signature);
    let mut parts: Vec<String> = header
        .iter()
        .map(|(k, v)| format!("{}={}", k, format!("\"{}\"", v)))
        .collect();
    parts.sort();
    let auth_header = format!("OAuth {}", parts.join(", "));

    xlog(&format!("POST {}", REQUEST_TOKEN_ENDPOINT));
    xlog(&format!("consumer_key baslangici: {}", &consumer_key[..consumer_key.len().min(8)]));
    xlog(&format!("callback: {}", callback));

    let client = http_client()?;
    xlog(&format!("full_auth_header: {}", auth_header));
    xlog(&format!("signing_key_baslangici: {}&", &consumer_secret[..consumer_secret.len().min(8)]));
    let resp = client
        .post(REQUEST_TOKEN_ENDPOINT)
        .header(reqwest::header::AUTHORIZATION, auth_header.clone())
        .send()
        .map_err(|e| {
            xlog(&format!("HTTP HATASI: {:?}", e));
            SocialError::OperationFailed
        })?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    xlog(&format!("status: {}", status));
    xlog(&format!("body: {}", &body[..body.len().min(500)]));
    if !status.is_success() {
        xlog(&format!("BASARISIZ: status={}", status));
        return Err(SocialError::OperationFailed);
    }
    let token = parse_form(&body, "oauth_token").ok_or(SocialError::OperationFailed)?;
    let secret = parse_form(&body, "oauth_token_secret").ok_or(SocialError::OperationFailed)?;
    xlog("request_token basarili");
    Ok((token, secret))
}

fn exchange_access_token(
    consumer_key: &str,
    consumer_secret: &str,
    request_token: &str,
    verifier: &str,
) -> Result<(String, String, String), SocialError> {
    let mut params = BTreeMap::new();
    params.insert("oauth_consumer_key".to_string(), consumer_key.to_string());
    params.insert("oauth_nonce".to_string(), generate_nonce());
    params.insert("oauth_signature_method".to_string(), "HMAC-SHA1".to_string());
    params.insert("oauth_timestamp".to_string(), timestamp());
    params.insert("oauth_token".to_string(), request_token.to_string());
    params.insert("oauth_verifier".to_string(), verifier.to_string());
    params.insert("oauth_version".to_string(), "1.0".to_string());

    let signing_key = format!("{}&{}", pct(consumer_secret), pct(request_token));
    let signature = oauth_signature("POST", ACCESS_TOKEN_ENDPOINT, &params, &signing_key);

    let mut header = params.clone();
    header.insert("oauth_signature".to_string(), signature);
    let mut parts: Vec<String> = header
        .iter()
        .map(|(k, v)| format!("{}={}", k, format!("\"{}\"", v)))
        .collect();
    parts.sort();
    let auth_header = format!("OAuth {}", parts.join(", "));

    let client = http_client()?;
    let resp = client
        .post(ACCESS_TOKEN_ENDPOINT)
        .header(reqwest::header::AUTHORIZATION, auth_header)
        .send()
        .map_err(|_| SocialError::OperationFailed)?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(SocialError::OperationFailed);
    }
    let token = parse_form(&body, "oauth_token").ok_or(SocialError::OperationFailed)?;
    let secret = parse_form(&body, "oauth_token_secret").ok_or(SocialError::OperationFailed)?;
    let user_id = parse_form(&body, "user_id").unwrap_or_default();
    Ok((token, secret, user_id))
}

/// X hesabına gerçek OAuth 1.0a akışıyla bağlanır.
pub fn connect(app: &tauri::AppHandle) -> Result<SocialAccountConnection, SocialError> {
    let log = |msg: &str| {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(
            std::env::temp_dir().join("esops_x_debug.log"),
        ) {
            let _ = writeln!(f, "[{}] {}", chrono_like_now(), msg);
        }
    };

    log("=== X connect basladi ===");
    let consumer_key = load_consumer_key().map_err(|e| {
        log(&format!("HATA load_consumer_key: {:?}", e));
        e
    })?;
    log(&format!("consumer_key yuklendi, uzunluk={}, ilk_8={}", consumer_key.len(), &consumer_key[..consumer_key.len().min(8)]));
    let consumer_secret = load_consumer_secret().map_err(|e| {
        log(&format!("HATA load_consumer_secret: {:?}", e));
        e
    })?;
    log(&format!("consumer_secret yuklendi, uzunluk={}", consumer_secret.len()));

    let listener = bind_loopback().map_err(|e| {
        log(&format!("HATA bind_loopback: {:?}", e));
        e
    })?;
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    log(&format!("loopback baglandi, port={}", port));

    let (req_token, _req_secret) = request_token(&listener).map_err(|e| {
        log(&format!("HATA request_token: {:?}", e));
        e
    })?;
    log(&format!("request_token basarili, token_uzunluk={}", req_token.len()));

    let auth_url = format!("{}?oauth_token={}", AUTHORIZE_ENDPOINT, pct(&req_token));
    log(&format!("auth_url: {}", auth_url));
    open_browser(app, &auth_url).map_err(|e| {
        log(&format!("HATA open_browser: {:?}", e));
        e
    })?;
    log("tarayici acildi, callback bekleniyor...");

    let verifier = wait_for_callback(&listener)?;
    let (access_token, access_token_secret, user_id) =
        exchange_access_token(&consumer_key, &consumer_secret, &req_token, &verifier)?;

    let dir = data_dir(app)?;
    let records = metadata_store::list_connections(&dir)?;
    let existing = records
        .iter()
        .find(|r| r.platform_id == PLATFORM_ID && r.external_account_id == user_id);
    let connection_id = match existing {
        Some(r) => r.connection_id.clone(),
        None => generate_connection_id()?,
    };

    credential_store::store_token(PLATFORM_ID, &connection_id, TokenType::AccessToken, &access_token)?;
    credential_store::store_token(
        PLATFORM_ID,
        &connection_id,
        TokenType::RefreshToken,
        &access_token_secret,
    )?;

    let account_name = existing
        .map(|r| r.account_display_name.clone())
        .unwrap_or_else(|| format!("X @{}", user_id));

    let record = ConnectionRecord {
        connection_id: connection_id.clone(),
        platform_id: PLATFORM_ID.to_string(),
        external_account_id: user_id.clone(),
        account_display_name: account_name.clone(),
        connection_status: ConnectionStatus::Connected,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    };
    metadata_store::upsert_connection(&dir, record).map_err(|_| SocialError::ConnectionStoreError)?;

    Ok(SocialAccountConnection {
        connection_id,
        platform_id: PLATFORM_ID.to_string(),
        external_account_id: user_id,
        account_display_name: account_name,
        connection_status: ConnectionStatus::Connected,
        token_exists: true,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    })
}

fn now_rfc3339() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!(
        "{}{:05}",
        dur.as_secs(),
        dur.subsec_nanos() / 1_000_000
    )
}

fn chrono_like_now() -> String {
    now_rfc3339()
}

// ---------------------------------------------------------------------------
// Medya yükleme (v1.1 media/upload.json) ve tweet oluşturma (v2 /2/tweets)
// ---------------------------------------------------------------------------

fn obtain_tokens(app: &tauri::AppHandle, record: &ConnectionRecord) -> Result<(String, String), SocialError> {
    let access = credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::AccessToken,
    )?
    .ok_or(SocialError::ConnectionStoreError)?;
    let secret = credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::RefreshToken,
    )?
    .ok_or(SocialError::ConnectionStoreError)?;
    Ok((access, secret))
}

fn mime_for(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".mp4") {
        "video/mp4"
    } else if lower.ends_with(".mov") {
        "video/quicktime"
    } else if lower.ends_with(".avi") {
        "video/x-msvideo"
    } else if lower.ends_with(".mkv") {
        "video/x-matroska"
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".m4v") || lower.ends_with(".3gp") {
        "video/mp4"
    } else if lower.ends_with(".mpeg") || lower.ends_with(".mpg") {
        "video/mpeg"
    } else if lower.ends_with(".ts") {
        "video/mp2t"
    } else if lower.ends_with(".wmv") {
        "video/x-ms-wmv"
    } else {
        "video/mp4"
    }
}

/// OAuth 1.0a ile imzalanmış `Authorization` başlığı. `extra` parametreleri
/// signature'a dahil edilir, `numral` form body parametreleri değildir.
fn build_auth_header(
    consumer_key: &str,
    consumer_secret: &str,
    access_token: &str,
    token_secret: &str,
    method: &str,
    base_url: &str,
    extra: &BTreeMap<String, String>,
) -> String {
    let mut params = oauth_params(consumer_key, access_token);
    for (k, v) in extra {
        params.insert(k.clone(), v.clone());
    }
    let signing_key = format!("{}&{}", pct(consumer_secret), pct(token_secret));
    let signature = oauth_signature(method, base_url, &params, &signing_key);
    params.insert("oauth_signature".to_string(), signature);

    let mut parts: Vec<String> = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, format!("\"{}\"", pct(v))))
        .collect();
    parts.sort();
    format!("OAuth {}", parts.join(", "))
}

/// Doğrudan (multipart) tek parça medya yükleme.
fn upload_media_direct(
    consumer_key: &str,
    consumer_secret: &str,
    access_token: &str,
    token_secret: &str,
    path: &str,
) -> Result<String, SocialError> {
    media_validation::verify_video_file(path)?;
    let bytes = std::fs::read(path).map_err(|_| SocialError::FileNotFound)?;
    let mime = mime_for(path);

    let extra: BTreeMap<String, String> = BTreeMap::new();
    let auth = build_auth_header(
        consumer_key,
        consumer_secret,
        access_token,
        token_secret,
        "POST",
        MEDIA_UPLOAD_ENDPOINT,
        &extra,
    );

    let client = http_client()?;
    let form = reqwest::blocking::multipart::Form::new()
        .part(
            "media",
            reqwest::blocking::multipart::Part::bytes(bytes)
                .file_name("media".to_string())
                .mime_str(mime)
                .map_err(|_| SocialError::OperationFailed)?,
        )
        .text("media_type", mime.to_string());
    let resp = client
        .post(MEDIA_UPLOAD_ENDPOINT)
        .header(reqwest::header::AUTHORIZATION, auth)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::OperationFailed)?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(SocialError::OperationFailed);
    }
    parse_json_str(&body, "media_id_string")
        .or_else(|| parse_json_str(&body, "media_id"))
        .ok_or(SocialError::OperationFailed)
}

fn parse_json_str(body: &str, key: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let v = json.get(key)?.as_str()?;
    Some(v.to_string())
}

/// Parçalı medya yükleme (INIT + APPEND + FINALIZE). Büyük videolar için.
fn upload_media_chunked(
    consumer_key: &str,
    consumer_secret: &str,
    access_token: &str,
    token_secret: &str,
    path: &str,
) -> Result<String, SocialError> {
    media_validation::verify_video_file(path)?;
    let bytes = std::fs::read(path).map_err(|_| SocialError::FileNotFound)?;
    let mime = mime_for(path);
    let total_chunks = bytes.len().div_ceil(CHUNK_SIZE).max(1);

    // 1) INIT
    let init_extra: BTreeMap<String, String> = BTreeMap::new();
    let init_auth = build_auth_header(
        consumer_key,
        consumer_secret,
        access_token,
        token_secret,
        "POST",
        MEDIA_UPLOAD_ENDPOINT,
        &init_extra,
    );
    let client = http_client()?;
    let form = reqwest::blocking::multipart::Form::new()
        .text("command", "INIT")
        .text("media_type", mime.to_string())
        .text("total_bytes", bytes.len().to_string());
    let init_resp = client
        .post(MEDIA_UPLOAD_ENDPOINT)
        .header(reqwest::header::AUTHORIZATION, init_auth)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::OperationFailed)?;
    let init_status = init_resp.status();
    let init_body = init_resp.text().unwrap_or_default();
    if !init_status.is_success() {
        return Err(SocialError::OperationFailed);
    }
    let media_id = parse_json_str(&init_body, "media_id_string")
        .or_else(|| parse_json_str(&init_body, "media_id"))
        .ok_or(SocialError::OperationFailed)?;

    // 2) APPEND (her parça ayrı istek, base64'e kodlanır)
    for i in 0..total_chunks {
        let start = i * CHUNK_SIZE;
        let end = std::cmp::min(start + CHUNK_SIZE, bytes.len());
        let chunk = &bytes[start..end];

        let mut extra: BTreeMap<String, String> = BTreeMap::new();
        extra.insert("media_id".to_string(), media_id.clone());
        extra.insert("segment_index".to_string(), i.to_string());
        let append_auth = build_auth_header(
            consumer_key,
            consumer_secret,
            access_token,
            token_secret,
            "POST",
            MEDIA_UPLOAD_ENDPOINT,
            &extra,
        );
        let appended = BASE64.encode(chunk);
        let form = reqwest::blocking::multipart::Form::new()
            .text("command", "APPEND")
            .text("media_id", media_id.clone())
            .text("segment_index", i.to_string())
            .part(
                "media_data",
                reqwest::blocking::multipart::Part::bytes(appended.into_bytes())
                    .mime_str("application/octet-stream")
                    .map_err(|_| SocialError::OperationFailed)?,
            );
        let append_resp = client
            .post(MEDIA_UPLOAD_ENDPOINT)
            .header(reqwest::header::AUTHORIZATION, append_auth)
            .multipart(form)
            .send()
            .map_err(|_| SocialError::OperationFailed)?;
        let append_status = append_resp.status();
        let _ = append_resp.text().unwrap_or_default();
        if !append_status.is_success() {
            // X bazı parçalarda 200/201 döner; aralıksız devam eder, hata halinde abort.
            return Err(SocialError::OperationFailed);
        }
    }

    // 3) FINALIZE
    let mut final_extra: BTreeMap<String, String> = BTreeMap::new();
    final_extra.insert("media_id".to_string(), media_id.clone());
    let final_auth = build_auth_header(
        consumer_key,
        consumer_secret,
        access_token,
        token_secret,
        "POST",
        MEDIA_UPLOAD_ENDPOINT,
        &final_extra,
    );
    let form = reqwest::blocking::multipart::Form::new()
        .text("command", "FINALIZE")
        .text("media_id", media_id.clone());
    let final_resp = client
        .post(MEDIA_UPLOAD_ENDPOINT)
        .header(reqwest::header::AUTHORIZATION, final_auth)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::OperationFailed)?;
    let final_status = final_resp.status();
    let _ = final_resp.text().unwrap_or_default();
    if !final_status.is_success() {
        return Err(SocialError::OperationFailed);
    }
    Ok(media_id)
}

fn upload_media(
    consumer_key: &str,
    consumer_secret: &str,
    access_token: &str,
    token_secret: &str,
    path: &str,
) -> Result<String, SocialError> {
    let len = std::fs::metadata(path).map(|m| m.len() as usize).unwrap_or(0);
    if len > DIRECT_MEDIA_LIMIT {
        upload_media_chunked(consumer_key, consumer_secret, access_token, token_secret, path)
    } else {
        upload_media_direct(consumer_key, consumer_secret, access_token, token_secret, path)
    }
}

/// X bağlantısına gerçek API ile yayın yapar (media upload + tweet.create).
pub fn publish_video(
    app: &tauri::AppHandle,
    connection_id: &str,
    video_path: &str,
    title: &str,
) -> Result<String, SocialError> {
    if connection_id.trim().is_empty() {
        return Err(SocialError::InvalidConnection);
    }
    let dir = data_dir(app)?;
    let record = metadata_store::get_connection(&dir, connection_id)?
        .ok_or(SocialError::InvalidConnection)?;
    if record.platform_id != PLATFORM_ID {
        return Err(SocialError::InvalidConnection);
    }
    if record.connection_status != ConnectionStatus::Connected {
        return Err(SocialError::InvalidConnection);
    }

    let consumer_key = load_consumer_key()?;
    let consumer_secret = load_consumer_secret()?;
    let (access_token, token_secret) = obtain_tokens(app, &record)?;

    let media_id = upload_media(
        &consumer_key,
        &consumer_secret,
        &access_token,
        &token_secret,
        video_path,
    )?;

    // tweet.create
    let extra: BTreeMap<String, String> = BTreeMap::new();
    let auth = build_auth_header(
        &consumer_key,
        &consumer_secret,
        &access_token,
        &token_secret,
        "POST",
        TWEETS_ENDPOINT,
        &extra,
    );

    let req_body = serde_json::json!({
        "text": title,
        "media": { "media_ids": [media_id] }
    });
    let client = http_client()?;
    let resp = client
        .post(TWEETS_ENDPOINT)
        .header(reqwest::header::AUTHORIZATION, auth)
        .json(&req_body)
        .send()
        .map_err(|_| SocialError::OperationFailed)?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(SocialError::OperationFailed);
    }
    let data: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    let tweet_id = data
        .get("data")
        .and_then(|d| d.get("id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let tw_id = tweet_id.ok_or(SocialError::OperationFailed)?;
    // Gerçek yayın id döndürülür.
    Ok(tw_id)
}





