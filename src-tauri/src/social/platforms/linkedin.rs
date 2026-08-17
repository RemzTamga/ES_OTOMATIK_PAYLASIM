//! Gerçek LinkedIn entegrasyonu.
//!
//! OAuth: LinkedIn'in masaüstü (native) uygulamalar için resmî PKCE akışı
//! (RFC 7636) kullanılır:
//! - Sistem tarayıcısını açar,
//! - `127.0.0.1` üzerinde dinamik loopback callback dinler,
//! - Kriptografik olarak güvenli `state` + S256 `code_challenge` kullanır,
//! - Token değişiminde yalnız `code_verifier` gönderilir.
//!
//! Client Secret KULLANILMAZ ve hiçbir yerde saklanmaz: yapılandırma ekranında
//! secret alanı yoktur, secret kaynağa/binary'ye/loga gömülmez, token
//! değişimine `client_secret` parametresi eklenmez.
//!
//! İzinler (yalnız gereken en dar set):
//! - `openid`: kişisel kimlik (person URN) için (OpenID userinfo).
//! - `profile`, `email`: OpenID zorunlu kapsamları.
//! - `w_member_social`: kişisel profile yayın.
//!   Kurumsal izinler (`w_organization_social`, `rw_organization_admin`) yalnızca
//!   LinkedIn portalda onaylandığında SCOPES sabitine eklenmeli.
//!
//! Şirket sayfası yayını için kabul edilen üye rolleri (resmî LinkedIn rol
//! değerleri): `ADMINISTRATOR`, `CONTENT_ADMIN`, `DIRECT_SPONSORED_CONTENT_POSTER`.
//!
//! Yayınlar güncel resmî API'lerle yapılır (eski UGC/Share API kullanılmaz):
//! - Metin: `POST /rest/posts` (Posts API).
//! - Görsel: Images API (`/rest/images?action=initializeUpload` → binary PUT →
//!   image URN) + `/rest/posts`.
//! - Video: Videos API (`/rest/videos?action=initializeUpload` → 4 MB parçalar
//!   (PUT + ETag) → `finalizeUpload` → `AVAILABLE` yoklaması) + `/rest/posts`.
//!
//! Tüm REST istekleri `X-Restli-Protocol-Version: 2.0.0` ve güncel
//! `Linkedin-Version: {YYYYMM}` başlıklarını taşır.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
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
use super::meta::MediaKind;

/// LinkedIn platform kimliği (katalogdaki değer).
pub const PLATFORM_ID: &str = "linkedin";

/// Güncel LinkedIn API sürümü (`Linkedin-Version` başlığı, YYYYMM biçiminde).
pub const API_VERSION: &str = "202607";

/// OAuth yetkilendirme uç adresi.
const AUTHORIZE_ENDPOINT: &str = "https://www.linkedin.com/oauth/v2/authorization";
/// OAuth token endpoint (HTTP POST, form-urlencoded).
const TOKEN_ENDPOINT: &str = "https://www.linkedin.com/oauth/v2/accessToken";
/// Rest.li tabanlı güncel REST API kök adresi (Posts / Images / Videos).
const REST_BASE: &str = "https://api.linkedin.com/rest";
/// OpenID Connect kullanıcı bilgi uç adresi (kişisel kimlik).
const OPENID_USERINFO_ENDPOINT: &str = "https://api.linkedin.com/v2/userinfo";

/// OAuth callback'inin kaç saniye beklenileceği.
const OAUTH_TIMEOUT_SECS: u64 = 300;

/// Videos API çok parçalı yüklemede parça boyutu (resmî değer: 4 MB).
/// Parça aralıkları yükleme başlatma yanıtındaki `uploadInstructions` ile
/// sunucu tarafından bildirilir; bu sabit resmî sınırın referansıdır.
const VIDEO_PART_SIZE: u64 = 4 * 1024 * 1024;

/// Video `AVAILABLE` durumu için yoklama denemesi (5 sn aralıklarla ~3 dk).
const VIDEO_POLL_ATTEMPTS: u32 = 36;
/// Video yoklama aralığı.
const VIDEO_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// LinkedIn OAuth izinleri (yalnız gereken en dar set).
/// LinkedIn OpenID akışı `openid` ile birlikte `profile email` kapsamlarını
/// zorunlu tutar (openid_insufficient_scope_error).
/// `w_organization_social` ve `rw_organization_admin` yalnızca uygulama
/// LinkedIn portalda onaylandığında eklenmeli; onaysızken `unauthorized_scope_error`
/// hatası verir. Bu nedenle yalnız kişisel yayın izni (`w_member_social`) istenir.
pub const SCOPES: &str = "openid profile email w_member_social";

/// Şirket sayfasına yayın için kabul edilen üye rolleri (resmî değerler).
pub const PAGE_POST_ROLES: [&str; 3] = [
    "ADMINISTRATOR",
    "CONTENT_ADMIN",
    "DIRECT_SPONSORED_CONTENT_POSTER",
];

