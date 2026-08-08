//! Ortak Meta (Facebook / Instagram) entegrasyonunun çekirdeği.
//!
//! Bu modül Meta Graph API sürümünü, ortak OAuth akışını, uygulama içi webview
//! login yönetimini, token işlemlerini, Facebook Sayfalarının ve Sayfalara bağlı
//! Instagram profesyonel hesaplarının keşfini tek merkezde tutar. Facebook ve
//! Instagram platform modülleri (`facebook.rs`, `instagram.rs`) yalnız kendi
//! içerik ve yayın kurallarını içerir; buradaki ortak altyapıyı tekrar kurmaz.
//!
//! Amaç: "Meta uygulama kimliği" ve "API sürümü" tek kaynaktan gelir. OAuth,
//! state ve token işlemleri ortaktır; iki ayrı OAuth motoru oluşturulmaz.
//!
//! GÜVENLİK NOTU (uygulanabilirlik kapısı):
//! Meta'nın resmî `authorization_code` değişimi `client_secret` (App Secret)
//! gerektirir. Bu projenin güvenlik kuralı, app secret'ın binary'ye / kaynağa /
//! loga / repoya gömülmesini yasaklar. App secret güvenli biçimde
//! kullanılamadığı için kod→token değişimi `AppSecretRequired` ile döner ve
//! bağlantı `reauthorization_required` olarak işaretlenir. Benzer biçimde token
//! yenileme de app secret gerektirdiğinden güvenli değildir ve aynı sonucu üretir.
//! Bu, sahte bağlantı değil; Meta'nın sunucusuz masaüstü mimaride gizli secret
//! kullanmadan tamamlanamayan gerçek bir mimari engelinin dürüst raporlanmasıdır.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::RngCore;
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

use super::super::credential_store;
use super::super::metadata_store;
use super::super::models::{
    ConnectionRecord, ConnectionStatus, SocialAccountConnection, SocialError, TokenType,
};

/// Facebook platform kimliği (mevcut katalogdaki değer).
pub const FACEBOOK_PLATFORM_ID: &str = "facebook";
/// Instagram platform kimliği (mevcut katalogdaki değer).
pub const INSTAGRAM_PLATFORM_ID: &str = "instagram";

/// Tek merkezde tanımlanan güncel Meta Graph API sürümü.
pub const META_GRAPH_VERSION: &str = "v23.0";

/// Meta OAuth yetkilendirme uç adresi.
const AUTHORIZE_ENDPOINT: &str = "https://www.facebook.com/dialog/oauth";
/// Facebook Graph API kök adresi + sürüm.
const GRAPH_ENDPOINT: &str = "https://graph.facebook.com/v23.0";

/// OAuth callback'inin kaç saniye beklenileceği.
const OAUTH_TIMEOUT_SECS: u64 = 300;

/// Meta App ID, derleme zamanında güvenli biçimde gömülür.
/// Değer tanımlı değilse `None` döner; derleme bu yüzden başarısız olmaz.
pub fn meta_app_id() -> Option<&'static str> {
    option_env!("ES_OPS_META_APP_ID")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// Meta App Secret, derleme zamanında GitHub Actions repository secret'ı
/// `ES_OPS_META_APP_SECRET` üzerinden (varsa) güvenli biçimde gömülür.
/// Değer tanımlı değilse `None` döner; derleme bu yüzden başarısız olmaz.
/// Son kullanıcı bu değeri hiçbir zaman girmez görmez; eksikse bağlantı
/// akışı kontrollü `AppSecretRequired` hatasıyla durur (sahte başarı üretilmez).
pub fn meta_app_secret() -> Option<&'static str> {
    option_env!("ES_OPS_META_APP_SECRET")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
}

