//! Gerçek YouTube entegrasyonu.
//!
//! Bu modül, Google'ın güncel masaüstü uygulaması OAuth 2.0 akışını uygular:
//! - Sistem tarayıcısını açar,
//! - `127.0.0.1` üzerinde dinamik bir loopback callback dinler,
//! - Kriptografik olarak güvenli `state` ve PKCE (`S256`) kullanır,
//! - Yetkilendirme kodunu gerçek Google token endpoint'iyle değiştirir,
//! - Offline erişim ister (refresh token),
//! - Gerçek kanal kimliğini ve kanal adını YouTube Data API'den alır,
//! - Tokenları yalnız Windows Credential Manager'da saklar,
//! - Gerçek `videos.insert` resumable upload ile video yükler.
//!
//! Ham tokenlar hiçbir koşulda JavaScript'e, DOM'a, localStorage'a, metadata
//! deposuna veya loglara gönderilmez. Client secret kullanılmaz; gerçek client
//! id yalnız derleme zamanında `ES_OPS_YOUTUBE_CLIENT_ID` üzerinden gömülür.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

use super::super::credential_store;
use super::super::media_validation;
use super::super::metadata_store;
use super::super::models::{
    ConnectionRecord, ConnectionStatus, SocialAccountConnection, SocialError, TokenType,
};

/// YouTube platform kimliği (mevcut katalogdaki değer).
pub const PLATFORM_ID: &str = "youtube";

const AUTH_ENDPOINT: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const CHANNELS_ENDPOINT: &str = "https://www.googleapis.com/youtube/v3/channels";
const VIDEOS_UPLOAD_ENDPOINT: &str = "https://www.googleapis.com/upload/youtube/v3/videos";
const VIDEOS_PART: &str = "snippet,status";

/// OAuth callback'inin kaç saniye beklenileceği.
const OAUTH_TIMEOUT_SECS: u64 = 300;

/// Client id, derleme zamanında güvenli biçimde gömülür.
/// Değer tanımlı değilse `None` döner; derleme bu yüzden başarısız olmaz.
fn youtube_client_id() -> Option<&'static str> {
    option_env!("ES_OPS_YOUTUBE_CLIENT_ID")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// Video gizlilik durumu (kontrollü değerler).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PrivacyStatus {
    Private,
    Unlisted,
    Public,
}

impl PrivacyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            PrivacyStatus::Private => "private",
            PrivacyStatus::Unlisted => "unlisted",
            PrivacyStatus::Public => "public",
        }
    }

    pub fn parse(value: &str) -> Option<PrivacyStatus> {
        match value.trim().to_ascii_lowercase().as_str() {
            "private" => Some(PrivacyStatus::Private),
            "unlisted" => Some(PrivacyStatus::Unlisted),
            "public" => Some(PrivacyStatus::Public),
            _ => None,
        }
    }
}

// ---- Yardımcılar: güvenli rastgele üretim ----

/// Kriptografik olarak güvenli rastgele bayt üretir.
fn random_bytes(len: usize) -> Result<Vec<u8>, SocialError> {
    let mut buf = vec![0u8; len];
    rand::rngs::OsRng
        .try_fill_bytes(&mut buf)
        .map_err(|_| SocialError::OperationFailed)?;
    Ok(buf)
}