/// LinkedIn Client ID, derleme zamanında (varsa) güvenli biçimde gömülür.
/// Client ID gizli bilgi değildir (public identifier); Client Secret bu
/// entegrasyonda hiçbir biçimde kullanılmaz.
pub fn linkedin_client_id() -> Option<&'static str> {
    option_env!("ES_OPS_LINKEDIN_CLIENT_ID")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

// ---- Güvenli rastgele üretim (state + PKCE) ----

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

/// PKCE `code_verifier` üretir (RFC 7636: 43-128 karakter, URL-safe).
/// 32 rastgele bayt → 43 karakterlik URL-safe dize.
fn generate_pkce_verifier() -> Result<String, SocialError> {
    let bytes = random_bytes(32)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// PKCE `code_challenge` üretir: `code_challenge = base64url(sha256(verifier))`.
fn pkce_challenge(verifier: &str) -> Result<String, SocialError> {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(verifier.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(digest))
}

// ---- Loopback callback ----

/// `127.0.0.1:8080` üzerinde dinleyici açar. LinkedIn, yönlendirme adresini
/// kayıtlı değerle birebir eşleştirdiğinden sabit port kullanılır; kullanıcının
/// LinkedIn uygulama ayarlarına `http://127.0.0.1:8080/` kaydedilmelidir.
fn bind_loopback() -> Result<TcpListener, SocialError> {
    TcpListener::bind(("127.0.0.1", 8080)).map_err(|_| SocialError::OperationFailed)
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

                let query = path_and_query
                    .split_once('?')
                    .map(|(_, q)| q)
                    .unwrap_or("");

                if query.contains("error=") {
                    let error_detail = query
                        .split('&')
                        .filter(|p| p.starts_with("error"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let response_body = format!(
                        "<html><body><h3>ES OPS - HATA</h3><p>LinkedIn bir hata dondurdu:</p><pre>{}</pre><p>Bu pencereyi kapatabilirsiniz.</p></body></html>",
                        error_detail
                    );
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                    return Err(SocialError::OauthCancelled);
                }

                let response_body =
                    "<html><body><h3>ES OPS</h3><p>Baglanti tamamlandi. Bu pencereyi kapatabilirsiniz.</p></body></html>";
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();

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

/// Ortak LinkedIn HTTP istemcisi. Uygun tek bloklama istemcisidir.
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

// ---- Auth URL ----

/// LinkedIn yetkilendirme URL'sini oluşturur (Native PKCE: S256 challenge).
pub fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    let scope_encoded = scope.replace(' ', "%20");
    format!(
        "{AUTHORIZE_ENDPOINT}?response_type=code&client_id={client_id}&redirect_uri={redirect_uri}&scope={scope_encoded}&state={state}&code_challenge={code_challenge}&code_challenge_method=S256"
    )
}

// ---- Token exchange (secret'sız) ----

use std::sync::Mutex;

/// Son token exchange hatasının detayı (frontend'e gösterilmek üzere).
static LAST_TOKEN_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Son token exchange hata detayını alır ve temizler.
pub fn take_last_token_error() -> Option<String> {
    LAST_TOKEN_ERROR.lock().ok().and_then(|mut d| d.take())
}

struct TokenSet {
    access_token: String,
    refresh_token: Option<String>,
}

/// Yetkilendirme kodunu LinkedIn token endpoint'inde değiştirir.
///
/// Native PKCE akışı: `client_id` + `code_verifier` gönderilir;
/// `client_secret` parametresi asla eklenmez ve hiçbir yerde saklanmaz.
fn exchange_code(
    client_id: &str,
    redirect_uri: &str,
    code: &str,
    code_verifier: &str,
) -> Result<TokenSet, SocialError> {
    let client = http_client()?;
    let params = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", client_id),
        ("redirect_uri", redirect_uri),
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
        let _ = LAST_TOKEN_ERROR.lock().map(|mut d| *d = Some(format!("HTTP {}: {}", status, body)));
        let lower = body.to_lowercase();
        if lower.contains("invalid_scope")
            || lower.contains("access_denied")
            || lower.contains("unauthorized_client")
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

// ---- Kullanıcı kimliği (OpenID userinfo) ----

struct MemberIdentity {
    /// Kişisel profil için person id (`urn:li:person:{id}` olur).
    person_id: String,
    display_name: String,
}

/// Mevcut access_token ile LinkedIn kullanıcı kimliğini alır (`/v2/userinfo`).
fn fetch_member_identity(access_token: &str) -> Result<MemberIdentity, SocialError> {
    let client = http_client()?;
    let resp = client
        .get(OPENID_USERINFO_ENDPOINT)
        .bearer_auth(access_token)
        .send()
        .map_err(|_| SocialError::LinkedinIdentityLookupFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED
            || status == reqwest::StatusCode::FORBIDDEN
        {
            return Err(SocialError::PermissionDenied);
        }
        return Err(SocialError::LinkedinIdentityLookupFailed);
    }

    #[derive(serde::Deserialize)]
    struct UserInfoResponse {
        sub: Option<String>,
        name: Option<String>,
    }
    let parsed: UserInfoResponse =
        serde_json::from_str(&body).map_err(|_| SocialError::LinkedinIdentityLookupFailed)?;
    let person_id = parsed.sub.filter(|s| !s.is_empty());
    person_id
        .map(|id| MemberIdentity {
            display_name: parsed.name.unwrap_or_default(),
            person_id: id,
        })
        .ok_or(SocialError::LinkedinIdentityLookupFailed)
}

// ---- Şirket sayfası keşfi ----

struct OrgTarget {
    /// Yayın hedefi olarak kullanılacak organizasyon URN'si.
    org_urn: String,
    display_name: String,
}

/// Kullanıcının yayın yapabildiği şirket sayfalarını bulur.
///
/// `GET /rest/organizationAcls?q=roleAssignee` sonucu yalnız kabul edilen
/// üye rolleriyle (`ADMINISTRATOR`, `CONTENT_ADMIN`,
/// `DIRECT_SPONSORED_CONTENT_POSTER`) ve `APPROVED` durumuyla filtrelenir.
/// Sayfa adı Organization Lookup ile alınmaya çalışılır; alınamazsa dürüst
/// URN tabanlı görünen ad kullanılır (sahte ad üretilmez).
fn fetch_postable_organizations(
    access_token: &str,
) -> Result<Vec<OrgTarget>, SocialError> {
    let client = http_client()?;
    let url = format!("{REST_BASE}/organizationAcls?q=roleAssignee");

    let resp = rest_get(&client, access_token, &url)?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SocialError::TokenExpired);
        }
        if status == reqwest::StatusCode::FORBIDDEN {
            return Err(SocialError::PermissionDenied);
        }
        return Err(SocialError::ApiError);
    }

    #[derive(serde::Deserialize)]
    struct AclsResponse {
        elements: Option<Vec<AclElement>>,
    }
    #[derive(serde::Deserialize)]
    struct AclElement {
        role: Option<String>,
        state: Option<String>,
        organization: Option<String>,
        #[serde(rename = "organizationTarget")]
        organization_target: Option<String>,
    }

    let parsed: AclsResponse = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_) => return Err(SocialError::ApiError),
    };

    let mut seen = std::collections::HashSet::new();
    let mut orgs = Vec::new();
    for item in parsed.elements.unwrap_or_default() {
        let role = item.role.unwrap_or_default();
        if !PAGE_POST_ROLES.contains(&role.as_str()) {
            continue;
        }
        let state = item.state.unwrap_or_default();
        if state != "APPROVED" {
            continue;
        }
        let org_urn = item
            .organization
            .or(item.organization_target)
            .unwrap_or_default();
        if org_urn.is_empty() || !org_urn.starts_with("urn:li:organization:") {
            continue;
        }
        if !seen.insert(org_urn.clone()) {
            continue;
        }
        orgs.push(OrgTarget {
            display_name: resolve_org_name(&client, access_token, &org_urn),
            org_urn,
        });
    }
    Ok(orgs)
}