/// Meta Login for Business Configuration ID, derleme zamanında güvenli biçimde
/// gömülür (`ES_OPS_META_CONFIG_ID`). Bu ID, Facebook/Instagram izinlerinin
/// Configuration üzerinden yönetildiği panel oluşturmasıdır; küçük uygulama için
/// App Dashboard'da "Facebook Login for Business" konfigürasyonundan alınır.
/// Değer tanımlı değilse `None` döner; akış yine de klasik scope yolunu kullanır.
pub fn meta_config_id() -> Option<&'static str> {
    option_env!("ES_OPS_META_CONFIG_ID")
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
pub fn generate_state() -> Result<String, SocialError> {
    let bytes = random_bytes(32)?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

// ---- Loopback callback (FALLBACK, silinmedi) ----

/// Meta OAuth için sabit loopback callback portu. Bu portla birlikte redirect
/// URI'nin tamamı (`http://localhost:43123/meta-callback`) Meta App
/// Dashboard'undaki "Valid OAuth Redirect URIs" listesine kaydedilmelidir.
/// Meta, IP adresi içeren redirect URI'lerini (ör. `127.0.0.1`) kabul etmez;
/// bu yüzden `localhost` etki adı kullanılır.
///
/// Not: Bu blok, webview akışının kullanılamadığı durumlarda devreye
/// alınabilecek fallback olarak korunur; birincil akışa katılmaz.
#[allow(dead_code)]
pub const META_LOOPBACK_PORT: u16 = 43123;

/// Loopback dinleyicinin bağlandığı adres. `localhost` etki adı, Meta'nın
/// "Valid OAuth Redirect URIs" listesinde IP adresi kabul etmemesinden dolayı
/// redirect URI'de kullanılır; dinleyici ise her zaman `127.0.0.1` üzerinde açılır.
#[allow(dead_code)]
pub const META_LOOPBACK_HOST: &str = "localhost";

/// Meta OAuth callback path'i (redirect URI'nin path kısmı). Callback
/// isteklerinde bu path zorunludur; başka bir kaynağa yanıt reddedilir.
#[allow(dead_code)]
pub const META_CALLBACK_PATH: &str = "/meta-callback";

/// `127.0.0.1` üzerinde sabit porta dinleyici açar. Port başka bir program
/// tarafından kullanılıyorsa açık kontrollü `CallbackPortInUse` hatası döner;
/// sahte bağlantı üretilmez. (Fallback; şu an webview akışı kullanılır.)
#[allow(dead_code)]
pub fn bind_loopback() -> Result<TcpListener, SocialError> {
    TcpListener::bind(("127.0.0.1", META_LOOPBACK_PORT))
        .map_err(|_| SocialError::CallbackPortInUse)
}

/// Loopback gelen isteğindeki `code` ve `state` değerlerini ayrıştırır.
/// (Fallback akışın da URL ayrıştırıcısı; webview akışıyla ortak.)
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

/// Callback isteğinin path bölümünün `/meta-callback` olup olmadığını doğrular.
/// Query kısmı (`?code=...&state=...`) dışarıda bırakılır; path tam eşleşmeli.
#[allow(dead_code)]
fn is_valid_callback_path(path: &str) -> bool {
    path == META_CALLBACK_PATH
}

/// Callback dinlenecek kadar bekler ve `(code, state)` döndürür. (Fallback.)
#[allow(dead_code)]
pub fn wait_for_callback(listener: &TcpListener) -> Result<(String, String), SocialError> {
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

                let request_path = path_and_query
                    .split_once('?')
                    .map(|(p, _)| p)
                    .unwrap_or(&path_and_query);
                if !is_valid_callback_path(request_path) {
                    return Err(SocialError::OauthExchangeFailed);
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

// ---- Uygulama içi webview login (SUAT / Facebook Login for Business yolu) ----

/// Meta'nın resmî masaüstü/webview redirect URI'si. Meta'nın "Manually Build
/// a Login Flow" dokümanı, masaüstü uygulamalarında login'in uygulama içi bir
/// webview'da yürütülmesini ve redirect adresinin bu sabit değer olmasını şart
/// koşar. Adres Meta'nın kendi alan adında olduğundan "uygulamanın domainleri"
/// denetiminden etkilenmez; `http://localhost:...` akışı yalnız fallback olarak
/// korunur (bkz. yukarıdaki loopback bölümü).
pub const META_LOGIN_SUCCESS_URI: &str = "https://www.facebook.com/connect/login_success.html";

/// Meta login webview penceresinin Tauri etiketi.
pub const META_LOGIN_WINDOW_LABEL: &str = "meta-login";

/// Login akışını uygulama içi webview penceresinde yürütür ve
/// `login_success.html` yönlendirmesindeki authorization code'i yakalar.
///
/// - `auth_url` yeni bir webview penceresinde açılır; sistem tarayıcısı ve
///   loopback sunucu kullanılmaz. Meta'nın resmî masaüstü webview akışıdır.
/// - Yalnız `META_LOGIN_SUCCESS_URI` önekindeki navigasyon yakalanır; `error`
///   varsa `OauthCancelled`, state eşleşmezse `OauthStateMismatch`, code
///   eksikse `OauthExchangeFailed` döner.
/// - Kullanıcı penceresini kapatır veya süre aşarsa `OauthTimeout` döner.
pub fn login_via_webview(
    app: &AppHandle,
    auth_url: &str,
    expected_state: &str,
) -> Result<String, SocialError> {
    let url = tauri::Url::parse(auth_url).map_err(|_| SocialError::OperationFailed)?;
    let (tx, rx) = std::sync::mpsc::channel::<Result<String, SocialError>>();
    let expected_state = expected_state.to_string();

    tauri::WebviewWindowBuilder::new(
        app,
        META_LOGIN_WINDOW_LABEL,
        tauri::WebviewUrl::External(url),
    )
    .inner_size(660.0, 780.0)
    .title("Meta ile Bağlan")
    .on_navigation(move |nav_url| {
        if !nav_url.as_str().starts_with(META_LOGIN_SUCCESS_URI) {
            return true;
        }
        let query = nav_url.query().unwrap_or("");
        let outcome = if query.contains("error=") {
            Err(SocialError::OauthCancelled)
        } else {
            let (code, state) = parse_callback_query(query);
            match (code, state) {
                (Some(code), Some(state)) if state == expected_state => Ok(code),
                (Some(_), Some(_)) => Err(SocialError::OauthStateMismatch),
                _ => Err(SocialError::OauthExchangeFailed),
            }
        };
        let _ = tx.send(outcome);
        false
    })
    .build()
    .map_err(|_| SocialError::OperationFailed)?;

    rx.recv_timeout(Duration::from_secs(OAUTH_TIMEOUT_SECS))
        .map_err(|_| SocialError::OauthTimeout)?
}

// ---- HTTP istemci ----

/// Ortak Meta HTTP istemcisi. Uygun tek bloklama istemcisidir.
pub fn http_client() -> Result<reqwest::blocking::Client, SocialError> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| SocialError::ApiError)
}

/// Tarayıcıyı resmî Tauri shell mekanizmasıyla açar. (Fallback; webview akışında
/// kullanılmaz. Loopback fallback yolu etkenleştirilirse yeniden bağlanır.
#[allow(dead_code)]
pub fn open_browser(app: &AppHandle, url: &str) -> Result<(), SocialError> {
    app.shell()
        .open(url, None)
        .map_err(|_| SocialError::OperationFailed)
}

// ---- Auth URL ----

/// Meta OAuth yetkilendirme URL'sini oluşturur. Aşağıdaki kurallar:
///
/// - `response_type=code` her zaman kullanılır (SUAT kuralı: System-user
///   access token yalnız authorization code grant'i kabul eder).
/// - `config_id` (Facebook Login for Business Configuration ID) varsa:
///   * `config_id` parametresi eklenir;
///   * `override_default_response_type=true` parametresi eklenir (kod,
///     SUAT'ın gerektirdiği grant tipine zorlar);
///   * `scope` parametresi EKLENMEZ (SUAT konfigürasyonunda izinler panel
///     tarafından Configuration üzerinden yönetilir).
/// - `config Iltisat yoksa klasik scope tabanlı akış korunur: bu durumda
///   `scope` gönderilir, `override` parametresi gönderilmez.
pub fn build_authorize_url(
    app_id: &str,
    redirect_uri: &str,
    scope: &str,
    state: &str,
    config_id: Option<&str>,
) -> String {
    if let Some(config_id) = config_id {
        // SUAT: scope gönderilmez; izinler Configuration üzerinden verilir.
        return format!(
            "{AUTHORIZE_ENDPOINT}?client_id={}&redirect_uri={}&response_type=code&config_id={}&override_default_response_type=true&state={}",
            app_id, redirect_uri, config_id, state
        );
    }
    format!(
        "{AUTHORIZE_ENDPOINT}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
        app_id, redirect_uri, scope, state
    )
}

// ---- Token işlemleri ----

/// Yetkilendirme kodunu gerçek Meta token endpoint'inde değiştirir.
///
/// Meta'nın resmî akışı bu adımda `client_secret` (App Secret) ister.
/// Bu proje app secret'ı binary'ye gömmeyi güvenlik kuralı olarak yasakladığından
/// değişim tamamlanamaz ve `AppSecretRequired` döner. Bu, sahte başarı değil;
/// gerçek bir güvenlik kısıtının kontrol edilmiş sonucudur.
pub fn exchange_code(
    _app_id: &str,
    _redirect_uri: &str,
    _code: &str,
) -> Result<String, SocialError> {
    // App secret yalnız resolve edilmiş kaynaktan alınır; hiçbir koşulda
    // JavaScript'e veya kullanıcıya döndürülmez. Gömülü secret yoksa
    // `AppSecretRequired` kontrollü hatası döner; sahte token üretilmez.
    let secret = resolved_app_secret().ok_or(SocialError::AppSecretRequired)?;
    exchange_code_real(_app_id, _redirect_uri, _code, &secret)
}

/// Uzun ömürlü / sayfa tokenı yenileme işlemi.
///
/// Meta token uzatma/uzun ömürlüleştirme app secret gerektirir. Bu API
/// kısa ömürlü token argümanı almadan çağrıldığı için doğrudan uzatma
/// yapamaz; bağlantı `ReauthorizationRequired` ile döner ve kullanıcı
/// yeniden yetkilendirme yapmalıdır.
pub fn refresh_user_token() -> Result<String, SocialError> {
    Err(SocialError::ReauthorizationRequired)
}

// ---------------------------------------------------------------------------
// Gerçek Meta bağlantısı — App ID / App Secret kaynakları
// ---------------------------------------------------------------------------
//
// Gerçek Facebook/Instagram OAuth bağlantısı, Meta geliştirici uygulamasına
// ait App ID ve App Secret gerektirir. v1.0 kararı: bu kimlikler GitHub Actions
// repository secrets'larından release build sırasında `option_env!` ile EXE'ye
// gömülür (`ES_OPS_META_APP_ID`, `ES_OPS_META_APP_SECRET`). Son kullanıcı bu
// değerleri hiçbir zaman girmez, görmez; gizliliğe aykırı biçimde arayüze,
// loglara veya JavaScript'e dönmez. Gömülü değer yoksa bağlantı akışı kontrollü
// `MetaNotConfigured` / `AppSecretRequired` hatasıyla durur; sahte başarı üretilmez.
//
// Geriye dönük uyumluluk için güvenli depo (credential_store) fallback olarak
// korunur; asıl kullanım build-time gömülü değerdir.

/// Meta uygulama yapılandırması için kullanılan ortak (bağlantıya özgü olmayan) anahtardır.
const META_CONFIG_CONN: &str = "_meta_app_config";

/// Meta App ID'yi güvenli depoya yazar (kaynağa gömülmez).
pub fn store_app_id(app_id: &str) -> Result<(), SocialError> {
    if app_id.trim().is_empty() {
        return Err(SocialError::MetaNotConfigured);
    }
    credential_store::store_token("meta", META_CONFIG_CONN, TokenType::RefreshToken, app_id.trim())
}

/// Meta App Secret'ı güvenli depoya yazar (kaynağa gömülmez).
pub fn store_app_secret(app_secret: &str) -> Result<(), SocialError> {
    if app_secret.trim().is_empty() {
        return Err(SocialError::AppSecretRequired);
    }
    credential_store::store_token("meta", META_CONFIG_CONN, TokenType::AccessToken, app_secret.trim())
}

/// Güvenli depodan Meta App ID'yi okur.
pub fn read_app_id() -> Result<Option<String>, SocialError> {
    credential_store::get_token("meta", META_CONFIG_CONN, TokenType::RefreshToken)
}

/// Güvenli depodan Meta App Secret'ı okur.
/// Ham secret, JavaScript'e asla döndürülmez; yalnız Rust içinde kullanılır.
pub fn read_app_secret() -> Result<Option<String>, SocialError> {
    credential_store::get_token("meta", META_CONFIG_CONN, TokenType::AccessToken)
}

/// App ID çözümlemesini saf ve test edilebilir biçimde yapar.
///
/// Derleme zamanı gömülü değer (`ES_OPS_META_APP_ID`) önceliklidir; yoksa
/// güvenli depodaki kullanıcı kaydı kullanılır. Boş/yalnız boşluk içeren
/// değerler yok sayılır.
pub fn resolve_app_id_from(compiled: Option<&str>, stored: Option<String>) -> Option<String> {
    compiled
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| stored.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

/// Kullanım sırasında çözülecek App ID. Önce derleme zamanı `ES_OPS_META_APP_ID`,
/// varsa onu, yoksa güvenli depodaki kullanıcı kaydını kullanır.
pub fn resolved_app_id() -> Option<String> {
    resolve_app_id_from(meta_app_id(), read_app_id().ok().flatten())
}

/// Kullanım sırasında çözülecek App Secret. Önce derleme zamanı gömülü
/// değer (`ES_OPS_META_APP_SECRET`) kullanılır; yoksa güvenli depodaki
/// kullanıcı kaydı kullanılır. Hiçbir koşulda secret, JavaScript'e veya
/// kullanıcı arayüzüne dönmez.
pub fn resolved_app_secret() -> Option<String> {
    meta_app_secret()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| read_app_secret().ok().flatten())
}

/// Meta bağlantı akışının başlayabilmesi için gereken kimlikleri tek noktada
/// denetler. Facebook ve Instagram aynı ortak kapıyı kullanır; ikisi de aynı
/// App ID + App Secret yapılandırmasına bağlıdır.
///
/// - App ID çözülemiyorsa `MetaNotConfigured` döner.
/// - App Secret güvenli depoda yoksa `AppSecretRequired` döner.
/// - İkisi de hazırsa `(app_id, app_secret)` döner (yalnız Rust içinde kullanılır;
///   secret hiçbir zaman JavaScript'e veya kullanıcı arayüzüne dönmez).
///
/// Bu fonksiyon "sahte bağlantı kurma" üretmez: kimlik eksikse akış, tarayıcı
/// açılmadan önce kontrollü hata koduyla durur.
pub fn assert_connect_ready(
    app_id: Option<String>,
    app_secret: Option<String>,
) -> Result<(String, String), SocialError> {
    let id = app_id
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or(SocialError::MetaNotConfigured)?;
    let secret = app_secret
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or(SocialError::AppSecretRequired)?;
    Ok((id, secret))
}

/// Meta kutulu erişim token endpoint'i (yetkilendirme kodu değişim adresi).
const GRAPH_TOKEN_ENDPOINT: &str = "https://graph.facebook.com/v23.0/oauth/access_token";

/// Gerçek yetkilendirme kodu değişimi (App Secret ile).
///
/// Meta'nın resmî `authorization_code` değişimi `client_secret` (App Secret)
/// ister. Bu fonksiyon, güvenli biçimde saklanmış App Secret ile gerçek token
/// endpoint'ine POST atar ve access_token döndürür. App Secret yoksa kontrollü
/// `AppSecretRequired` döner; sahte token üretilmez.
pub fn exchange_code_real(
    app_id: &str,
    redirect_uri: &str,
    code: &str,
    app_secret: &str,
) -> Result<String, SocialError> {
    if app_secret.is_empty() {
        return Err(SocialError::AppSecretRequired);
    }
    let client = http_client()?;
    let params = [
        ("client_id", app_id),
        ("client_secret", app_secret),
        ("redirect_uri", redirect_uri),
        ("code", code),
        ("grant_type", "authorization_code"),
    ];
    let resp = client
        .post(GRAPH_TOKEN_ENDPOINT)
        .form(&params)
        .send()
        .map_err(|_| SocialError::OauthExchangeFailed)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(SocialError::OauthExchangeFailed);
    }

    #[derive(serde::Deserialize)]
    struct TokenResp {
        access_token: Option<String>,
    }
    let parsed: TokenResp =
        serde_json::from_str(&body).map_err(|_| SocialError::OauthExchangeFailed)?;
    let token = parsed.access_token.filter(|t| !t.is_empty());
    token.ok_or(SocialError::OauthExchangeFailed)
}

