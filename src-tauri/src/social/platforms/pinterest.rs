//! Gerçek Pinterest API v5 entegrasyonu.
//!
//! OAuth: Pinterest'in masaüstü için uyumlu OAuth 2.0 Authorization Code akışı
//! kullanılır:
//! - Sistem tarayıcısını açar,
//! - `127.0.0.1` üzerinde dinamik loopback callback dinler (resmî dokümandaki
//!   `redirect_uri=http://localhost/` örneğiyle uyumlu),
//! - Kriptografik olarak güvenli `state` üretir,
//! - Kimlik doğrulama, Pinterest'in resmî token endpoint'ine HTTP Basic
//!   (`client_id:client_secret`, base64) ile yapılır.
//!
//! Client Secret gerekir; ancak asla kaynağa gömülmez. Kullanıcı, Pinterest
//! Developer Portal'da oluşturduğu Pinterest uygulamasının Client ID ve Client
//! Secret değerlerini Ayarlar ekranında girer; ikisi de güvenli depoda
//! (Windows Credential Manager) saklanır ve yalnız Rust tarafında token
//! alışverişi sırasında kullanılır.
//!
//! İzinler (yalnız gereken en dar set):
//! - `boards:read`, `boards:write`: yayın yapılacak panoları / pano yönetimini
//!   görmek ve kullanmak.
//! - `pins:read`, `pins:write`: pin oluşturmak (yayın çekirdeği).
//! Reklam/catalog/billing ile ilgili izinler asla istenmez.
//!
//! Bağlantı modeli: Pinterest yayını bir panoya (board) yapılır. Bağlantı
//! akışında kullanıcının tüm panoları keşfedilir ve her pano ayrı bir hedef
//! bağlantı olarak eklenir (dış hesap kimliği = pano id). Pano bulunamazsa
//! kontrollü `pinterest_board_not_found` döner; sahte hedef üretilmez.
//!
//! Yayınlar güncel resmî v5 API'lerle yapılır:
//! - Görsel: `image_base64` media source ile yerel JPG/PNG doğrudan base64
//!   yüklenir; herkese açık `image_url` gerekmez.
//! - Çoklu görsel (carousel): `multiple_image_base64` (resmî sınır 2-5 öğe).
//! - Video: `/v5/media` ile yükleme kaydı → imzalı S3 `upload_url`'e multipart
//!   dosya yüklemesi → `/v5/media/{media_id}` durum yoklaması (`succeeded`)
//!   → `video_id` media source ile pin oluşturma.
//!
//! Zamanlanmış paylaşım resmî API'de yoktur; uygulama zamanlayıcısı istemci
//! tarafındadır (bu modül yalnız anlık yayın yapar).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use rand::RngCore;
use reqwest::blocking::multipart::{Form, Part};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

use super::super::credential_store;
use super::super::media_validation;
use super::super::metadata_store;
use super::super::models::{
    ConnectionRecord, ConnectionStatus, SocialAccountConnection, SocialError, TokenType,
};
use super::meta::MediaKind;

/// Pinterest platform kimliği (katalogdaki değer).
pub const PLATFORM_ID: &str = "pinterest";

/// Pinterest OAuth yetkilendirme uç adresi (resmî v5).
const AUTHORIZE_ENDPOINT: &str = "https://www.pinterest.com/oauth/";
/// Pinterest OAuth token endpoint (HTTP POST, Basic auth).
const TOKEN_ENDPOINT: &str = "https://api.pinterest.com/v5/oauth/token";
/// Pinterest v5 API kök adresi.
pub const API_BASE: &str = "https://api.pinterest.com/v5";

/// OAuth callback'inin kaç saniye bekleneceği.
const OAUTH_TIMEOUT_SECS: u64 = 300;

/// Pano listelemede sayfa boyutu (resmî üst sınır 250).
const BOARD_PAGE_SIZE: u32 = 100;

/// Video işleme sonrası durum (media/get) yoklama sayısı ve aralığı.
const MEDIA_POLL_ATTEMPTS: u32 = 60;
const MEDIA_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Pinterest OAuth izinleri (yalnız gereken en dar set).
/// Yayın çekirdeği pin + pano; reklam/catalog/billing izni istenmez;
/// yalnız okuma amaçlı izinler yayın için zorunlu değildir.
pub const SCOPES: &str = "boards:read boards:write pins:read pins:write";

/// Pinterest Client ID, derleme zamanında (varsa) gömülür. Gizli bilgi değildir.
pub fn pinterest_client_id() -> Option<&'static str> {
    option_env!("ES_OPS_PINTEREST_CLIENT_ID")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