/// Organization Lookup ile sayfa adını almaya çalışır; alınamazsa URN tabanlı
/// dürüst görünen ad döner (sahte ad üretilmez, hata sessiz yutulur).
fn resolve_org_name(
    client: &reqwest::blocking::Client,
    access_token: &str,
    org_urn: &str,
) -> String {
    let url = format!("{REST_BASE}/organizations/{}", url_encode_path(org_urn));
    let resp = match rest_get(client, access_token, &url) {
        Ok(r) => r,
        Err(_) => return format!("LinkedIn Sayfasi ({org_urn})"),
    };
    if !resp.status().is_success() {
        return format!("LinkedIn Sayfasi ({org_urn})");
    }
    let body = resp.text().unwrap_or_default();

    #[derive(serde::Deserialize)]
    struct OrgResponse {
        localized_name: Option<String>,
        #[serde(rename = "localizedName")]
        localized_name_alt: Option<String>,
        name: Option<String>,
    }
    let parsed: OrgResponse = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_) => return format!("LinkedIn Sayfasi ({org_urn})"),
    };
    parsed
        .localized_name
        .or(parsed.localized_name_alt)
        .or(parsed.name)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("LinkedIn Sayfasi ({org_urn})"))
}

// ---- Rest.li yardımcıları ----