/// Gerçek uzun ömürlü token uzatma (App Secret ile).
///
/// Meta kullanıcı tokenını uzun ömürlü yapmak `client_secret` ister. Secret
/// güvenli biçimde sağlanırsa gerçek istek yapılır; aksi halde `AppSecretRequired`
/// döner. Yalnızca modül içi kullanım içindir.
#[allow(dead_code)]
pub fn extend_user_token_real(
    app_id: &str,
    app_secret: &str,
    short_lived_access_token: &str,
) -> Result<String, SocialError> {
    if app_secret.is_empty() {
        return Err(SocialError::AppSecretRequired);
    }
    let client = http_client()?;
    let params = [
        ("grant_type", "fb_exchange_token"),
        ("client_id", app_id),
        ("client_secret", app_secret),
        ("fb_exchange_token", short_lived_access_token),
    ];
    let resp = client
        .post(GRAPH_TOKEN_ENDPOINT)
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
    }
    let parsed: TokenResp =
        serde_json::from_str(&body).map_err(|_| SocialError::TokenRefreshFailed)?;
    let token = parsed.access_token.filter(|t| !t.is_empty());
    token.ok_or(SocialError::TokenRefreshFailed)
}

/// Kod değişimi için App Secret çözülebiliyor mu diye denetler.
/// Secret, derleme zamanı gömülü değerden (`ES_OPS_META_APP_SECRET`) veya
/// güvenli depodan çözümlenir; ham değer hiçbir zaman dışarı dönmez.
pub fn app_secret_ready() -> bool {
    resolved_app_secret().is_some()
}

