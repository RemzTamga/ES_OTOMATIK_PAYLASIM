//! Gerçek TikTok entegrasyonu.
//!
//! Bu modül, TikTok Content Posting API'nin OAuth akışını uygular:
//! - Sistem tarayıcısını açar,
//! - `127.0.0.1` üzerinde dinamik bir loopback callback dinler,
//! - Kriptografik olarak güvenli `state` kullanır (`response_type=code`),
//! - Yetkilendirme kodunu gerçek TikTok token endpoint'iyle değiştirir,
//! - `user.info.basic,video.publish` izinlerini ister,
//! - Gerçek kullanıcı kimliğini (open_id) ve ekran adını
//!   TikTok User Info API'den alır,
//! - Tokenları yalnız Windows Credential Manager'da saklar.
//!
//! TikTok, benzer Meta (Facebook/Instagram) modelinde kullanıcı tarafından
//! yapılandırılan Client Key ve Client Secret gerektirir. Client Key derleme
//! zamanında `ES_OPS_TIKTOK_CLIENT_KEY` üzerinden (varsa) gömülebilir; Client
//! Secret güvenli olarak kullanıcı tarafından girilip Credential Manager'da
//! saklanır (kaynağa/loga/binary'ye gömülmez). Gizli hiçbir bilgi
//! JavaScript'e, DOM'a veya lokale döndürülmez.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

use super::super::credential_store;
use super::super::media_validation;
use super::super::metadata_store;
use super::super::models::{
    ConnectionRecord, ConnectionStatus, SocialAccountConnection, SocialError, TokenType,
};

/// TikTok platform kimliği (katalogdaki değer).
pub const PLATFORM_ID: &str = "tiktok";

/// TikTok OAuth yetkilendirme uç adresi.
const AUTHORIZE_ENDPOINT: &str = "https://www.tiktok.com/v2/auth/authorize/";
/// TikTok token endpoint (HTTP POST, form-urlencoded).
const TOKEN_ENDPOINT: &str = "https://open.tiktokapis.com/v2/oauth/token/";
/// TikTok kullanıcı bilgi endpoint'i.
const USER_INFO_ENDPOINT: &str = "https://open.tiktokapis.com/v2/user/info/";
/// TikTok video yayınını başlatan endpoint (Content Posting API).
const VIDEO_INIT_ENDPOINT: &str = "https://open.tiktokapis.com/v2/post/publish/video/init/";
/// TikTok yayın durumunu sorgulayan endpoint (Content Posting API).
const PUBLISH_STATUS_ENDPOINT: &str =
    "https://open.tiktokapis.com/v2/post/publish/status/fetch/";

/// Video yükleme için tekil parça boyutu (Content Posting API varsayılanı).
const CHUNK_SIZE: usize = 1024 * 1024;

/// OAuth callback'inin kaç saniye beklenileceği.
const OAUTH_TIMEOUT_SECS: u64 = 300;

/// TikTok Client Key, derleme zamanında (varsa) güvenli biçimde gömülür.
/// Değer tanımlı değilse `None` döner; derleme bu yüzden başarısız olmaz.
fn tiktok_client_key() -> Option<&'static str> {
    option_env!("ES_OPS_TIKTOK_CLIENT_KEY")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

// ---- Güvenli rastgele üretim (OAuth state) ----

/// Kriptografik olarak güvenli rastgele bayt üretir.
fn random_bytes(len: usize) -> Result<Vec<u8>, SocialError> {
    let mut buf = vec![0u8; len];
    rand::rngs::OsRng
        .try_fill_bytes(&mut buf)
        .map_err(|_| SocialError::OperationFailed)?;
    Ok(buf)
}