/// Tüm Rest.li isteklerinde zorunlu ortak başlıkları ekleyerek GET atar.
fn rest_get(
    client: &reqwest::blocking::Client,
    access_token: &str,
    url: &str,
) -> Result<reqwest::blocking::Response, SocialError> {
    client
        .get(url)
        .header("X-Restli-Protocol-Version", "2.0.0")
        .header("Linkedin-Version", API_VERSION)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .map_err(|_| SocialError::ApiError)
}

/// Tüm Rest.li isteklerinde zorunlu ortak başlıkları ekleyerek JSON POST atar.
fn rest_post_json(
    client: &reqwest::blocking::Client,
    access_token: &str,
    url: &str,
    body: serde_json::Value,
) -> Result<reqwest::blocking::Response, SocialError> {
    client
        .post(url)
        .header("X-Restli-Protocol-Version", "2.0.0")
        .header("Linkedin-Version", API_VERSION)
        .header("Authorization", format!("Bearer {access_token}"))
        .json(&body)
        .send()
        .map_err(|_| SocialError::ApiError)
}

/// URL yol parçası için percent-encoding (örn. `urn:li:...` → `urn%3Ali%3A...`).
fn url_encode_path(s: &str) -> String {
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

// ---- Yapılandırma (yalnız Client ID; secret yok) ----

/// LinkedIn uygulama yapılandırması için kullanılan ortak (bağlantıya özgü
/// olmayan) anahtardır. Yalnız Client ID saklanır; Client Secret bu
/// entegrasyonda asla saklanmaz veya kullanılmaz.
const LINKEDIN_CONFIG_CONN: &str = "_linkedin_app_config";

/// LinkedIn Client ID'yi güvenli depoya yazar (Client ID gizli bilgi değildir).
pub fn store_client_id(client_id: &str) -> Result<(), SocialError> {
    if client_id.trim().is_empty() {
        return Err(SocialError::LinkedinNotConfigured);
    }
    credential_store::store_token(
        PLATFORM_ID,
        LINKEDIN_CONFIG_CONN,
        TokenType::RefreshToken,
        client_id.trim(),
    )
}

/// Güvenli depodan LinkedIn Client ID'yi okur.
pub fn read_client_id() -> Result<Option<String>, SocialError> {
    credential_store::get_token(PLATFORM_ID, LINKEDIN_CONFIG_CONN, TokenType::RefreshToken)
}

/// Kullanım sırasında çözülecek Client ID. Önce derleme zamanı
/// `ES_OPS_LINKEDIN_CLIENT_ID`, varsa onu, yoksa güvenli depodaki kullanıcı
/// kaydını kullanır.
pub fn resolved_client_id() -> Option<String> {
    linkedin_client_id()
        .map(|s| s.to_string())
        .or_else(|| read_client_id().ok().flatten())
}

/// Güvenli depodaki LinkedIn yapılandırmasını temizler.
pub fn clear_config() -> Result<(), SocialError> {
    credential_store::delete_all_tokens(PLATFORM_ID, LINKEDIN_CONFIG_CONN)
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
    Ok(format!("linkedin_{}", URL_SAFE_NO_PAD.encode(bytes)))
}

/// Bir hedef (kişisel profil veya şirket sayfası) için bağlantı kaydı kurar.
///
/// Aynı hedef (aynı author URN) ikinci kez eklenmez. Tokenlar başarıyla
/// yazılmadan `connected` yapılmaz; metadata yazımı başarısız olursa yarım
/// token kayıtları temizlenir.
fn save_connection(
    app: &AppHandle,
    author_urn: &str,
    display_name: &str,
    tokens: &TokenSet,
) -> Result<SocialAccountConnection, SocialError> {
    let dir = data_dir(app)?;
    let records = metadata_store::list_connections(&dir)?;
    let existing = records
        .iter()
        .find(|r| r.platform_id == PLATFORM_ID && r.external_account_id == author_urn);

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
        external_account_id: author_urn.to_string(),
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
        external_account_id: author_urn.to_string(),
        account_display_name: display_name.to_string(),
        connection_status: ConnectionStatus::Connected,
        token_exists: true,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    })
}