/// OAuth `state` değeri üretir (URL-safe, tahmin edilemez).
pub fn generate_state() -> Result<String, SocialError> {
    let bytes = random_bytes(32)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// PKCE `code_verifier` üretir (43-128 arası, RFC 7636 karakter kümesi).
pub fn generate_code_verifier() -> Result<String, SocialError> {
    const ALPHABET: &[u8] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let bytes = random_bytes(64)?;
    let mut out = String::with_capacity(bytes.len());
    for b in bytes {
        out.push(ALPHABET[(b as usize) % ALPHABET.len()] as char);
    }
    Ok(out)
}

/// PKCE S256 `code_challenge` üretir (URL-safe base64, padsız).
pub fn compute_code_challenge(code_verifier: &str) -> Result<String, SocialError> {
    let hash = Sha256::digest(code_verifier.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(hash))
}

/// Benzersiz bağlantı kimliği üretir.
fn generate_connection_id() -> Result<String, SocialError> {
    let bytes = random_bytes(16)?;
    Ok(format!("youtube_{}", URL_SAFE_NO_PAD.encode(bytes)))
}

// ---- Loopback callback ----

/// `127.0.0.1` üzerinde dinamik (serbest) bir portta dinleyici açar.
fn bind_loopback() -> Result<TcpListener, SocialError> {
    TcpListener::bind(("127.0.0.1", 0)).map_err(|_| SocialError::OauthTimeout)
}

/// Loopback gelen isteğindeki `code` ve `state` değerlerini ayrıştırır.
pub fn parse_callback_query(query: &str) -> (Option<String>, Option<String>) {
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

// ---- OAuth URL ----

fn build_auth_url(
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    format!(
        "{AUTH_ENDPOINT}?client_id={client_id}&redirect_uri={redirect_uri}&response_type=code&scope={scope}&access_type=offline&state={state}&code_challenge={code_challenge}&code_challenge_method=S256"
    )
}

/// Tarayıcıyı resmî Tauri shell mekanizmasıyla açar.
fn open_browser(app: &AppHandle, url: &str) -> Result<(), SocialError> {
    app.shell()
        .open(url, None)
        .map_err(|_| SocialError::OperationFailed)
}

// ---- Token exchange / yenileme ----

struct TokenSet {
    access_token: String,
    refresh_token: Option<String>,
}

/// Yetkilendirme kodunu gerçek token endpoint'inde değiştirir (PKCE ile).
fn exchange_code(
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
) -> Result<TokenSet, SocialError> {
    let client = http_client()?;
    let params = [
        ("code", code),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
        ("grant_type", "authorization_code"),
        ("code_verifier", code_verifier),
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
    let access_token = parsed.access_token.ok_or(SocialError::OauthExchangeFailed)?;
    Ok(TokenSet {
        access_token,
        refresh_token: parsed.refresh_token,
    })
}

/// Refresh token ile yeni access token alır.
fn refresh_access_token(client_id: &str, refresh_token: &str) -> Result<TokenSet, SocialError> {
    let client = http_client()?;
    let params = [
        ("client_id", client_id),
        ("refresh_token", refresh_token),
        ("grant_type", "refresh_token"),
    ];
    let resp = client
        .post(TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .map_err(|_| SocialError::TokenRefreshFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(SocialError::TokenRefreshFailed);
    }

    let parsed: TokenResponse =
        serde_json::from_str(&body).map_err(|_| SocialError::TokenRefreshFailed)?;
    let access_token = parsed.access_token.ok_or(SocialError::TokenRefreshFailed)?;
    Ok(TokenSet {
        access_token,
        refresh_token: parsed.refresh_token,
    })
}

// ---- Kanal bilgisi ----

struct ChannelInfo {
    id: String,
    title: String,
}

/// Şu anki erişim tokenına ait kanalın kimliğini ve adını alır.
fn fetch_channel(access_token: &str) -> Result<ChannelInfo, SocialError> {
    let client = http_client()?;
    let resp = client
        .get(CHANNELS_ENDPOINT)
        .query(&[("part", "snippet"), ("mine", "true")])
        .bearer_auth(access_token)
        .send()
        .map_err(|_| SocialError::ChannelLookupFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(SocialError::ChannelLookupFailed);
    }

    let parsed: ChannelResponse =
        serde_json::from_str(&body).map_err(|_| SocialError::ChannelLookupFailed)?;
    let item = parsed
        .items
        .into_iter()
        .flatten()
        .next()
        .ok_or(SocialError::ChannelLookupFailed)?;
    let id = item.id.unwrap_or_default();
    let title = item.snippet.and_then(|s| s.title).unwrap_or_default();
    if id.is_empty() {
        return Err(SocialError::ChannelLookupFailed);
    }
    Ok(ChannelInfo { id, title })
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
fn connect_for_channel(
    app: &AppHandle,
    tokens: &TokenSet,
    channel: &ChannelInfo,
) -> Result<SocialAccountConnection, SocialError> {
    let dir = data_dir(app)?;
    let records = metadata_store::list_connections(&dir)?;

    let existing = records
        .iter()
        .find(|r| r.platform_id == PLATFORM_ID && r.external_account_id == channel.id);
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
        external_account_id: channel.id.clone(),
        account_display_name: channel.title.clone(),
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
        external_account_id: channel.id.clone(),
        account_display_name: channel.title.clone(),
        connection_status: ConnectionStatus::Connected,
        token_exists: true,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    })
}

/// YouTube'a gerçek OAuth ile bağlanır.
pub fn connect(app: &AppHandle) -> Result<SocialAccountConnection, SocialError> {
    let client_id = youtube_client_id().ok_or(SocialError::YoutubeNotConfigured)?;

    let listener = bind_loopback()?;
    let port = listener
        .local_addr()
        .map_err(|_| SocialError::OauthTimeout)?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/");

    let state = generate_state()?;
    let code_verifier = generate_code_verifier()?;
    let code_challenge = compute_code_challenge(&code_verifier)?;

    let scope = "https://www.googleapis.com/auth/youtube.upload https://www.googleapis.com/auth/youtube.readonly";
    let auth_url = build_auth_url(&client_id, &redirect_uri, scope, &state, &code_challenge);

    open_browser(app, &auth_url)?;

    let (code, callback_state) = wait_for_callback(&listener)?;
    if callback_state != state {
        return Err(SocialError::OauthStateMismatch);
    }

    let tokens = exchange_code(&client_id, &redirect_uri, &code, &code_verifier)?;
    let channel = fetch_channel(&tokens.access_token)?;

    connect_for_channel(app, &tokens, &channel)
}

// ---- Token edinme ve yenileme ----

fn obtain_access_token(
    app: &AppHandle,
    record: &ConnectionRecord,
) -> Result<String, SocialError> {
    let access = credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::AccessToken,
    )
    .map_err(|_| SocialError::CredentialStoreError)?;

    if let Some(token) = access {
        if !token.is_empty() {
            return Ok(token);
        }
    }

    let refresh = credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::RefreshToken,
    )
    .map_err(|_| SocialError::CredentialStoreError)?;

    let refresh = match refresh {
        Some(r) if !r.is_empty() => r,
        _ => {
            update_status(app, &record.connection_id, ConnectionStatus::TokenExpired);
            return Err(SocialError::TokenExpired);
        }
    };

    let client_id = youtube_client_id().ok_or(SocialError::YoutubeNotConfigured)?;
    let tokens = refresh_access_token(client_id, &refresh).map_err(|_| {
        update_status(app, &record.connection_id, ConnectionStatus::Error);
        SocialError::TokenRefreshFailed
    })?;

    if credential_store::store_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::AccessToken,
        &tokens.access_token,
    )
    .is_err()
    {
        return Err(SocialError::CredentialStoreError);
    }
    if let Some(rt) = &tokens.refresh_token {
        let _ = credential_store::store_token(
            &record.platform_id,
            &record.connection_id,
            TokenType::RefreshToken,
            rt,
        );
    }

    Ok(tokens.access_token)
}