/// OAuth `state` değeri üretir (URL-safe, tahmin edilemez).
fn generate_state() -> Result<String, SocialError> {
    let bytes = random_bytes(32)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// Benzersiz bağlantı kimliği üretir.
fn generate_connection_id() -> Result<String, SocialError> {
    let bytes = random_bytes(16)?;
    Ok(format!("tiktok_{}", URL_SAFE_NO_PAD.encode(bytes)))
}

// ---- Loopback callback ----

/// `127.0.0.1` üzerinde dinamik (serbest) bir portta dinleyici açar.
fn bind_loopback() -> Result<TcpListener, SocialError> {
    TcpListener::bind(("127.0.0.1", 0)).map_err(|_| SocialError::OauthTimeout)
}

/// Loopback gelen isteğindeki `code` ve `state` değerlerini ayrıştırır.
fn parse_callback_query(query: &str) -> (Option<String>, Option<String>) {
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        if let Some(eq) = pair.find('=') {
            let k = &pair[..eq];
            let v = &pair[eq + 1..];
            match k {
                "code" => code = Some(v.to_string()),
                "state" => state = Some(v.to_string()),
                _ => {}
            }
        }
    }
    (code, state)
}

/// Callback dinlenecek kadar bekler ve `(code, state)` döndürür.
fn wait_for_callback(listener: &TcpListener) -> Result<(String, String), SocialError> {
    listener
        .set_nonblocking(true)
        .map_err(|_| SocialError::OauthTimeout)?;

    let deadline = std::time::Instant::now() + Duration::from_secs(OAUTH_TIMEOUT_SECS);

    loop {
        if std::time::Instant::now() > deadline {
            return Err(SocialError::OauthTimeout);
        }

        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let mut total = Vec::new();
                let mut buf = [0u8; 4096];
                for _ in 0..128 {
                    match stream.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            total.extend_from_slice(&buf[..n]);
                            if total.windows(4).any(|w| w == b"\r\n\r\n") {
                                break;
                            }
                            if total.len() >= 8192 {
                                break;
                            }
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(50));
                            if std::time::Instant::now() > deadline {
                                return Err(SocialError::OauthTimeout);
                            }
                            continue;
                        }
                        Err(_) => break,
                    }
                }

                let request_text = String::from_utf8_lossy(&total);
                let path_and_query = request_text
                    .lines()
                    .next()
                    .and_then(|l| l.split_whitespace().nth(1))
                    .unwrap_or("/");

                let response_body =
                    "<html><body><h3>ES OPS</h3><p>Baglanti tamamlandi. Bu pencereyi kapatabilirsiniz.</p></body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();

                let query = path_and_query
                    .split_once('?')
                    .map(|(_, q)| q)
                    .unwrap_or("");

                if query.contains("error=") {
                    return Err(SocialError::OauthCancelled);
                }
                let (code, state) = parse_callback_query(query);
                let code = code.ok_or(SocialError::OauthExchangeFailed)?;
                let state = state.ok_or(SocialError::OauthStateMismatch)?;
                return Ok((code, state));
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => {
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

// ---- HTTP istemci ----

fn http_client() -> Result<reqwest::blocking::Client, SocialError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| SocialError::ApiError)
}

/// Tarayıcıyı resmî Tauri shell mekanizmasıyla açar.
fn open_browser(app: &AppHandle, url: &str) -> Result<(), SocialError> {
    app.shell()
        .open(url, None)
        .map_err(|_| SocialError::OperationFailed)
}

// ---- OAuth URL ----

/// TikTok yetkilendirme URL'sini oluşturur (`response_type=code`).
/// TLS'ye ilişkin PKCE isteğe bağlı olduğundan burada kullanılmaz; yalnız
/// resmî `response_type=code` + state akışı uygulanır.
fn build_authorize_url(
    client_key: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
) -> String {
    format!(
        "{AUTHORIZE_ENDPOINT}?response_type=code&client_key={client_key}&redirect_uri={redirect_uri}&scope={scope}&state={state}&disable_visibility_permission=1"
    )
}

// ---- Token exchange ----

struct TokenSet {
    access_token: String,
    open_id: String,
    scope: String,
    refresh_token: Option<String>,
}