// ---- Güvenli rastgele üretim (state) ----

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
    Ok(url_safe_no_pad_encode(&bytes))
}

fn url_safe_no_pad_encode(bytes: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    URL_SAFE_NO_PAD.encode(bytes)
}

// ---- Loopback callback ----

/// `127.0.0.1` üzerinde dinamik (serbest) bir portta dinleyici açar.
fn bind_loopback() -> Result<TcpListener, SocialError> {
    TcpListener::bind(("127.0.0.1", 0)).map_err(|_| SocialError::OauthTimeout)
}

/// Loopback gelen isteğindeki `code` ve `state` değerlerini ayıklar.
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

/// Ortak Pinterest HTTP istemcisi. Tek bloklama istemcisidir.
fn http_client() -> Result<reqwest::blocking::Client, SocialError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|_| SocialError::ApiError)
}

/// Tarayıcıyı resmî Tauri shell mekanizmasıyla açar.
fn open_browser(app: &AppHandle, url: &str) -> Result<(), SocialError> {
    app.shell()
        .open(url, None)
        .map_err(|_| SocialError::OperationFailed)
}

/// Pinterest token endpoint'in HTTP Basic kimlik başlığı değeri.
/// `base64(client_id + ":" + client_secret)`. Secret asla URL/log'a yazılmaz.
fn basic_auth_header(client_id: &str, client_secret: &str) -> String {
    format!("Basic {}", STANDARD.encode(format!("{client_id}:{client_secret}")))
}

// ---- Auth URL ----

/// Pinterest yetkilendirme URL'sini oluşturur (Authorization Code).
/// `client_secret` HTTP Basic başlıkta gittiği için URL'de asla bulunmaz.
pub fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
) -> String {
    let scope_encoded = scope.replace(' ', "%20");
    format!(
        "{AUTHORIZE_ENDPOINT}?client_id={client_id}&redirect_uri={redirect_uri}&response_type=code&scope={scope_encoded}&state={state}"
    )
}

// ---- Token exchange ----

struct TokenSet {
    access_token: String,
    refresh_token: Option<String>,
}

/// Yetkilendirme kodunu Pinterest token endpoint'inde HTTP Basic ile değiştirir.
///
/// Pinterest resmî akışı: `client_id` + `client_secret` Basic auth ile verilir;
/// form'da `grant_type=authorization_code`, `code`, `redirect_uri` gönderilir.
fn exchange_code(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<TokenSet, SocialError> {
    let client = http_client()?;
    let params: Vec<(&str, &str)> = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
    ];
    let resp = client
        .post(TOKEN_ENDPOINT)
        .header("Authorization", basic_auth_header(client_id, client_secret))
        .form(&params)
        .send()
        .map_err(|_| SocialError::OauthExchangeFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        let lower = body.to_lowercase();
        if lower.contains("invalid_scope")
            || lower.contains("access_denied")
            || lower.contains("invalid_grant")
        {
            return Err(SocialError::PermissionDenied);
        }
        return Err(SocialError::OauthExchangeFailed);
    }

    #[derive(serde::Deserialize)]
    struct TokenResp {
        access_token: Option<String>,
        refresh_token: Option<String>,
    }
    let parsed: TokenResp =
        serde_json::from_str(&body).map_err(|_| SocialError::OauthExchangeFailed)?;
    let access_token = parsed.access_token.filter(|t| !t.is_empty());
    access_token
        .map(|t| TokenSet {
            access_token: t,
            refresh_token: parsed.refresh_token.filter(|t| !t.is_empty()),
        })
        .ok_or(SocialError::OauthExchangeFailed)
}

/// Süresi geçmiş access tokenını Pinterest refresh token ile yeniler.
///
/// Pinterest'in "continuous refresh token" modeliyle (yenilenebilir) uyumludur:
/// `grant_type=refresh_token` gönderilir; yanıt_da yeni `access_token` döner.
fn refresh_access_token(
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
) -> Result<TokenSet, SocialError> {
    if refresh_token.trim().is_empty() {
        return Err(SocialError::TokenRefreshFailed);
    }
    let client = http_client()?;
    let params: Vec<(&str, &str)> = vec![
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
    ];
    let resp = client
        .post(TOKEN_ENDPOINT)
        .header("Authorization", basic_auth_header(client_id, client_secret))
        .form(&params)
        .send()
        .map_err(|_| SocialError::TokenRefreshFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(SocialError::TokenRefreshFailed);
    }

    #[derive(serde::Deserialize)]
    struct TokenResp {
        access_token: Option<String>,
        refresh_token: Option<String>,
    }
    let parsed: TokenResp =
        serde_json::from_str(&body).map_err(|_| SocialError::TokenRefreshFailed)?;
    let access_token = parsed.access_token.filter(|t| !t.is_empty());
    let new_refresh = parsed.refresh_token.filter(|t| !t.is_empty());
    access_token
        .map(|t| TokenSet {
            access_token: t,
            refresh_token: new_refresh.or(Some(refresh_token.to_string())),
        })
        .ok_or(SocialError::TokenRefreshFailed)
}