fn update_status(app: &AppHandle, connection_id: &str, status: ConnectionStatus) {
    if let Ok(dir) = data_dir(app) {
        let _ = metadata_store::update_connection_status(&dir, connection_id, status);
    }
}

// ---- Video yükleme ----

enum PutOutcome {
    Uploaded(String),
    AuthRequired,
    NeedsRetry(String, u64),
    Failed(SocialError),
}

fn init_resumable_session(
    client: &reqwest::blocking::Client,
    access_token: &str,
    title: &str,
    description: &str,
    privacy: PrivacyStatus,
    mime_type: &str,
) -> Result<String, PutOutcome> {
    let metadata = serde_json::json!({
        "snippet": {
            "title": title,
            "description": description
        },
        "status": {
            "privacyStatus": privacy.as_str()
        }
    });

    let resp = match client
        .post(format!(
            "{VIDEOS_UPLOAD_ENDPOINT}?uploadType=resumable&part={VIDEOS_PART}"
        ))
        .bearer_auth(access_token)
        .header("Content-Type", "application/json")
        .header("X-Upload-Content-Type", mime_type)
        .body(metadata.to_string())
        .send()
    {
        Ok(r) => r,
        Err(_) => return Err(PutOutcome::Failed(SocialError::UploadSessionFailed)),
    };

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Err(PutOutcome::AuthRequired);
    }
    if !status.is_success() {
        return Err(PutOutcome::Failed(SocialError::UploadSessionFailed));
    }

    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    location.ok_or(PutOutcome::Failed(SocialError::UploadSessionFailed))
}