/// Yetkilendirme kodunu TikTok token endpoint'inde client_secret ile değiştirir.
fn exchange_code(
    client_key: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<TokenSet, SocialError> {
    let client = http_client()?;
    let params = [
        ("client_key", client_key),
        ("client_secret", client_secret),
        ("code", code),
        ("grant_type", "authorization_code"),
        ("redirect_uri", redirect_uri),
    ];
    let resp = client
        .post(TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .map_err(|_| SocialError::OauthExchangeFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(SocialError::OauthExchangeFailed);
    }

    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|_| SocialError::OauthExchangeFailed)?;
    // TikTok başarılı yanıtta kod 0 / success döndürür.
    let access_token = match parsed.access_token {
        Some(t) if !t.is_empty() => t,
        _ => {
            let code = parsed.code.unwrap_or(-1);
            let msg = parsed.message.clone().unwrap_or_default();
            if is_token_scope_error(code, &msg) {
                return Err(SocialError::PermissionDenied);
            }
            return Err(SocialError::OauthExchangeFailed);
        }
    };
    let open_id = parsed.open_id.unwrap_or_default();
    if open_id.is_empty() {
        return Err(SocialError::OauthExchangeFailed);
    }
    Ok(TokenSet {
        access_token,
        open_id,
        scope: parsed.scope.unwrap_or_default(),
        refresh_token: parsed.refresh_token,
    })
}

/// TikTok hata kodunu dengeli biçimde eşler (token/kod sızdırmaz).
fn is_token_scope_error(code: i64, msg: &str) -> bool {
    if code == 10023 || code == 100027 {
        // 100023 / 100027: yetki / kapsam sorunu.
        return true;
    }
    let lower = msg.to_lowercase();
    lower.contains("invalid scope")
        || lower.contains("unauthorize")
        || lower.contains("permission")
}

// ---- Kullanıcı bilgisi ----

struct UserInfo {
    open_id: String,
    display_name: String,
}

/// Mevcut access_token ile TikTok kullanıcı bilgisini alır.
fn fetch_user_info(access_token: &str) -> Result<UserInfo, SocialError> {
    let client = http_client()?;
    let resp = client
        .get(USER_INFO_ENDPOINT)
        .query(&[("fields", "open_id,display_name")])
        .bearer_auth(access_token)
        .send()
        .map_err(|_| SocialError::ChannelLookupFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            return Err(SocialError::PermissionDenied);
        }
        return Err(SocialError::ChannelLookupFailed);
    }

    let parsed: UserInfoResponse =
        serde_json::from_str(&body).map_err(|_| SocialError::ChannelLookupFailed)?;
    let data = parsed.data.unwrap_or_default();
    let user = data.user.unwrap_or_default();
    let open_id = user.open_id.unwrap_or_default();
    if open_id.is_empty() {
        return Err(SocialError::ChannelLookupFailed);
    }
    let display_name = user
        .display_name
        .unwrap_or_default()
        .unwrap_or_else(|| open_id.clone());
    Ok(UserInfo {
        open_id,
        display_name,
    })
}

// ---- Bağlantı kurma ----

fn data_dir(app: &AppHandle) -> Result<std::path::PathBuf, SocialError> {
    app.path()
        .app_data_dir()
        .map_err(|_| SocialError::ConnectionStoreError)
}

fn now_rfc3339() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = now / 86400;
    let seconds = now % 86400;
    format!("{}d+{}s", days, seconds)
}