// ---- Kullanıcı kimliği ----

struct PuserIdentity {
    /// Pinterest kullanıcı adı (görünen ad için).
    username: String,
}

/// `/v5/user_account` ile token sahibinin kullanıcı bilgilerini alır.
fn fetch_user_identity(access_token: &str) -> Result<PuserIdentity, SocialError> {
    let client = http_client()?;
    let resp = client
        .get(format!("{API_BASE}/user_account"))
        .bearer_auth(access_token)
        .send()
        .map_err(|_| SocialError::PinterestIdentityLookupFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            return Err(SocialError::PermissionDenied);
        }
        return Err(SocialError::PinterestIdentityLookupFailed);
    }

    #[derive(serde::Deserialize)]
    struct UserAccountResponse {
        username: Option<String>,
    }
    let parsed: UserAccountResponse =
        serde_json::from_str(&body).map_err(|_| SocialError::PinterestIdentityLookupFailed)?;
    Ok(PuserIdentity {
        username: parsed.username.unwrap_or_default(),
    })
}

// ---- Pano (board) keşfi ----

struct BoardTarget {
    board_id: String,
    display_name: String,
}

/// `/v5/boards` ile hesabın sahip olduğu tüm panoları (bölümleriyle tek tek)
/// listeler; sürüm yuması için `bookmark` sayfalnır.
fn fetch_boards(access_token: &str) -> Result<Vec<BoardTarget>, SocialError> {
    let client = http_client()?;
    let mut boards = Vec::new();
    let mut bookmark: Option<String> = None;

    loop {
        let mut url = format!("{API_BASE}/boards?page_size={BOARD_PAGE_SIZE}");
        if let Some(bm) = &bookmark {
            url.push_str("&bookmark=");
            url.push_str(&url_encode_query(bm));
        }

        let resp = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .map_err(|_| SocialError::PinterestBoardNotFound)?;
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        if !status.is_success() {
            if status == reqwest::StatusCode::UNAUTHORIZED {
                return Err(SocialError::TokenExpired);
            }
            if status == reqwest::StatusCode::FORBIDDEN {
                return Err(SocialError::PermissionDenied);
            }
            return Err(SocialError::PinterestBoardNotFound);
        }

        #[derive(serde::Deserialize)]
        struct BoardsResponse {
            items: Option<Vec<BoardItem>>,
            bookmark: Option<String>,
        }
        #[derive(serde::Deserialize)]
        struct BoardItem {
            id: Option<String>,
            name: Option<String>,
        }

        let parsed: BoardsResponse =
            serde_json::from_str(&body).map_err(|_| SocialError::PinterestBoardNotFound)?;

        for item in parsed.items.unwrap_or_default() {
            let board_id = item.id.filter(|s| !s.is_empty());
            if let Some(board_id) = board_id {
                let name = item.name.filter(|s| !s.is_empty()).unwrap_or_default();
                boards.push(BoardTarget {
                    display_name: name,
                    board_id,
                });
            }
        }

        let next = parsed.bookmark.filter(|s| !s.is_empty());
        match next {
            Some(n) => bookmark = Some(n),
            None => break,
        }
    }

    Ok(boards)
}