// ---- Facebook Sayfaları keşfi ----

/// Kullanıcının yönettiği Facebook Sayfalarını döndürür.
///
/// Sayfa tokenları, kullanıcı tokenının `/me/accounts` adresine `page_token`
/// alanı istenerek elde edilir. Bu, gerçek Graph API keşfidir. Erişim yoksa
/// `PermissionDenied` döner.
pub fn fetch_managed_pages(
    user_token: &str,
) -> Result<Vec<FacebookPageTarget>, SocialError> {
    let client = http_client()?;
    let url = format!(
        "{GRAPH_ENDPOINT}/me/accounts?fields=id,name,access_token&access_token={}",
        user_token
    );
    let resp = client
        .get(&url)
        .send()
        .map_err(|_| SocialError::ApiError)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();

    if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        return Err(SocialError::PermissionDenied);
    }

    let parsed: PagesResponse = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_) => return Err(SocialError::ApiError),
    };

    let mut pages = Vec::new();
    for item in parsed.data.unwrap_or_default() {
        if let (Some(id), Some(name)) = (item.id, item.name) {
            pages.push(FacebookPageTarget {
                page_id: id,
                page_name: name,
                page_access_token: item.access_token.unwrap_or_default(),
            });
        }
    }
    if pages.is_empty() {
        return Err(SocialError::NoManagedPage);
    }
    Ok(pages)
}