/// OAuth başarısı sonrasında bağlantıyı kurar.
fn connect_for_user(
    app: &AppHandle,
    tokens: &TokenSet,
    user: &UserInfo,
) -> Result<SocialAccountConnection, SocialError> {
    let dir = data_dir(app)?;
    let records = metadata_store::list_connections(&dir)?;

    let existing = records
        .iter()
        .find(|r| r.platform_id == PLATFORM_ID && r.external_account_id == user.open_id);
    let connection_id = match existing {
        Some(r) => r.connection_id.clone(),
        None => generate_connection_id()?,
    };

    credential_store::store_token(
        PLATFORM_ID,
        &connection_id,
        TokenType::AccessToken,
        &tokens.access_token,
    )
    .map_err(|_| SocialError::CredentialStoreError)?;
    if let Some(rt) = &tokens.refresh_token {
        if !rt.is_empty()
            && credential_store::store_token(PLATFORM_ID, &connection_id, TokenType::RefreshToken, rt)
                .is_err()
        {
            let _ = credential_store::delete_all_tokens(PLATFORM_ID, &connection_id);
            return Err(SocialError::CredentialStoreError);
        }
    }

    let record = ConnectionRecord {
        connection_id: connection_id.clone(),
        platform_id: PLATFORM_ID.to_string(),
        external_account_id: user.open_id.clone(),
        account_display_name: user.display_name.clone(),
        connection_status: ConnectionStatus::Connected,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    };

    if metadata_store::upsert_connection(&dir, record).is_err() {
        let _ = credential_store::delete_all_tokens(PLATFORM_ID, &connection_id);
        return Err(SocialError::ConnectionStoreError);
    }

    Ok(SocialAccountConnection {
        connection_id,
        platform_id: PLATFORM_ID.to_string(),
        external_account_id: user.open_id.clone(),
        account_display_name: user.display_name.clone(),
        connection_status: ConnectionStatus::Connected,
        token_exists: true,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    })
}

/// TikTok'a gerçek OAuth ile bağlanır.
///
/// Client Key derleme zamanından veya (kullanıcı tarafından yapılandırılmışsa)
/// güvenli depodan çözülür; Client Secret yalnız güvenli depodan okunur.
/// Değerlerden biri eksikse `TiktokNotConfigured` / `OperationFailed` döner
/// ve tarayıcı anlamsız bir oturum için açılmaz (sahte bağlantı üretilmez).
pub fn connect(app: &AppHandle) -> Result<SocialAccountConnection, SocialError> {
    let client_key = resolved_client_key().ok_or(SocialError::TiktokNotConfigured)?;
    let client_secret = resolved_client_secret().ok_or(SocialError::OperationFailed)?;

    let listener = bind_loopback()?;
    let port = listener
        .local_addr()
        .map_err(|_| SocialError::OauthTimeout)?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/");
    let state = generate_state()?;
    let scope = "user.info.basic,video.publish";

    let auth_url = build_authorize_url(&client_key, &redirect_uri, scope, &state);
    open_browser(app, &auth_url)?;

    let (code, callback_state) = wait_for_callback(&listener)?;
    if callback_state != state {
        return Err(SocialError::OauthStateMismatch);
    }

    let tokens = exchange_code(&client_key, &client_secret, &redirect_uri, &code)?;
    let user = fetch_user_info(&tokens.access_token)?;

    connect_for_user(app, &tokens, &user)
}

// ---- TikTok yapılandırma (Client Key / Client Secret) ----

/// TikTok yapılandırması için kullanılan ortak (bağlantıya özgü olmayan) anahtar.
const TIKTOK_CONFIG_CONN: &str = "_tiktok_app_config";

/// Client Key'i güvenli depoya yazar (kaynağa gömülmez).
pub fn store_client_key(client_key: &str) -> Result<(), SocialError> {
    if client_key.trim().is_empty() {
        return Err(SocialError::TiktokNotConfigured);
    }
    credential_store::store_token(
        "tiktok",
        TIKTOK_CONFIG_CONN,
        TokenType::RefreshToken,
        client_key.trim(),
    )
}

/// Client Secret'ı güvenli depoya yazar (kaynağa gömülmez).
pub fn store_client_secret(client_secret: &str) -> Result<(), SocialError> {
    if client_secret.trim().is_empty() {
        return Err(SocialError::OperationFailed);
    }
    credential_store::store_token(
        "tiktok",
        TIKTOK_CONFIG_CONN,
        TokenType::AccessToken,
        client_secret.trim(),
    )
}

/// Güvenli depodan Client Key'i okur.
fn read_client_key() -> Result<Option<String>, SocialError> {
    credential_store::get_token("tiktok", TIKTOK_CONFIG_CONN, TokenType::RefreshToken)
}