/// Sorgu değerleri için basit percent-encoding (bookmark vb.).
fn url_encode_query(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ---- Yapılandırma (Client ID + Client Secret) ----

/// Pinterest uygulama yapılandırması için ortak (bağlantıya özgü olmayan) anahtarı.
const PINTEREST_CONFIG_CONN: &str = "_pinterest_app_config";

/// Pinterest Client ID'yi güvenli depoya yazar (client ID gizli bilgi değildir).
pub fn store_client_id(client_id: &str) -> Result<(), SocialError> {
    if client_id.trim().is_empty() {
        return Err(SocialError::PinterestNotConfigured);
    }
    credential_store::store_token(
        PLATFORM_ID,
        PINTEREST_CONFIG_CONN,
        TokenType::RefreshToken,
        client_id.trim(),
    )
}

/// Pinterest Client Secret'ı güvenli depoya yazar (ham secret JS'ye dönmez).
pub fn store_client_secret(client_secret: &str) -> Result<(), SocialError> {
    if client_secret.trim().is_empty() {
        return Err(SocialError::PinterestNotConfigured);
    }
    credential_store::store_token(
        PLATFORM_ID,
        PINTEREST_CONFIG_CONN,
        TokenType::AccessToken,
        client_secret.trim(),
    )
}

/// Güvenli depodan Pinterest Client ID'yi okur.
fn read_client_id() -> Result<Option<String>, SocialError> {
    credential_store::get_token(PLATFORM_ID, PINTEREST_CONFIG_CONN, TokenType::RefreshToken)
}

/// Güvenli depodan Pinterest Client Secret'ı okur.
fn read_client_secret() -> Result<Option<String>, SocialError> {
    credential_store::get_token(PLATFORM_ID, PINTEREST_CONFIG_CONN, TokenType::AccessToken)
}

/// Kullanım sırasında çözülecek Client ID. Önce derleme zamanı
/// `ES_OPS_PINTEREST_CLIENT_ID`, varsa onu, yoksa güvenli depodaki kaydı kullanır.
pub fn resolved_client_id() -> Option<String> {
    pinterest_client_id()
        .map(|s| s.to_string())
        .or_else(|| read_client_id().ok().flatten())
}

/// Kullanım sırasında çözülecek Client Secret. Yalnız güvenli depodan okunur.
pub fn resolved_client_secret() -> Option<String> {
    read_client_secret().ok().flatten()
}

/// Pinterest Client ID / Client Secret güvenli depoda yapılandırılmış mı?
pub fn config_status() -> Result<(bool, bool), SocialError> {
    let has_id = resolved_client_id().is_some();
    let has_secret = read_client_secret()?.is_some();
    Ok((has_id, has_secret))
}

/// Güvenli depodaki Pinterest uygulama yapılandırmasını temizler.
pub fn clear_config() -> Result<(), SocialError> {
    credential_store::delete_all_tokens(PLATFORM_ID, PINTEREST_CONFIG_CONN)
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

fn generate_connection_id() -> Result<String, SocialError> {
    let bytes = random_bytes(16)?;
    Ok(format!("pinterest_{}", url_safe_no_pad_encode(&bytes)))
}

/// Bir pano hedefi için bağlantı kaydı kurar.
///
/// Aynı pano (aynı board id) ikinci kez eklenmez. Token başarıyla yazılmadan
/// `connected` yapılmaz; metadata yazımı başarısız olursa yarım token temizlenir.
fn save_board_connection(
    app: &AppHandle,
    board_id: &str,
    display_name: &str,
    tokens: &TokenSet,
) -> Result<SocialAccountConnection, SocialError> {
    let dir = data_dir(app)?;
    let records = metadata_store::list_connections(&dir)?;
    let existing = records
        .iter()
        .find(|r| r.platform_id == PLATFORM_ID && r.external_account_id == board_id);

    let connection_id = match existing {
        Some(r) => r.connection_id.clone(),
        None => generate_connection_id()?,
    };

    if credential_store::store_token(
        PLATFORM_ID,
        &connection_id,
        TokenType::AccessToken,
        &tokens.access_token,
    )
    .is_err()
    {
        return Err(SocialError::CredentialStoreError);
    }
    if let Some(rt) = &tokens.refresh_token {
        if credential_store::store_token(PLATFORM_ID, &connection_id, TokenType::RefreshToken, rt)
            .is_err()
        {
            let _ = credential_store::delete_all_tokens(PLATFORM_ID, &connection_id);
            return Err(SocialError::CredentialStoreError);
        }
    }

    let record = ConnectionRecord {
        connection_id: connection_id.clone(),
        platform_id: PLATFORM_ID.to_string(),
        external_account_id: board_id.to_string(),
        account_display_name: display_name.to_string(),
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
        external_account_id: board_id.to_string(),
        account_display_name: display_name.to_string(),
        connection_status: ConnectionStatus::Connected,
        token_exists: true,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    })
}

/// Pinterest'e gerçek OAuth akışıyla bağlanır.
///
/// Akış:
/// 1. Client ID + Client Secret çözülür (derleme zamanı veya güvenli depo).
///    İkisinden biri yoksa `pinterest_not_configured` döner; OAuth başlatılmaz.
/// 2. Loopback listener açılır, `state` üretilir ve resmî Pinterest
///    yetkilendirme sayfası sistem tarayıcısında açılır.
/// 3. Callback'ten kod ve state alınır; state eşleşmesi doğrulanır.
/// 4. Kod, HTTP Basic (client_id:client_secret) ile token'a değiştirilir.
/// 5. Kullanıcı kimliği alınır; yapılabilecek tüm panolar listelenir ve her
///    biri ayrı bir hedef bağlantı olarak eklenir.
///
/// Pazarli pano yoksa `pinterest_board_not_found` döner; sahte hedef üretilmez.
pub fn connect(app: &AppHandle) -> Result<Vec<SocialAccountConnection>, SocialError> {
    let client_id = resolved_client_id().ok_or(SocialError::PinterestNotConfigured)?;
    let client_secret = resolved_client_secret().ok_or(SocialError::PinterestNotConfigured)?;

    let listener = bind_loopback()?;
    let port = listener
        .local_addr()
        .map_err(|_| SocialError::OauthTimeout)?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/");
    let state = generate_state()?;

    let auth_url = build_authorize_url(&client_id, &redirect_uri, SCOPES, &state);
    open_browser(app, &auth_url)?;

    let (code, callback_state) = wait_for_callback(&listener)?;
    if callback_state != state {
        return Err(SocialError::OauthStateMismatch);
    }

    let tokens = exchange_code(&client_id, &client_secret, &redirect_uri, &code)?;
    let identity = fetch_user_identity(&tokens.access_token)?;
    let boards = fetch_boards(&tokens.access_token)?;

    if boards.is_empty() {
        return Err(SocialError::PinterestBoardNotFound);
    }

    let mut connections = Vec::new();
    for board in boards {
        let display = if board.display_name.trim().is_empty() {
            format!("Pinterest ({})", board.board_id)
        } else if identity.username.trim().is_empty() {
            board.display_name.clone()
        } else {
            format!("{} / {}", identity.username, board.display_name)
        };
        connections.push(save_board_connection(app, &board.board_id, &display, &tokens)?);
    }

    Ok(connections)
}

// ---- Yayın ----

/// Pinterest yayınını çağırmak için hazırlanan kontrollü yayın girdisi.
pub struct PinterestPostInput {
    pub connection_id: String,
    pub message: String,
    pub title: String,
    /// Teşhis edilmiş içerik türü (kontrollü).
    pub media_kind: Option<MediaKind>,
    /// Yerel görsel/video dosya yolları (video pin'i için ilk = video,
    /// isteğe bağlı ikinci = kapak görseli).
    pub media_files: Vec<String>,
}

/// Bağlantı kaydından hedef pano id'sini ve tokenlar çözer.
fn resolve_board_and_tokens(
    app: &AppHandle,
    connection_id: &str,
) -> Result<(String, String, Option<String>), SocialError> {
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

    // Hedef, yalnızca geometrik kontrol edilen bir pano id olmalı.
    let board_id = record.external_account_id.clone();
    if board_id.is_empty() || !board_id.chars().all(|c| c.is_ascii_digit()) {
        return Err(SocialError::InvalidConnection);
    }

    let access_token = credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::AccessToken,
    )
    .map_err(|_| SocialError::CredentialStoreError)?
    .filter(|t| !t.is_empty())
    .ok_or(SocialError::TokenMissing)?;

    let refresh_token = credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::RefreshToken,
    )
    .map_err(|_| SocialError::CredentialStoreError)?
    .filter(|t| !t.is_empty());

    Ok((board_id, access_token, refresh_token))
}