fn upload_to_session(
    client: &reqwest::blocking::Client,
    session_uri: &str,
    bytes: &[u8],
    mime_type: &str,
    offset: u64,
) -> Result<PutOutcome, PutOutcome> {
    let body = &bytes[(offset as usize)..];

    let mut req = client
        .put(session_uri)
        .header("Content-Type", mime_type)
        .body(body.to_vec());
    if offset > 0 {
        req = req.header(
            "Content-Range",
            format!("bytes {}-{}/*", offset, bytes.len() as u64 - 1),
        );
    }

    let resp = match req.send() {
        Ok(r) => r,
        Err(_) => {
            return Ok(PutOutcome::NeedsRetry(session_uri.to_string(), offset));
        }
    };

    let status = resp.status();
    if status == reqwest::StatusCode::UNAUTHORIZED {
        return Ok(PutOutcome::AuthRequired);
    }
    if status == reqwest::StatusCode::OK || status == reqwest::StatusCode::CREATED {
        let body = resp.text().unwrap_or_default();
        let parsed: VideoInsertResponse =
            serde_json::from_str(&body).map_err(|_| PutOutcome::Failed(SocialError::UploadFailed))?;
        let id = parsed.id.ok_or(PutOutcome::Failed(SocialError::UploadFailed))?;
        return Ok(PutOutcome::Uploaded(id));
    }
    // HTTP 308 "Resume Incomplete" (YouTube, eksik yuklemeyi iletir).
    // reqwest'teki eşdeğer sabit PERMANENT_REDIRECT'tir (308).
    if status == reqwest::StatusCode::PERMANENT_REDIRECT {
        let range = resp
            .headers()
            .get("Range")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let resume_from = parse_range_end(&range).map(|e| e + 1).unwrap_or(offset);
        if resume_from >= bytes.len() as u64 {
            return Ok(PutOutcome::Failed(SocialError::UploadFailed));
        }
        return Ok(PutOutcome::NeedsRetry(session_uri.to_string(), resume_from));
    }

    Ok(PutOutcome::Failed(SocialError::UploadFailed))
}

fn parse_range_end(range: &str) -> Option<u64> {
    let after_equal = range.split('=').nth(1)?.to_string();
    let end_part = after_equal.split('-').nth(1)?;
    end_part.split('/').next()?.trim().parse().ok()
}

fn probe_session(
    client: &reqwest::blocking::Client,
    session_uri: &str,
) -> Result<Option<String>, SocialError> {
    let resp = match client.get(session_uri).send() {
        Ok(r) => r,
        Err(_) => return Err(SocialError::UploadFailed),
    };
    if resp.status() == reqwest::StatusCode::OK
        || resp.status() == reqwest::StatusCode::CREATED
    {
        let body = resp.text().unwrap_or_default();
        let parsed: VideoInsertResponse =
            serde_json::from_str(&body).map_err(|_| SocialError::UploadFailed)?;
        return Ok(parsed.id);
    }
    Ok(None)
}

fn perform_upload(
    client: &reqwest::blocking::Client,
    access_token: &str,
    title: &str,
    description: &str,
    privacy: PrivacyStatus,
    bytes: &[u8],
    mime_type: &str,
) -> Result<String, PutOutcome> {
    let session_uri =
        init_resumable_session(client, access_token, title, description, privacy, mime_type)?;

    let mut offset = 0u64;
    let mut session = session_uri;
    for _ in 0..6 {
        match upload_to_session(client, &session, bytes, mime_type, offset)? {
            PutOutcome::Uploaded(id) => return Ok(id),
            PutOutcome::AuthRequired => return Err(PutOutcome::AuthRequired),
            PutOutcome::NeedsRetry(uri, new_offset) => {
                session = uri;
                offset = new_offset;
            }
            PutOutcome::Failed(e) => return Err(PutOutcome::Failed(e)),
        }
    }
    Err(PutOutcome::Failed(SocialError::UploadFailed))
}