/// LinkedIn'e gerçek Native PKCE OAuth akışıyla bağlanır.
///
/// Akış:
/// 1. Client ID'yi çözer (derleme zamanı veya güvenli depo). Yoksa
///    `linkedin_not_configured` döner; Client Secret asla istenmez.
/// 2. Loopback listener açılır, `state` + S256 `code_challenge` üretilir ve
///    resmî LinkedIn yetkilendirme sayfası sistem tarayıcısında açılır.
/// 3. Callback'ten kod ve state alınır; state eşleşmesi doğrulanır.
/// 4. Kod, `code_verifier` ile secret'sız değiştirilir.
/// 5. Kişisel profil (OpenID userinfo) her zaman bağlanır; yayın yapılabilir
///    şirket sayfaları keşfedilip her biri ayrı bağlantı olarak eklenir.
///
/// Başarısız adımlarda kontrollü hata kodu döner; sahte bağlantı/token üretilmez.
pub fn connect(app: &AppHandle) -> Result<Vec<SocialAccountConnection>, SocialError> {
    let client_id = resolved_client_id().ok_or(SocialError::LinkedinNotConfigured)?;

    let listener = bind_loopback()?;
    let port = listener
        .local_addr()
        .map_err(|_| SocialError::OauthTimeout)?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/");
    let state = generate_state()?;
    let verifier = generate_pkce_verifier()?;
    let challenge = pkce_challenge(&verifier)?;

    let auth_url = build_authorize_url(&client_id, &redirect_uri, SCOPES, &state, &challenge);
    open_browser(app, &auth_url)?;

    let (code, callback_state) = wait_for_callback(&listener)?;
    if callback_state != state {
        return Err(SocialError::OauthStateMismatch);
    }

    let tokens = exchange_code(&client_id, &redirect_uri, &code, &verifier)?;
    let identity = fetch_member_identity(&tokens.access_token)?;

    let personal_urn = format!("urn:li:person:{}", identity.person_id);
    let mut connections = Vec::new();
    let personal_name = if identity.display_name.trim().is_empty() {
        "LinkedIn Profili".to_string()
    } else {
        identity.display_name.clone()
    };
    connections.push(save_connection(app, &personal_urn, &personal_name, &tokens)?);

    // Şirket sayfası keşfi başarısız olursa kişisel bağlantı geçersiz sayılmaz;
    // sayfalar yalnızca gerçekten keşfedilebildiğinde eklenir (sahte eklenmez).
    match fetch_postable_organizations(&tokens.access_token) {
        Ok(orgs) => {
            for org in orgs {
                let name = if org.display_name.trim().is_empty() {
                    format!("LinkedIn Sayfasi ({})", org.org_urn)
                } else {
                    org.display_name
                };
                connections.push(save_connection(app, &org.org_urn, &name, &tokens)?);
            }
        }
        Err(SocialError::LinkedinOrgNotFound) | Err(SocialError::PermissionDenied) => {}
        Err(_) => {}
    }

    Ok(connections)
}

// ---- Yayın ----

/// LinkedIn yayınını çağırmak için hazırlanan kontrollü yayın girdisi.
/// Mevcut yayın motorunun güvenilir veri modelinden gelen alanlardır;
/// JavaScript'ten serbest metin gelmez.
pub struct LinkedinPostInput {
    pub connection_id: String,
    pub message: String,
    pub title: String,
    /// Teşhis edilmiş içerik türü (kontrollü). Boşsa metin yayını kabul edilir.
    pub media_kind: Option<MediaKind>,
    /// Yerel görsel/video dosya yolları (LinkedIn yalnız tek medya kabul eder).
    pub media_files: Vec<String>,
}

/// Bağlantı kaydından hedef yazar URN'sini ve token'ı çözer.
fn resolve_author_and_token(
    app: &AppHandle,
    connection_id: &str,
) -> Result<(String, String), SocialError> {
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

    // Yazar URN'si kontrollü iki türden biri olmalı: kişisel veya organizasyon.
    let author = record.external_account_id;
    if !author.starts_with("urn:li:person:") && !author.starts_with("urn:li:organization:") {
        return Err(SocialError::InvalidConnection);
    }

    let token = credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::AccessToken,
    )
    .map_err(|_| SocialError::CredentialStoreError)?
    .filter(|t| !t.is_empty())
    .ok_or(SocialError::TokenMissing)?;

    Ok((author, token))
}

/// LinkedIn'e gerçek Posts API ile yayın yapar ve gerçek post URN'sini döndürür.
///
/// - Metin: `POST /rest/posts` (yalnız commentary).
/// - Tek görsel: Images API ile görsel yüklenir, sonra `/rest/posts`.
/// - Video: Videos API ile 4 MB parçalar halinde yüklenir, `AVAILABLE` için
///   yoklanır, sonra `/rest/posts`.
///
/// Yayın başlamadan önce içerik türü doğrulanır. Başarısız yayın başarılı
/// kaydedilmez; gerçek LinkedIn post URN'si döner.
pub fn publish(
    app: &AppHandle,
    input: &LinkedinPostInput,
) -> Result<String, SocialError> {
    let (author, token) = resolve_author_and_token(app, &input.connection_id)?;
    let media = input.media_kind.unwrap_or(MediaKind::Text);

    match media {
        MediaKind::Text => publish_text(&author, &token, input),
        MediaKind::Photo => publish_photo(&author, &token, input),
        MediaKind::Video => publish_video(&author, &token, input),
        MediaKind::Carousel => Err(SocialError::UnsupportedPostType),
    }
}

fn publish_text(
    author: &str,
    token: &str,
    input: &LinkedinPostInput,
) -> Result<String, SocialError> {
    if input.message.trim().is_empty() {
        return Err(SocialError::UnsupportedPostType);
    }
    create_post(author, token, &input.message, None)
}