/// Güvenli depodan Client Secret'ı okur.
/// Ham secret JavaScript'e asla döndürülmez; yalnız Rust içinde kullanılır.
fn read_client_secret() -> Result<Option<String>, SocialError> {
    credential_store::get_token("tiktok", TIKTOK_CONFIG_CONN, TokenType::AccessToken)
}

/// Kullanım sırasında çözülecek Client Key. Önce derleme zamanı
/// `ES_OPS_TIKTOK_CLIENT_KEY`, varsa onu, yoksa güvenli depoyu kullanır.
fn resolved_client_key() -> Option<String> {
    tiktok_client_key()
        .map(|s| s.to_string())
        .or_else(|| read_client_key().ok().flatten())
}

/// Kullanım sırasında çözülecek Client Secret. Yalnız güvenli depodan okunur.
fn resolved_client_secret() -> Option<String> {
    read_client_secret().ok().flatten()
}

/// Client Key / Client Secret'in yapılandırılıp yapılandırılmadığını döndürür.
/// Ham secret asla döndürülmez.
pub fn config_status() -> Result<(bool, bool), SocialError> {
    let has_key = resolved_client_key().is_some();
    let has_secret = read_client_secret()?.is_some();
    Ok((has_key, has_secret))
}

/// Güvenli depodaki TikTok yapılandırmasını (Client Key / Client Secret) temizler.
pub fn clear_config() -> Result<(), SocialError> {
    credential_store::delete_all_tokens("tiktok", TIKTOK_CONFIG_CONN)
}

// ---- Video yayınlama (Content Posting API) ----

/// TikTok video gizlilik düzeyi (kontrollü değerler).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacyLevel {
    /// Herkese açık.
    Public,
    /// Yalnızca kendisi görür.
    SelfOnly,
}

impl PrivacyLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrivacyLevel::Public => "PUBLIC_TO_EVERYONE",
            PrivacyLevel::SelfOnly => "SELF_ONLY",
        }
    }

    pub fn parse(value: &str) -> Option<PrivacyLevel> {
        match value.trim().to_ascii_uppercase().as_str() {
            "PUBLIC_TO_EVERYONE" | "PUBLIC" => Some(PrivacyLevel::Public),
            "SELF_ONLY" | "PRIVATE" => Some(PrivacyLevel::SelfOnly),
            _ => None,
        }
    }
}

/// Belirli bir bağlantıya ait TikTok erişim tokenını güvenli depodan okur.
fn obtain_access_token(app: &AppHandle, record: &ConnectionRecord) -> Result<String, SocialError> {
    let access = credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::AccessToken,
    )
    .map_err(|_| SocialError::TokenMissing)?;
    access
        .filter(|t| !t.is_empty())
        .ok_or(SocialError::TokenMissing)
}