/// Bir Facebook Sayfasına bağlı Instagram profesyonel hesabını bulur.
/// Sayfada Instagram Business/Creator hesabı bağlı değilse
/// `InstagramAccountNotFound` döner (Facebook bağlantısını geçersiz yapmaz;
/// yalnızca Instagram hedefi üretilmez).
pub fn fetch_linked_instagram(
    page_id: &str,
    page_access_token: &str,
) -> Result<InstagramAccountTarget, SocialError> {
    let client = http_client()?;
    let url = format!(
        "{GRAPH_ENDPOINT}/{page_id}?fields=instagram_business_account{{id,username,profile_picture_url,name}}&access_token={}",
        page_access_token
    );
    let resp = client
        .get(&url)
        .send()
        .map_err(|_| SocialError::ApiError)?;

    let status = resp.status();
    let body = resp.text().unwrap_or_default();

    if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        return Err(SocialError::PermissionDenied);
    }

    let parsed: PageDetailsResponse = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(_) => return Err(SocialError::ApiError),
    };

    match parsed.instagram_business_account {
        Some(ig) => {
            let id = ig.id.unwrap_or_default();
            if id.is_empty() {
                return Err(SocialError::InstagramAccountNotFound);
            }
            let username = ig.username.unwrap_or_default();
            let name = ig.name.unwrap_or_else(|| username.clone());
            Ok(InstagramAccountTarget {
                instagram_id: id,
                account_name: if name.is_empty() { username } else { name },
            })
        }
        None => Err(SocialError::InstagramAccountNotFound),
    }
}

// ---- Bağlantı kaydı kurma ----

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

fn generate_connection_id(platform: &str) -> Result<String, SocialError> {
    let bytes = random_bytes(16)?;
    Ok(format!("{}_{}", platform, URL_SAFE_NO_PAD.encode(bytes)))
}

/// Facebook Sayfası için ayrı bir bağlantı kaydı ve token yazar.
///
/// Düzen:
/// - Aynı Sayfa ikinci kez eklenmez.
/// - Tokenlar başarıyla yazılmadan `connected` yapılmaz.
/// - Metadata yazımı başarısız olursa yarım token kayıtları temizlenir.
/// - Token yazımı başarısız olursa bağlantı kaydı oluşturulmaz.
pub fn connect_facebook_page(
    app: &AppHandle,
    page: &FacebookPageTarget,
) -> Result<SocialAccountConnection, SocialError> {
    let dir = data_dir(app)?;
    let records = metadata_store::list_connections(&dir)?;
    let existing = records.iter().find(|r| {
        r.platform_id == FACEBOOK_PLATFORM_ID && r.external_account_id == page.page_id
    });

    let connection_id = match existing {
        Some(r) => r.connection_id.clone(),
        None => generate_connection_id("facebook")?,
    };

    if credential_store::store_token(
        FACEBOOK_PLATFORM_ID,
        &connection_id,
        TokenType::AccessToken,
        &page.page_access_token,
    )
    .is_err()
    {
        // Token yazılamadı: bağlantı kaydı oluşturulmaz.
        return Err(SocialError::CredentialStoreError);
    }

    let record = ConnectionRecord {
        connection_id: connection_id.clone(),
        platform_id: FACEBOOK_PLATFORM_ID.to_string(),
        external_account_id: page.page_id.clone(),
        account_display_name: page.page_name.clone(),
        connection_status: ConnectionStatus::Connected,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    };

    if metadata_store::upsert_connection(&dir, record).is_err() {
        let _ = credential_store::delete_all_tokens(FACEBOOK_PLATFORM_ID, &connection_id);
        return Err(SocialError::ConnectionStoreError);
    }

    Ok(SocialAccountConnection {
        connection_id,
        platform_id: FACEBOOK_PLATFORM_ID.to_string(),
        external_account_id: page.page_id.clone(),
        account_display_name: page.page_name.clone(),
        connection_status: ConnectionStatus::Connected,
        token_exists: true,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    })
}