fn publish_photo(
    author: &str,
    token: &str,
    input: &LinkedinPostInput,
) -> Result<String, SocialError> {
    let path = input.media_files.first().ok_or(SocialError::InvalidMediaFile)?;
    media_validation::verify_image_or_photo_file(path)?;

    let client = http_client()?;
    let image_urn = upload_image(&client, author, token, path)?;
    create_post(author, token, &input.message, Some((image_urn.as_str(), None)))
}

fn publish_video(
    author: &str,
    token: &str,
    input: &LinkedinPostInput,
) -> Result<String, SocialError> {
    let path = input.media_files.first().ok_or(SocialError::InvalidMediaFile)?;
    media_validation::verify_video_file(path)?;

    let client = http_client()?;
    let video_urn = upload_video(&client, author, token, path)?;
    let title = if input.title.trim().is_empty() {
        None
    } else {
        Some(input.title.trim())
    };
    create_post(author, token, &input.message, Some((video_urn.as_str(), title)))
}

/// Posts API ile içerik oluşturur; 201 yanıtındaki `x-restli-id` post
/// URN'sini döndürür. `media` varsa `content.media.id` olarak eklenir
/// (video için isteğe bağlı başlık da eklenir).
fn create_post(
    author: &str,
    token: &str,
    commentary: &str,
    media: Option<(&str, Option<&str>)>,
) -> Result<String, SocialError> {
    if commentary.trim().is_empty() && media.is_none() {
        return Err(SocialError::UnsupportedPostType);
    }

    let mut body = serde_json::json!({
        "author": author,
        "commentary": commentary,
        "visibility": "PUBLIC",
        "distribution": {
            "feedDistribution": "MAIN_FEED",
            "targetEntities": [],
            "thirdPartyDistributionChannels": []
        },
        "lifecycleState": "PUBLISHED",
        "isReshareDisabledByAuthor": false
    });

    if let Some((media_urn, video_title)) = media {
        let mut media_obj = serde_json::Map::new();
        media_obj.insert("id".to_string(), serde_json::Value::String(media_urn.to_string()));
        if let Some(t) = video_title {
            media_obj.insert("title".to_string(), serde_json::Value::String(t.to_string()));
        }
        body["content"] = serde_json::json!({ "media": serde_json::Value::Object(media_obj) });
    }

    let client = http_client()?;
    let url = format!("{REST_BASE}/posts");
    let resp = rest_post_json(&client, token, &url, body)?;
    let status = resp.status();
    if status != reqwest::StatusCode::CREATED {
        let resp_body = resp.text().unwrap_or_default();
        return Err(map_api_error(status, &resp_body));
    }

    let post_urn = resp
        .headers()
        .get("x-restli-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    post_urn.ok_or(SocialError::ApiError)
}

/// Images API: `initializeUpload` → binary PUT → image URN.
fn upload_image(
    client: &reqwest::blocking::Client,
    author: &str,
    token: &str,
    path: &str,
) -> Result<String, SocialError> {
    let url = format!("{REST_BASE}/images?action=initializeUpload");
    let body = serde_json::json!({
        "initializeUploadRequest": { "owner": author }
    });
    let resp = rest_post_json(client, token, &url, body)?;
    let status = resp.status();
    let resp_body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(map_api_error(status, &resp_body));
    }

    #[derive(serde::Deserialize)]
    struct InitResponse {
        value: Option<InitValue>,
    }
    #[derive(serde::Deserialize)]
    struct InitValue {
        upload_url: Option<String>,
        image: Option<String>,
    }
    let parsed: InitResponse =
        serde_json::from_str(&resp_body).map_err(|_| SocialError::UploadFailed)?;
    let value = parsed.value.ok_or(SocialError::UploadFailed)?;
    let upload_url = value.upload_url.filter(|u| !u.is_empty());
    let image_urn = value.image.filter(|u| !u.is_empty());
    let (upload_url, image_urn) = match (upload_url, image_urn) {
        (Some(u), Some(i)) => (u, i),
        _ => return Err(SocialError::UploadFailed),
    };

    // Yerel görsel dosyayı LinkedIn'in sağladığı imzalı adrese PUT ile yükle.
    let bytes = std::fs::read(path).map_err(|_| SocialError::FileNotFound)?;
    let up = client
        .put(&upload_url)
        .header("Content-Type", "application/octet-stream")
        .body(bytes)
        .send()
        .map_err(|_| SocialError::UploadFailed)?;
    if !up.status().is_success() {
        return Err(SocialError::UploadFailed);
    }

    Ok(image_urn)
}