/// Bir bağlantıya ait videoyu yükler.
pub fn upload_video(
    app: &AppHandle,
    connection_id: &str,
    video_path: &str,
    title: &str,
    description: &str,
    privacy: PrivacyStatus,
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
    let mime_type = video_mime_type(video_path);

    let client = http_client()?;
    let access_token = obtain_access_token(app, &record)?;

    let mut attempt =
        perform_upload(&client, &access_token, title, description, privacy, &bytes, &mime_type);

    if matches!(attempt, Err(PutOutcome::AuthRequired)) {
        let client_id = youtube_client_id().ok_or(SocialError::YoutubeNotConfigured)?;
        let refresh_token = credential_store::get_token(
            &record.platform_id,
            &record.connection_id,
            TokenType::RefreshToken,
        )
        .map_err(|_| SocialError::CredentialStoreError)?
        .ok_or(SocialError::TokenRefreshFailed)?;

        let tokens = refresh_access_token(client_id, &refresh_token).map_err(|_| {
            update_status(app, &record.connection_id, ConnectionStatus::TokenExpired);
            SocialError::TokenRefreshFailed
        })?;

        if credential_store::store_token(
            &record.platform_id,
            &record.connection_id,
            TokenType::AccessToken,
            &tokens.access_token,
        )
        .is_err()
        {
            return Err(SocialError::CredentialStoreError);
        }
        attempt = perform_upload(
            &client,
            &tokens.access_token,
            title,
            description,
            privacy,
            &bytes,
            &mime_type,
        );
    }

    match attempt {
        Ok(id) => Ok(id),
        Err(PutOutcome::Uploaded(id)) => Ok(id),
        Err(PutOutcome::AuthRequired) => {
            update_status(app, &record.connection_id, ConnectionStatus::TokenExpired);
            Err(SocialError::TokenExpired)
        }
        Err(PutOutcome::Failed(e)) => Err(e),
        Err(PutOutcome::NeedsRetry(uri, _)) => {
            if let Some(id) = probe_session(&client, &uri)? {
                return Ok(id);
            }
            Err(SocialError::UploadFailed)
        }
    }
}

/// Uzantıya göre bir video MIME türü döner (resumable yükleme için).
fn video_mime_type(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".mov") || lower.ends_with(".m4v") {
        "video/quicktime"
    } else if lower.ends_with(".avi") {
        "video/x-msvideo"
    } else if lower.ends_with(".mkv") {
        "video/x-matroska"
    } else if lower.ends_with(".3gp") {
        "video/3gpp"
    } else if lower.ends_with(".ogv") {
        "video/ogg"
    } else if lower.ends_with(".mpg") || lower.ends_with(".mpeg") {
        "video/mpeg"
    } else {
        "video/mp4"
    }
}

// ---- Serde yanıt yapıları ----

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChannelResponse {
    items: Option<Vec<ChannelItem>>,
}

#[derive(serde::Deserialize)]
struct ChannelItem {
    id: Option<String>,
    snippet: Option<ChannelSnippet>,
}

#[derive(serde::Deserialize)]
struct ChannelSnippet {
    title: Option<String>,
}

#[derive(serde::Deserialize)]
struct VideoInsertResponse {
    id: Option<String>,
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
    fn code_verifier_len_and_charset() {
        let v = generate_code_verifier().unwrap();
        assert!(v.len() >= 43 && v.len() <= 128);
        assert!(v
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' || c == '~'));
    }

    #[test]
    fn code_challenge_is_stable_for_same_verifier() {
        let v = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let a = compute_code_challenge(v).unwrap();
        let b = compute_code_challenge(v).unwrap();
        assert_eq!(a, b);
        assert_eq!(a, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
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
    fn privacy_parse_accepts_controlled_values() {
        assert_eq!(PrivacyStatus::parse("public"), Some(PrivacyStatus::Public));
        assert_eq!(PrivacyStatus::parse("unlisted"), Some(PrivacyStatus::Unlisted));
        assert_eq!(PrivacyStatus::parse("private"), Some(PrivacyStatus::Private));
        assert_eq!(PrivacyStatus::parse("PUBLIC"), Some(PrivacyStatus::Public));
        assert_eq!(PrivacyStatus::parse(""), None);
        assert_eq!(PrivacyStatus::parse("gizli"), None);
    }

    #[test]
    fn privacy_as_str_is_controlled() {
        assert_eq!(PrivacyStatus::Public.as_str(), "public");
        assert_eq!(PrivacyStatus::Unlisted.as_str(), "unlisted");
        assert_eq!(PrivacyStatus::Private.as_str(), "private");
    }

    #[test]
    fn parse_range_end_extracts_end_byte() {
        assert_eq!(parse_range_end("bytes=0-12345"), Some(12345));
        assert_eq!(parse_range_end("bytes=100-200/1000"), Some(200));
        assert_eq!(parse_range_end("bytes=0-100"), Some(100));
    }

    #[test]
    fn parse_range_end_handles_malformed() {
        assert_eq!(parse_range_end(""), None);
        assert_eq!(parse_range_end("bytes="), None);
        assert_eq!(parse_range_end("bytes=-5"), None);
    }
}