/// Pinterest'e gerçek v5 API ile pin oluşturur ve gerçek pin id döndürür.
///
/// - Görsel: `image_base64` (yerel JPG/PNG doğrudan, URL gerekmez).
/// - Çoklu görsel: `multiple_image_base64` (2-5 öğe, resmî sınır).
/// - Video: `/v5/media` kaydı → imzalı S3'e multipart yükleme → durum yoklaması
///   → `video_id` ile pin.
/// - Metin yalnız: Pinterest pin görsel/video ister; `unsupported_post_type` döner.
///
/// Erişim token'ı geçersizse (`token_expired`) bir kez refresh token denemiyle
/// yenilenip yayın tekrar denenir. Başarısız yayın başarılı kaydedilmez.
pub fn publish(
    app: &AppHandle,
    input: &PinterestPostInput,
) -> Result<String, SocialError> {
    let (board_id, access_token, refresh_token) =
        resolve_board_and_tokens(app, &input.connection_id)?;
    let client_secret = resolved_client_secret().ok_or(SocialError::PinterestNotConfigured)?;

    let mut token = access_token;
    let mut refreshed_once = false;
    loop {
        let result = publish_once(&token, &board_id, input);
        match result {
            Err(SocialError::TokenExpired) if !refreshed_once => {
                let rt = refresh_token
                    .clone()
                    .ok_or(SocialError::TokenRefreshFailed)?;
                let new_tokens = refresh_access_token(
                    &resolved_client_id().unwrap_or_default(),
                    &client_secret,
                    &rt,
                )?;
                token = new_tokens.access_token.clone();
                // Yeni token'ı bu bağlantı kaydına geri yaz (kalıcı).
                let dir = data_dir(app)?;
                let record =
                    metadata_store::get_connection(&dir, &input.connection_id)?;
                if let Some(rec) = record {
                    let _ = credential_store::store_token(
                        PLATFORM_ID,
                        &rec.connection_id,
                        TokenType::AccessToken,
                        &new_tokens.access_token,
                    );
                    if let Some(rt) = &new_tokens.refresh_token {
                        let _ = credential_store::store_token(
                            PLATFORM_ID,
                            &rec.connection_id,
                            TokenType::RefreshToken,
                            rt,
                        );
                    }
                }
                refreshed_once = true;
            }
            other => return other,
        }
    }
}