/// Videos API: `initializeUpload` → 4 MB parça PUT + ETag → `finalizeUpload`
/// → `AVAILABLE` yoklaması → video URN.
fn upload_video(
    client: &reqwest::blocking::Client,
    author: &str,
    token: &str,
    path: &str,
) -> Result<String, SocialError> {
    let file_size = std::fs::metadata(path)
        .map_err(|_| SocialError::FileNotFound)?
        .len();
    if file_size == 0 {
        return Err(SocialError::InvalidVideoFile);
    }

    let url = format!("{REST_BASE}/videos?action=initializeUpload");
    let body = serde_json::json!({
        "initializeUploadRequest": {
            "owner": author,
            "fileSizeBytes": file_size,
            "uploadCaptions": false,
            "uploadThumbnail": false
        }
    });
    let resp = rest_post_json(client, token, &url, body)?;
    let status = resp.status();
    let resp_body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(map_api_error(status, &resp_body));
    }

    #[derive(serde::Deserialize)]
    struct InitResponse {
        value: Option<InitValue>,
    }
    #[derive(serde::Deserialize)]
    struct InitValue {
        video: Option<String>,
        upload_token: Option<String>,
        upload_instructions: Option<Vec<UploadInstruction>>,
    }
    #[derive(serde::Deserialize)]
    struct UploadInstruction {
        first_byte: Option<u64>,
        last_byte: Option<u64>,
        upload_url: Option<String>,
    }
    let parsed: InitResponse =
        serde_json::from_str(&resp_body).map_err(|_| SocialError::UploadSessionFailed)?;
    let value = parsed.value.ok_or(SocialError::UploadSessionFailed)?;
    let video_urn = value.video.filter(|u| !u.is_empty());
    let upload_token = value.upload_token.unwrap_or_default();
    let instructions = value
        .upload_instructions
        .filter(|v| !v.is_empty());
    let (video_urn, instructions) = match (video_urn, instructions) {
        (Some(v), Some(i)) => (v, i),
        _ => return Err(SocialError::UploadSessionFailed),
    };

    // Her parçayı diskten okuyup ilgili imzalı adrese PUT ile yükle; her
    // yanıttaki ETag'ı `uploadedPartIds` için sakla.
    let mut uploaded_part_ids = Vec::with_capacity(instructions.len());
    for instr in &instructions {
        let (first_byte, last_byte) = match (instr.first_byte, instr.last_byte) {
            (Some(f), Some(l)) if l >= f => (f, l),
            _ => return Err(SocialError::UploadFailed),
        };
        let upload_url = instr.upload_url.as_deref().unwrap_or_default();
        if upload_url.is_empty() {
            return Err(SocialError::UploadFailed);
        }

        let mut file = File::open(path).map_err(|_| SocialError::FileNotFound)?;
        file.seek(SeekFrom::Start(first_byte))
            .map_err(|_| SocialError::UploadFailed)?;
        let part_len = (last_byte - first_byte + 1) as usize;
        let mut buf = vec![0u8; part_len];
        file.read_exact(&mut buf)
            .map_err(|_| SocialError::UploadFailed)?;

        let up = client
            .put(upload_url)
            .header("Content-Type", "application/octet-stream")
            .body(buf)
            .send()
            .map_err(|_| SocialError::UploadFailed)?;
        if !up.status().is_success() {
            return Err(SocialError::UploadFailed);
        }
        let etag = up
            .headers()
            .get("etag")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        uploaded_part_ids.push(etag.ok_or(SocialError::UploadFailed)?);
    }

    // Parçaları birleştir ve video yüklemesini tamamla.
    let finalize_url = format!("{REST_BASE}/videos?action=finalizeUpload");
    let finalize_body = serde_json::json!({
        "finalizeUploadRequest": {
            "video": video_urn,
            "uploadToken": upload_token,
            "uploadedPartIds": uploaded_part_ids
        }
    });
    let resp = rest_post_json(client, token, &finalize_url, finalize_body)?;
    let status = resp.status();
    if !status.is_success() {
        let resp_body = resp.text().unwrap_or_default();
        return Err(map_api_error(status, &resp_body));
    }

    wait_video_available(client, token, &video_urn)?;
    Ok(video_urn)
}

/// Video işlenene kadar (`AVAILABLE`) yoklar. İşleme başarısız olursa veya
/// süre aşılırsa kontrollü hata döner; yayın başlatılmaz.
fn wait_video_available(
    client: &reqwest::blocking::Client,
    token: &str,
    video_urn: &str,
) -> Result<(), SocialError> {
    let url = format!("{REST_BASE}/videos/{}", url_encode_path(video_urn));
    for _ in 0..VIDEO_POLL_ATTEMPTS {
        let resp = match rest_get(client, token, &url) {
            Ok(r) => r,
            Err(_) => return Err(SocialError::MediaProcessingTimeout),
        };
        if !resp.status().is_success() {
            return Err(SocialError::MediaProcessingTimeout);
        }
        let body = resp.text().unwrap_or_default();

        #[derive(serde::Deserialize)]
        struct VideoStatusResponse {
            status: Option<String>,
        }
        match serde_json::from_str::<VideoStatusResponse>(&body) {
            Ok(parsed) => match parsed.status.as_deref() {
                Some("AVAILABLE") => return Ok(()),
                Some("PROCESSING_FAILED") | Some("WAITING_UPLOAD") => {
                    return Err(SocialError::UploadFailed);
                }
                _ => {}
            },
            Err(_) => return Err(SocialError::MediaProcessingTimeout),
        }
        std::thread::sleep(VIDEO_POLL_INTERVAL);
    }
    Err(SocialError::MediaProcessingTimeout)
}