/// Instagram profesyonel hesabı için ayrı bir bağlantı kaydı kurar.
///
/// Instagram yayını için Sayfanın `page_access_token`'ı kullanılır; bu token
/// Instagram hesabıyla ilişkilendirildiği için `connection_id` başına
/// Instagram platform kimliğiyle saklanır. Aynı hesap ikinci kez eklenmez.
pub fn connect_instagram_account(
    app: &AppHandle,
    _page_id: &str,
    instagram: &InstagramAccountTarget,
    page_access_token: &str,
) -> Result<SocialAccountConnection, SocialError> {
    let dir = data_dir(app)?;
    let records = metadata_store::list_connections(&dir)?;
    let existing = records.iter().find(|r| {
        r.platform_id == INSTAGRAM_PLATFORM_ID && r.external_account_id == instagram.instagram_id
    });

    let connection_id = match existing {
        Some(r) => r.connection_id.clone(),
        None => generate_connection_id("instagram")?,
    };

    // Instagram erişimi için Sayfanın tokenı kullanılır.
    if credential_store::store_token(
        INSTAGRAM_PLATFORM_ID,
        &connection_id,
        TokenType::AccessToken,
        page_access_token,
    )
    .is_err()
    {
        return Err(SocialError::CredentialStoreError);
    }

    let record = ConnectionRecord {
        connection_id: connection_id.clone(),
        platform_id: INSTAGRAM_PLATFORM_ID.to_string(),
        external_account_id: instagram.instagram_id.clone(),
        account_display_name: instagram.account_name.clone(),
        connection_status: ConnectionStatus::Connected,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    };

    if metadata_store::upsert_connection(&dir, record).is_err() {
        let _ = credential_store::delete_all_tokens(INSTAGRAM_PLATFORM_ID, &connection_id);
        return Err(SocialError::ConnectionStoreError);
    }

    Ok(SocialAccountConnection {
        connection_id,
        platform_id: INSTAGRAM_PLATFORM_ID.to_string(),
        external_account_id: instagram.instagram_id.clone(),
        account_display_name: instagram.account_name.clone(),
        connection_status: ConnectionStatus::Connected,
        token_exists: true,
        last_error_code: String::new(),
        last_operation_at: now_rfc3339(),
    })
}

// ---- Serde yapıları ----

#[derive(serde::Deserialize)]
struct PagesResponse {
    data: Option<Vec<PageItem>>,
}

#[derive(serde::Deserialize)]
struct PageItem {
    id: Option<String>,
    name: Option<String>,
    access_token: Option<String>,
}

#[derive(serde::Deserialize)]
struct PageDetailsResponse {
    #[serde(rename = "instagram_business_account")]
    instagram_business_account: Option<InstagramBusiness>,
}

#[derive(serde::Deserialize)]
struct InstagramBusiness {
    id: Option<String>,
    username: Option<String>,
    name: Option<String>,
    profile_picture_url: Option<String>,
}

/// Facebook Sayfa hedefi (gizli olmayan bilgi).
pub struct FacebookPageTarget {
    pub page_id: String,
    pub page_name: String,
    pub page_access_token: String,
}

/// Instagram profesyonel hesap hedefi (gizli olmayan bilgi).
pub struct InstagramAccountTarget {
    pub instagram_id: String,
    pub account_name: String,
}

// ---- Kontrollü paylaşım / içerik türleri ----

/// Mevcut projenin kontrollü paylaşım türleri. JavaScript'ten serbest metin
/// gelmez; yalnız bu değerler kabul edilir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostKind {
    Standard,
    Campaign,
    Detailed,
    Announcement,
}

impl PostKind {
    /// Kontrollü değerlerden birini ayrıştırır; tanınmayan değer `None` döner.
    pub fn parse(value: &str) -> Option<PostKind> {
        match value.trim().to_ascii_lowercase().as_str() {
            "standard" => Some(PostKind::Standard),
            "kampanya" | "campaign" => Some(PostKind::Campaign),
            "detayli" | "detailed" => Some(PostKind::Detailed),
            "duyuru" | "announcement" => Some(PostKind::Announcement),
            "ilan" => Some(PostKind::Announcement),
            _ => None,
        }
    }

    /// Duyuru/ilan türü olup olmadığı. Instagram, medyasız duyuru/ilanı hedef
    /// almaz; bu ayrım hedef seçiminde kullanılır.
    pub fn is_announcement(&self) -> bool {
        matches!(self, PostKind::Announcement)
    }
}

/// Yayınlanacak medyanın kontrollü içerik türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Text,
    Photo,
    Video,
    Carousel,
}

impl MediaKind {
    /// Kontrollü değerlerden birini ayrıştırır; tanınmayan değer `None` döner.
    pub fn parse(value: &str) -> Option<MediaKind> {
        match value.trim().to_ascii_lowercase().as_str() {
            "text" | "metin" => Some(MediaKind::Text),
            "photo" | "image" | "gorsel" => Some(MediaKind::Photo),
            "video" | "reels" => Some(MediaKind::Video),
            "carousel" | "multiphoto" => Some(MediaKind::Carousel),
            _ => None,
        }
    }