fn publish_once(
    token: &str,
    board_id: &str,
    input: &PinterestPostInput,
) -> Result<String, SocialError> {
    let media = input.media_kind.unwrap_or(MediaKind::Text);
    match media {
        MediaKind::Text => Err(SocialError::UnsupportedPostType),
        MediaKind::Photo => publish_image(token, board_id, input),
        MediaKind::Carousel => publish_carousel(token, board_id, input),
        MediaKind::Video => publish_video(token, board_id, input),
    }
}

/// Görsel dosyasını base64 ile doğrudan pin'e çevirir.
fn publish_image(
    token: &str,
    board_id: &str,
    input: &PinterestPostInput,
) -> Result<String, SocialError> {
    let path = input.media_files.first().ok_or(SocialError::InvalidMediaFile)?;
    let (content_type, data) = image_base64(path)?;
    let media_source = serde_json::json!({
        "source_type": "image_base64",
        "content_type": content_type,
        "data": data
    });
    create_pin(token, board_id, input, media_source)
}

/// 2-5 arası görseli `multiple_image_base64` ile çoklu görsel pin'i oluşturur.
fn publish_carousel(
    token: &str,
    board_id: &str,
    input: &PinterestPostInput,
) -> Result<String, SocialError> {
    let paths: Vec<&String> = input.media_files.iter().collect();
    if paths.len() < 2 || paths.len() > 5 {
        return Err(SocialError::InvalidMediaFile);
    }
    let mut items = Vec::with_capacity(paths.len());
    for path in paths {
        let (content_type, data) = image_base64(path)?;
        items.push(serde_json::json!({
            "content_type": content_type,
            "data": data
        }));
    }
    let media_source = serde_json::json!({
        "source_type": "multiple_image_base64",
        "items": items
    });
    create_pin(token, board_id, input, media_source)
}

/// Video'yu resmî media upload akışıyla yükleyip video pin'i oluşturur.
fn publish_video(
    token: &str,
    board_id: &str,
    input: &PinterestPostInput,
) -> Result<String, SocialError> {
    let video_path = input.media_files.first().ok_or(SocialError::InvalidMediaFile)?;
    media_validation::verify_video_file(video_path)?;

    let client = http_client()?;
    let media_id = upload_video(&client, token, video_path)?;

    // İsteğe bağlı kapak görseli: ikinci dosya görselse base64 kapak kullanılır
    // (herkese açık kapak URL'si gerekmez).
    let mut media_source = serde_json::json!({
        "source_type": "video_id",
        "media_id": media_id
    });
    if let Some(cover_path) = input.media_files.get(1) {
        if let Ok((content_type, data)) = image_base64(cover_path) {
            media_source["cover_image_content_type"] = serde_json::Value::String(content_type);
            media_source["cover_image_data"] = serde_json::Value::String(data);
        }
    }

    create_pin(token, board_id, input, media_source)
}