/// Video dosyasını TikTok tarafından verilen arka plandan presigned URL'ye yükler.
fn upload_chunks(
    upload_url: &str,
    bytes: &[u8],
    chunk_size: usize,
) -> Result<(), SocialError> {
    if upload_url.is_empty() {
        return Err(SocialError::UploadFailed);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()
        .map_err(|_| SocialError::ApiError)?;

    let total = bytes.len();
    let mut start = 0usize;
    let mut index = 0usize;
    let total_chunks = if chunk_size == 0 {
        1
    } else {
        total.div_ceil(chunk_size).max(1)
    };

    while start < total {
        let end = (start + chunk_size).min(total);
        let chunk = &bytes[start..end];
        let content_range = format!(
            "bytes {}-{}/{}",
            start,
            end - 1,
            total
        );

        let mut req = client
            .put(upload_url)
            .header(reqwest::header::CONTENT_TYPE, "video/mp4")
            .header("Content-Range", content_range)
            .header("Content-Length", chunk.len().to_string())
            .body(chunk.to_vec());

        // İlk parçada oluşturma modu, sonraki parçalarda devam modu belirtilir.
        if index == 0 {
            req = req.header("Content-Disposition", "attachment; filename=\"video.mp4\"");
        }

        let resp = req.send().map_err(|_| SocialError::UploadFailed)?;
        let status = resp.status();
        if !status.is_success() && status != reqwest::StatusCode::PARTIAL_CONTENT {
            // 308 "Resume Incomplete" bazı arka planlar tarafından döndürülür.
            if status != reqwest::StatusCode::PERMANENT_REDIRECT {
                return Err(SocialError::UploadFailed);
            }
        }

        start = end;
        index += 1;
        let _ = total_chunks;
    }
    Ok(())
}

/// Yayın durumunu belirli bir süre boyunca yoklar ve nihai sonucu döndürür.
fn poll_publish_status(
    app: &AppHandle,
    access_token: &str,
    publish_id: &str,
) -> Result<(), SocialError> {
    let client = http_client()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(300);

    loop {
        if std::time::Instant::now() > deadline {
            return Err(SocialError::MediaProcessingTimeout);
        }

        let payload = serde_json::json!({ "publish_id": publish_id });
        let resp = client
            .post(PUBLISH_STATUS_ENDPOINT)
            .bearer_auth(access_token)
            .json(&payload)
            .send()
            .map_err(|_| SocialError::PublishFailed)?;

        let status = resp.status();
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                update_status(app, &String::new(), ConnectionStatus::TokenExpired);
            }
            return Err(SocialError::PermissionDenied);
        }
        if !status.is_success() {
            return Err(SocialError::PublishFailed);
        }

        let body = resp.text().unwrap_or_default();
        let parsed: PublishStatusResponse = match serde_json::from_str(&body) {
            Ok(p) => p,
            Err(_) => return Err(SocialError::PublishFailed),
        };
        let state = parsed.status.clone().unwrap_or_default();
        match state.as_str() {
            "PUBLISH_COMPLETE" => return Ok(()),
            "FAILED" | "PUBLISH_FAILED" => {
                let reason = parsed
                    .failed_reason
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "bilinmeyen neden".to_string());
                let _ = eprintln!(
                    "[es-ops:tiktok] publish failed: {} (reason: {})",
                    publish_id, reason
                );
                return Err(SocialError::PublishFailed);
            }
            // PROCESSING_DOWNSTREAM / PUBLISHING / vb.: bekle ve tekrar dene.
            _ => {
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    }
}

/// TikTok bağlantısına gerçek Content Posting API ile video yayınlar.
///
/// - Bağlı bağlantının erişim tokenı güvenli depodan okunur.
/// - `POST /v2/post/publish/video/init/` ile yayın oturumu başlatılır.
/// - Video dosyası verilen presigned URL'ye parçalar halinde PUT ile yüklenir.
/// - `POST /v2/post/publish/status/fetch/` ile yayın durumu pubish tamamlanana
///   kadar yoklanır.
///
/// Gerçek başarıda boş string döndürülmez; herhangi bir adımda başarısızlıkta
/// kontrollü `SocialError` döner. Sahte yayın, sahte video id veya taklit
/// başarı üretilmez.
pub fn publish_video(
    app: &AppHandle,
    connection_id: &str,
    video_path: &str,
    title: &str,
    privacy: PrivacyLevel,
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

    media_validation::verify_video_file(video_path)?;
    let bytes = std::fs::read(video_path).map_err(|_| SocialError::FileNotFound)?;
    if bytes.is_empty() {
        return Err(SocialError::InvalidVideoFile);
    }

    let access_token = obtain_access_token(app, &record)?;

    // 1) Yayın oturumunu başlat.
    let chunk_size = CHUNK_SIZE;
    let total_chunk_count = bytes.len().div_ceil(chunk_size).max(1);
    let init_payload = serde_json::json!({
        "post_info": {
            "title": title,
            "privacy_level": privacy.as_str(),
            "disable_comment": false,
            "disable_duet": false,
            "disable_stitch": false,
            "video_cover_timestamp_ms": 1000
        },
        "source_info": {
            "source": "FILE_UPLOAD",
            "video_size": bytes.len(),
            "chunk_size": chunk_size,
            "total_chunk_count": total_chunk_count
        }
    });

    let client = http_client()?;
    let init_resp = client
        .post(VIDEO_INIT_ENDPOINT)
        .bearer_auth(&access_token)
        .json(&init_payload)
        .send()
        .map_err(|_| SocialError::UploadSessionFailed)?;

    let init_status = init_resp.status();
    if init_status == reqwest::StatusCode::UNAUTHORIZED
        || init_status == reqwest::StatusCode::FORBIDDEN
    {
        if init_status == reqwest::StatusCode::UNAUTHORIZED {
            update_status(app, &record.connection_id, ConnectionStatus::TokenExpired);
        }
        return Err(SocialError::PermissionDenied);
    }
    let init_body = init_resp.text().unwrap_or_default();
    if !init_status.is_success() || init_body.is_empty() {
        return Err(SocialError::UploadSessionFailed);
    }
    let init_parsed: VideoInitResponse =
        serde_json::from_str(&init_body).map_err(|_| SocialError::UploadSessionFailed)?;
    let publish_id = init_parsed.publish_id.ok_or(SocialError::UploadSessionFailed)?;
    let upload_url = init_parsed.upload_url.ok_or(SocialError::UploadSessionFailed)?;

    // 2) Video parçalarını presigned URL'ye yükle.
    upload_chunks(&upload_url, &bytes, chunk_size)?;

    // 3) Yayın durumunu yokla.
    poll_publish_status(app, &access_token, &publish_id)?;

    // Gerçek yayın id döndürülür; taklit/yer tutucu üretilmez.
    Ok(publish_id)
}