/// LinkedIn REST hata durumunu kontrollü hata kodlarına eşler.
/// Ham LinkedIn hata cevabı kullanıcıya gösterilmez; token sızdırılmaz.
fn map_api_error(status: reqwest::StatusCode, _body: &str) -> SocialError {
    match status {
        reqwest::StatusCode::UNAUTHORIZED => SocialError::TokenExpired,
        reqwest::StatusCode::FORBIDDEN => SocialError::PermissionDenied,
        _ => SocialError::ApiError,
    }
}

// ---- Testler ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_is_current_format() {
        // Sürüm, resmî YYYYMM biçimindedir.
        assert_eq!(API_VERSION.len(), 6);
        assert!(API_VERSION.parse::<u32>().is_ok());
    }

    #[test]
    fn scopes_are_minimal_and_no_extra_read() {
        // En dar izin seti: kimlik (OpenID) + kişisel yayın.
        assert!(SCOPES.contains("openid"));
        assert!(SCOPES.contains("profile"));
        assert!(SCOPES.contains("email"));
        assert!(SCOPES.contains("w_member_social"));
        // Kurumsal izinler yalnızca uygulama onaylandığında eklenmeli.
        assert!(!SCOPES.contains("w_organization_social"));
        assert!(!SCOPES.contains("rw_organization_admin"));
        // Yalnız okuma yapılırsa gerekli olan izin istenmez (yayın için zorunlu değil).
        assert!(!SCOPES.contains("r_organization_social"));
    }

    #[test]
    fn page_roles_are_official_and_correct() {
        // Kabul edilen roller resmî değerlerdir; CONTENT_ADMINISTRATOR yanlış
        // eski değer asla kullanılmaz.
        assert!(PAGE_POST_ROLES.contains(&"ADMINISTRATOR"));
        assert!(PAGE_POST_ROLES.contains(&"CONTENT_ADMIN"));
        assert!(PAGE_POST_ROLES.contains(&"DIRECT_SPONSORED_CONTENT_POSTER"));
        assert!(!PAGE_POST_ROLES.contains(&"CONTENT_ADMINISTRATOR"));
    }

    #[test]
    fn no_client_secret_anywhere() {
        // Client Secret hiçbir kod satırında yoktur. Denetim yalnız test dışı
        // kod ve yorumlardan arındırılmış satırlarda yapılır.
        let module_source = include_str!("linkedin.rs");
        let prod_code = module_source.split("#[cfg(test)]").next().unwrap_or(module_source);
        let code: String = prod_code
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!code.contains("client_secret"));
        assert!(!code.contains("clientSecret"));
        assert!(!code.contains("ES_OPS_LINKEDIN_CLIENT_SECRET"));
    }

    #[test]
    fn pkce_verifier_is_urlsafe_and_correct_length() {
        let verifier = generate_pkce_verifier().unwrap();
        assert_eq!(verifier.len(), 43);
        assert!(verifier
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn pkce_challenge_is_deterministic_and_urlsafe() {
        let verifier = generate_pkce_verifier().unwrap();
        let c1 = pkce_challenge(&verifier).unwrap();
        let c2 = pkce_challenge(&verifier).unwrap();
        assert_eq!(c1, c2);
        assert_eq!(c1.len(), 43);
        assert!(c1
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
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
    fn authorize_url_includes_pkce_and_scope_without_secret() {
        let url = build_authorize_url(
            "cid",
            "http://127.0.0.1:9999/",
            "openid w_member_social",
            "st",
            "ch",
        );
        assert!(url.contains("client_id=cid"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid%20w_member_social"));
        assert!(url.contains("state=st"));
        assert!(url.contains("code_challenge=ch"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(!url.contains("client_secret"));
    }

    #[test]
    fn url_encode_path_encodes_urns() {
        assert_eq!(
            url_encode_path("urn:li:person:ABC123"),
            "urn%3Ali%3Aperson%3AABC123"
        );
        assert_eq!(url_encode_path("simple"), "simple");
    }

    #[test]
    fn video_part_size_matches_official_value() {
        // Resmî Videos API parça boyutu 4 MB'dir; sunucu aralıkları bu sınırla
        // uyumlu bildirir.
        assert_eq!(VIDEO_PART_SIZE, 4 * 1024 * 1024);
    }
}