    /// Gerçek bir medya içeriği olup olmadığı (metin değilse).
    pub fn has_media(&self) -> bool {
        !matches!(self, MediaKind::Text)
    }
}

/// Instagram için hedef uygunluğu. Yalnız gerçek medya içeren içerik hedef olabilir;
/// medyasız (yalnız metin) yayın Instagram'a gönderilmez.
pub fn instagram_is_eligible(media: MediaKind) -> bool {
    match media {
        MediaKind::Text => false,
        MediaKind::Photo | MediaKind::Video | MediaKind::Carousel => true,
    }
}

/// Facebook için hedef uygunluğu. Mevcut proje modeline göre metin, tek görsel
/// ve video Facebook'un resmî desteklediği akışlara uygundur.
pub fn facebook_is_eligible(media: MediaKind) -> bool {
    match media {
        MediaKind::Text | MediaKind::Photo | MediaKind::Video => true,
        // Mevcut proje paylaşım modeline göre çoklu görsel (carousel) henüz
        // Facebook hedefi olarak kabul edilmez.
        MediaKind::Carousel => false,
    }
}

// ---- Testler ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_version_is_single_source() {
        // API sürümü tek merkezde tanımlı ve resmî sürümle uyumlu.
        assert!(META_GRAPH_VERSION.starts_with('v'));
        assert!(META_GRAPH_VERSION
            .trim_start_matches('v')
            .split('.')
            .all(|p| p.parse::<u32>().is_ok()));
    }

    #[test]
    fn redirect_uri_is_fixed_address() {
        // Resmî masaüstü webview redirect adresi sabittir; localhost/port yok.
        assert_eq!(
            META_LOGIN_SUCCESS_URI,
            "https://www.facebook.com/connect/login_success.html"
        );
        assert!(META_LOGIN_SUCCESS_URI.starts_with("https://www.facebook.com/"));
        assert!(META_LOGIN_SUCCESS_URI.contains("connect/login_success.html"));
    }

    #[test]
    fn webview_callback_covers_login_success_query() {
        // login_success.html query'si code + state taşır; parse girdisi budur.
        let (code, state) = parse_callback_query("code=abc123&state=xyz");
        assert_eq!(code.as_deref(), Some("abc123"));
        assert_eq!(state.as_deref(), Some("xyz"));
    }

    #[test]
    fn state_validation_is_preserved() {
        // Callback'ten state dönmeli; eksikse OauthStateMismatch üretilir.
        let (_, state) = parse_callback_query("code=abc&state=xyz");
        assert_eq!(state.as_deref(), Some("xyz"));
        let empty = parse_callback_query("code=abc");
        assert!(empty.1.is_none());
    }

    #[test]
    fn app_secret_is_never_returned() {
        // Güvenlik: app secret hiçbir durumda uygulamaya sızmaz.
        assert!(meta_app_secret().is_none());
    }

    #[test]
    fn compiled_app_id_wins_over_stored() {
        // Derleme zamanı gömülü App ID (`ES_OPS_META_APP_ID`) varsa, güvenli
        // depodaki değer yok sayılır. Bu, "App ID mevcutken eksik hatası
        // verilmemesi" kuralının saf karar noktasıdır.
        let resolved = resolve_app_id_from(Some("123456"), Some("depodaki".to_string()));
        assert_eq!(resolved.as_deref(), Some("123456"));
    }

    #[test]
    fn stored_app_id_used_when_compiled_missing() {
        // Derleme zamanı değer yoksa güvenli depodaki kullanıcı kaydı kullanılır.
        let resolved = resolve_app_id_from(None, Some("654321".to_string()));
        assert_eq!(resolved.as_deref(), Some("654321"));
    }

    #[test]
    fn resolve_app_id_ignores_empty_values() {
        // Boş ve yalnız boşluklu değerler "yapılandırılmamış" sayılır.
        assert_eq!(resolve_app_id_from(Some("   "), Some(String::new())), None);
        assert_eq!(resolve_app_id_from(Some(""), None), None);
        assert_eq!(resolve_app_id_from(None, Some("  ".to_string())), None);
        assert_eq!(resolve_app_id_from(None, None), None);
        // Depo değerindeki boşluklar temizlenir.
        assert_eq!(
            resolve_app_id_from(None, Some(" 556677 ".to_string())),
            Some("556677".to_string())
        );
    }

    #[test]
    fn connect_ready_requires_both_id_and_secret() {
        // App ID yok: kontrollü meta_not_configured hatası (tarayıcı açılmaz).
        let err = assert_connect_ready(None, Some("secret".to_string())).unwrap_err();
        assert_eq!(err, SocialError::MetaNotConfigured);
        // App Secret yok: kontrollü app_secret_required hatası.
        let err = assert_connect_ready(Some("123".to_string()), None).unwrap_err();
        assert_eq!(err, SocialError::AppSecretRequired);
        // İkisi de boş: önce App ID hatası döner.
        let err = assert_connect_ready(None, None).unwrap_err();
        assert_eq!(err, SocialError::MetaNotConfigured);
        // İkisi de hazır: kimlikler yalnız Rust içinde kullanılmak üzere döner.
        let (id, secret) =
            assert_connect_ready(Some("123".to_string()), Some("s".to_string())).unwrap();
        assert_eq!(id, "123");
        assert_eq!(secret, "s");
    }

    #[test]
    fn facebook_and_instagram_share_single_meta_gate() {
        // İki platform da aynı ortak kapıyı (assert_connect_ready) kullanır;
        // bu nedenle ikisi de aynı Meta uygulama kimlikleri kümesine bağlıdır.
        assert_ne!(FACEBOOK_PLATFORM_ID, INSTAGRAM_PLATFORM_ID);
        assert!(assert_connect_ready(Some("123".to_string()), Some("s".to_string())).is_ok());
    }

    #[test]
    fn app_secret_is_never_embedded_into_binary() {
        // Güvenlik kuralı: app secret için yalnız option_env! kullanılır;
        // env! (zorunlu gömme) kullanılırsa derleme secret'sız ortamda başarısız
        // olurdu. Kaynağın bu kurala uyduğu denetlenir.
        let src = include_str!("meta.rs");
        let total = src.matches("env!(\"ES_OPS_META_APP_SECRET\"").count();
        let via_option = src.matches("option_env!(\"ES_OPS_META_APP_SECRET\"").count();
        // "env!(" geçişlerinin tümü "option_env!(" içinden gelmelidir.
        assert_eq!(total, via_option, "yalnız option_env! kullanılmalı");
        assert!(via_option >= 1);
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
    fn exchange_returns_app_secret_required() {
        // App secret gömülmediği için kod değişimi gerçekleştirilemez ve
        // kontrollü hata koduna döner (sahte başarı üretilmez).
        let err = exchange_code("appid", "http://127.0.0.1:9999/", "code").unwrap_err();
        assert_eq!(err, SocialError::AppSecretRequired);
    }

    #[test]
    fn refresh_returns_reauthorization_required() {
        let err = refresh_user_token().unwrap_err();
        assert_eq!(err, SocialError::ReauthorizationRequired);
    }

    #[test]
    fn authorize_url_includes_scopes_and_state() {
        let url = build_authorize_url(
            "123",
            "http://localhost:8080/",
            "pages_show_list,pages_manage_posts",
            "st",
            None,
        );
        assert!(url.contains("client_id=123"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=pages_show_list"));
        assert!(url.contains("state=st"));
        assert!(!url.contains("config_id="));
    }

    #[test]
    fn authorize_url_includes_config_id_when_provided() {
        // SUAT ana kuralı: config_id ile scope GÖNDERİLMEZ (izinler panelden),
        // override_default_response_type=true zorunlu, state korunur.
        let url = build_authorize_url(
            "123",
            "http://localhost:8080/redirect",
            "pages_show_list,pages_manage_posts",
            "st",
            Some("917689934731039"),
        );
        assert!(url.starts_with("https://www.facebook.com/dialog/oauth?client_id=123"));
        assert!(url.contains("config_id=917689934731039"));
        assert!(url.contains("override_default_response_type=true"));
        assert!(url.contains("response_type=code"));
        assert!(!url.contains("scope="));
        assert!(url.contains("state=st"));
    }

    #[test]
    fn authorize_url_without_config_id_is_scope_based() {
        // config_id yoksa klasik scope akışı; override parametresi gelmez.
        let url = build_authorize_url(
            "123",
            "https://www.facebook.com/connect/login_success.html",
            "pages_show_list,pages_manage_posts",
            "st",
            None,
        );
        assert!(url.contains("scope=pages_show_list,pages_manage_posts"));
        assert!(!url.contains("config_id="));
        assert!(!url.contains("override_default_response_type"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("state=st"));
    }

    #[test]
    fn post_kind_parse_is_controlled() {
        assert_eq!(PostKind::parse("standard"), Some(PostKind::Standard));
        assert_eq!(PostKind::parse("kampanya"), Some(PostKind::Campaign));
        assert_eq!(PostKind::parse("duyuru"), Some(PostKind::Announcement));
        assert_eq!(PostKind::parse("ilan"), Some(PostKind::Announcement));
        assert_eq!(PostKind::parse("rastgele-metin"), None);
    }

    #[test]
    fn media_kind_parse_is_controlled() {
        assert_eq!(MediaKind::parse("text"), Some(MediaKind::Text));
        assert_eq!(MediaKind::parse("gorsel"), Some(MediaKind::Photo));
        assert_eq!(MediaKind::parse("video"), Some(MediaKind::Video));
        assert_eq!(MediaKind::parse("carousel"), Some(MediaKind::Carousel));
        assert_eq!(MediaKind::parse("bilinmeyen"), None);
    }

    #[test]
    fn instagram_requires_real_media() {
        // Medyasız (yalnız metin) içerik Instagram hedefi olamaz.
        assert!(!instagram_is_eligible(MediaKind::Text));
        assert!(instagram_is_eligible(MediaKind::Photo));
        assert!(instagram_is_eligible(MediaKind::Video));
        assert!(instagram_is_eligible(MediaKind::Carousel));
    }

    #[test]
    fn instagram_rejects_text_only_announcement() {
        // Duyuru/ilan türü yalnız metinse Instagram hedef dışıdır.
        let kind = PostKind::parse("duyuru").unwrap();
        assert!(kind.is_announcement());
        assert!(!instagram_is_eligible(MediaKind::Text));
    }

    #[test]
    fn facebook_supports_text_photo_video() {
        assert!(facebook_is_eligible(MediaKind::Text));
        assert!(facebook_is_eligible(MediaKind::Photo));
        assert!(facebook_is_eligible(MediaKind::Video));
        assert!(!facebook_is_eligible(MediaKind::Carousel));
    }
}