/// Bağlantı durum meta verisini günceller (publish hatalarında).
fn update_status(app: &AppHandle, connection_id: &str, status: ConnectionStatus) {
    if let Ok(dir) = data_dir(app) {
        let _ = metadata_store::update_connection_status(&dir, connection_id, status);
    }
}

// ---- Serde yanıt yapıları ----

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
    open_id: Option<String>,
    scope: Option<String>,
    code: Option<i64>,
    message: Option<String>,
}

#[derive(serde::Deserialize)]
struct UserInfoResponse {
    data: Option<UserInfoData>,
}

#[derive(serde::Deserialize, Default)]
struct UserInfoData {
    user: Option<UserInfoUser>,
}

#[derive(serde::Deserialize, Default)]
struct UserInfoUser {
    open_id: Option<String>,
    display_name: Option<Option<String>>,
}

#[derive(serde::Deserialize)]
struct VideoInitResponse {
    publish_id: Option<String>,
    upload_url: Option<String>,
    #[allow(dead_code)]
    failed_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct PublishStatusResponse {
    status: Option<String>,
    #[allow(dead_code)]
    failed_reason: Option<String>,
}

// ---- Testler ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_urlsafe() {
        let s = generate_state().unwrap();
        assert!(!s.is_empty());
        assert!(s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn parse_callback_query_extracts_code_and_state() {
        let (code, state) = parse_callback_query("code=abc123&state=xyz");
        assert_eq!(code.as_deref(), Some("abc123"));
        assert_eq!(state.as_deref(), Some("xyz"));
    }

    #[test]
    fn parse_callback_query_handles_missing_parts() {
        let (code, state) = parse_callback_query("foo=bar");
        assert!(code.is_none());
        assert!(state.is_none());
    }

    #[test]
    fn authorize_url_includes_scopes_and_state() {
        let url = build_authorize_url(
            "clientkey123",
            "http://127.0.0.1:8080/",
            "user.info.basic,video.publish",
            "st",
        );
        assert!(url.contains("response_type=code"));
        assert!(url.contains("client_key=clientkey123"));
        assert!(url.contains("redirect_uri=http://127.0.0.1:8080/"));
        assert!(url.contains("scope=user.info.basic"));
        assert!(url.contains("state=st"));
    }

    #[test]
    fn token_scope_error_mapping() {
        assert!(is_token_scope_error(10023, "invalid scope"));
        assert!(is_token_scope_error(100027, "unauthorized"));
        assert!(!is_token_scope_error(0, "success"));
    }
}