/// `/v5/media` kaydı → imzalı upload → durum yoklaması → media_id döndürür.
fn upload_video(
    client: &reqwest::blocking::Client,
    token: &str,
    path: &str,
) -> Result<String, SocialError> {
    let filesize = std::fs::metadata(path)
        .map_err(|_| SocialError::FileNotFound)?
        .len();
    if filesize == 0 {
        return Err(SocialError::InvalidVideoFile);
    }

    // 1) Yükleme niyetini kaydet.
    let resp = client
        .post(format!("{API_BASE}/media"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "media_type": "video" }))
        .send()
        .map_err(|_| SocialError::UploadSessionFailed)?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SocialError::TokenExpired);
        }
        return Err(SocialError::UploadSessionFailed);
    }

    #[derive(serde::Deserialize)]
    struct MediaUploadResponse {
        media_id: Option<String>,
        upload_url: Option<String>,
        upload_parameters: Option<serde_json::Map<String, serde_json::Value>>,
    }
    let parsed: MediaUploadResponse =
        serde_json::from_str(&body).map_err(|_| SocialError::UploadSessionFailed)?;
    let media_id = parsed.media_id.filter(|s| !s.is_empty());
    let upload_url = parsed.upload_url.filter(|s| !s.is_empty());
    let upload_parameters = parsed.upload_parameters;
    let (media_id, upload_url, upload_parameters) = match (media_id, upload_url, upload_parameters)
    {
        (Some(m), Some(u), Some(p)) => (m, u, p),
        _ => return Err(SocialError::UploadSessionFailed),
    };

    // 2) Dosyayı imzalı adrese multipart ile yükle (Bearer gerekmez).
    let mut form = Form::new();
    for (key, value) in &upload_parameters {
        if let Some(v) = value.as_str() {
            form = form.text(key.clone(), v.to_string());
        }
    }
    let file_name = std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "video.mp4".to_string());
    let mime_type = guess_video_mime(path);
    let bytes = std::fs::read(path).map_err(|_| SocialError::FileNotFound)?;
    let part = Part::bytes(bytes)
        .file_name(file_name)
        .mime_str(mime_type)
        .map_err(|_| SocialError::UploadFailed)?;
    form = form.part("file", part);

    let up = client
        .post(&upload_url)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::UploadFailed)?;
    if !up.status().is_success() && up.status() != reqwest::StatusCode::NO_CONTENT {
        return Err(SocialError::UploadFailed);
    }

    // 3) İşlem durumunu yokla (`succeeded`).
    wait_media_succeeded(client, token, &media_id)?;

    Ok(media_id)
}

/// Video yükleme/işleme durumunu `/v5/media/{media_id}` ile yoklar.
fn wait_media_succeeded(
    client: &reqwest::blocking::Client,
    token: &str,
    media_id: &str,
) -> Result<(), SocialError> {
    let url = format!("{API_BASE}/media/{media_id}");
    for _ in 0..MEDIA_POLL_ATTEMPTS {
        let resp = match client.get(&url).bearer_auth(token).send() {
            Ok(r) => r,
            Err(_) => return Err(SocialError::MediaProcessingTimeout),
        };
        if !resp.status().is_success() {
            if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                return Err(SocialError::TokenExpired);
            }
            return Err(SocialError::MediaProcessingTimeout);
        }
        let body = resp.text().unwrap_or_default();

        #[derive(serde::Deserialize)]
        struct MediaDetailResponse {
            status: Option<String>,
        }
        match serde_json::from_str::<MediaDetailResponse>(&body) {
            Ok(parsed) => match parsed.status.as_deref() {
                Some("succeeded") => return Ok(()),
                Some("failed") => return Err(SocialError::UploadFailed),
                _ => {}
            },
            Err(_) => return Err(SocialError::MediaProcessingTimeout),
        }
        std::thread::sleep(MEDIA_POLL_INTERVAL);
    }
    Err(SocialError::MediaProcessingTimeout)
}

/// `POST /v5/pins` ile pano'ya pin oluşturur ve gerçek pin id döndürür.
fn create_pin(
    token: &str,
    board_id: &str,
    input: &PinterestPostInput,
    media_source: serde_json::Value,
) -> Result<String, SocialError> {
    let client = http_client()?;
    let mut body = serde_json::Map::new();
    body.insert("board_id".to_string(), serde_json::Value::String(board_id.to_string()));
    let description = input.message.trim();
    if !description.is_empty() {
        body.insert("description".to_string(), serde_json::Value::String(description.to_string()));
    }
    let title = input.title.trim();
    if !title.is_empty() {
        body.insert("title".to_string(), serde_json::Value::String(title.to_string()));
    }
    body.insert("media_source".to_string(), media_source);

    let resp = client
        .post(format!("{API_BASE}/pins"))
        .bearer_auth(token)
        .json(&serde_json::Value::Object(body))
        .send()
        .map_err(|_| SocialError::PublishFailed)?;

    let status = resp.status();
    let resp_body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(map_api_error(status, &resp_body));
    }

    #[derive(serde::Deserialize)]
    struct PinResponse {
        id: Option<String>,
    }
    let parsed: PinResponse =
        serde_json::from_str(&resp_body).map_err(|_| SocialError::PublishFailed)?;
    parsed
        .id
        .filter(|s| !s.is_empty())
        .ok_or(SocialError::PublishFailed)
}

/// Pinterest API hata durumunu kontrollü hata kodlarına eşler.
/// Ham Pinterest hata cevabı kullanıcıya gösterilmez; token sızdırılmaz.
fn map_api_error(status: reqwest::StatusCode, _body: &str) -> SocialError {
    match status {
        reqwest::StatusCode::UNAUTHORIZED => SocialError::TokenExpired,
        reqwest::StatusCode::FORBIDDEN => SocialError::PermissionDenied,
        _ => SocialError::PublishFailed,
    }
}

// ---- Medya yardımcıları ----

/// Yerel görsel dosyayı doğrular ve `(content_type, base64 data)` döndürür.
fn image_base64(path: &str) -> Result<(String, String), SocialError> {
    media_validation::verify_image_or_photo_file(path)?;
    let content_type = image_content_type(path).ok_or(SocialError::UnsupportedContentType)?;
    let bytes = std::fs::read(path).map_err(|_| SocialError::FileNotFound)?;
    Ok((content_type.to_string(), STANDARD.encode(&bytes)))
}

fn image_content_type(path: &str) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".png") {
        Some("image/png")
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        Some("image/jpeg")
    } else {
        None
    }
}

fn guess_video_mime(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with(".mov") {
        "video/quicktime"
    } else if lower.ends_with(".m4v") {
        "video/x-m4v"
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else {
        "video/mp4"
    }
}

// ---- Testler ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_are_minimal_and_writable() {
        // En dar izin seti: pano + pin (oku/yaz). Gereksiz reklam/catalog/billing izni yok.
        assert!(SCOPES.contains("boards:read"));
        assert!(SCOPES.contains("boards:write"));
        assert!(SCOPES.contains("pins:read"));
        assert!(SCOPES.contains("pins:write"));
        assert!(!SCOPES.contains("ads:"));
        assert!(!SCOPES.contains("catalogs:"));
        assert!(!SCOPES.contains("billing:"));
    }

    #[test]
    fn basic_auth_header_is_base64_credentials() {
        // Resmî örnek: client_id=123, client_secret=456 → MTIzOjQ1Ng==
        assert_eq!(basic_auth_header("123", "456"), "Basic MTIzOjQ1Ng==");
    }

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
    fn authorize_url_includes_required_params_without_secret() {
        let url = build_authorize_url(
            "cid",
            "http://127.0.0.1:9999/",
            "boards:read pins:write",
            "st",
        );
        assert!(url.contains("client_id=cid"));
        assert!(url.contains("redirect_uri=http://127.0.0.1:9999/"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=boards:read%20pins:write"));
        assert!(url.contains("state=st"));
        assert!(!url.contains("client_secret"));
        assert!(!url.contains("code_challenge"));
    }

    #[test]
    fn url_encode_query_preserves_safe_chars() {
        assert_eq!(url_encode_query("abc_-."), "abc_-.");
        assert_eq!(url_encode_query("a b=c"), "a%20b%3Dc");
    }

    #[test]
    fn image_content_type_supported_extensions() {
        assert_eq!(image_content_type("a.png"), Some("image/png"));
        assert_eq!(image_content_type("a.jpg"), Some("image/jpeg"));
        assert_eq!(image_content_type("a.jpeg"), Some("image/jpeg"));
        assert_eq!(image_content_type("a.gif"), None);
        assert!(paths_are_2_to_5(vec!["a.png", "b.png"]).unwrap());
        assert!(!paths_are_2_to_5(vec!["a.png"]).unwrap());
        assert!(!paths_are_2_to_5(vec!["1", "2", "3", "4", "5", "6"]).unwrap());
    }

    /// Test için yardımcı: çoklu görsel adeti 2-5 aralığını doğrula.
    fn paths_are_2_to_5(paths: Vec<&str>) -> Result<bool, ()> {
        let ok = paths.len() >= 2 && paths.len() <= 5;
        Ok(ok)
    }

    #[test]
    fn guess_video_mime_maps_common_formats() {
        assert_eq!(guess_video_mime("v.mp4"), "video/mp4");
        assert_eq!(guess_video_mime("v.MOV"), "video/quicktime");
        assert_eq!(guess_video_mime("v.m4v"), "video/x-m4v");
        assert_eq!(guess_video_mime("v.webm"), "video/webm");
    }
}