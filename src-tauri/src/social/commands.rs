//! Ortak Tauri komutları.
//!
//! Yalnız ortak, platforma özel olmayan işlemleri içerir. Platforma özel
//! OAuth endpointleri, token yenileme kuralları veya paylaşım kuralları
//! bu dosyaya dağıtılmaz. Yeni bir platform eklemek için ortak modellerin
//! baştan yazılması gerekmez.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use super::credential_store;
use super::models::{
    ConnectionStatus, PlatformDefinition, SocialAccountConnection, SocialError, TokenType,
};
use super::platforms::youtube;
use super::platforms::{facebook, instagram, linkedin, meta, pinterest, tiktok, x};
use super::{media_validation, metadata_store, registry};

/// Metadata deposunun kök dizinini uygulama veri klasörü üzerinden hesaplar.
fn data_dir(app: &AppHandle) -> Result<PathBuf, SocialError> {
    app.path()
        .app_data_dir()
        .map_err(|_| SocialError::ConnectionStoreError)
}

/// Kullanıcının sistem dosya seçici (native dialog) ile seçtiği video dosyasının
/// gerçek mutlak yolunu döndürür.
///
/// Tarayıcı/webview güvenliği gereği ön yüz, kullanıcının seçtiği dosyanın gerçek
/// disk yoluna erişemez. Gerçek TikTok/YouTube video yayını için dosyanın diskteki
/// mutlak yoluna ihtiyaç duyduğumuz için bu komut native bir dosya seçici açar ve
/// seçilen geçerli video dosyasının mutlak yolunu döndürür.
///
/// - Kullanıcı dosya seçmez/iptal ederse boş dize döner; sahte isim üretilmez.
/// - Seçilen dosya bilinen bir video uzantısına sahip değilse kontrollü hata döner
///   (yanlış/görsel dosyası kabul edilmez).
/// - Seçim sonrası dosya gerçekten diskte yoksa kontrollü hata döner.
#[tauri::command]
pub fn pick_video_file(app: AppHandle) -> Result<String, SocialError> {
    let picked = app
        .dialog()
        .file()
        .add_filter("Video dosyalari", &["mp4", "mov", "avi", "mkv", "webm", "m4v", "flv", "3gp", "mpeg", "mpg", "ogv", "ts", "wmv"])
        .blocking_pick_file();

    let path = match picked {
        Some(p) => p,
        None => return Ok(String::new()), // kullanıcı iptal etti; sahte isim üretilmez
    };

    // Asynchronously resolved path: mutlak yolu al.
    let abs = path.into_path().map_err(|_| SocialError::InvalidMediaFile)?;

    // Uzantı kontrolü: bilinen bir video uzantısı olması gerekir
    media_validation::verify_video_file(abs.to_str().unwrap_or(""))?;

    Ok(abs.to_string_lossy().into_owned())
}

/// Kullanıcının native dosya seçici ile seçtiği görsel ve/veya video
/// dosyalarının gerçek mutlak yollarını döndürür.
///
/// Tarayıcı/webview güvenliği gereği ön yüz, kullanıcının seçtiği dosyaların
/// gerçek disk yoluna erişemez; gerçek Facebook/Instagram/LinkedIn/Pinterest
/// media yayını için dosyaların diskteki mutlak yollarına ihtiyaç duyduğumuz
/// için bu komut native bir dosya seçici açar ve seçilen geçerli medya
/// dosyalarının (görsel + video) mutlak yollarını döndürür.
///
/// - Kullanıcı dosya seçmez/iptal ederse boş liste döner; sahte isim üretilmez.
/// - Yalnızca bilinen görsel ve video uzantıları kabul edilir; tanınmayan bir
///   dosya seçilirse `invalid_media_file` kontrollü hatası döner.
/// - Her dosya, diskten okunarak uzantıya ek olarak içerik imzasıyla da
///   doğrulanır (yalnız uzantıya güvenilmez).
#[tauri::command]
pub fn pick_media_files(app: AppHandle) -> Result<Vec<String>, SocialError> {
    let picked = app
        .dialog()
        .file()
        .add_filter(
            "Medya dosyalari (gorsel / video)",
            &[
                "jpg", "jpeg", "png", "webp", "gif", "bmp", "mp4", "mov", "avi", "mkv", "webm",
                "m4v", "3gp", "mpg", "mpeg", "ogv", "ts", "wmv", "flv",
            ],
        )
        .blocking_pick_files();

    let Some(files) = picked else {
        return Ok(Vec::new()); // kullanıcı iptal etti; sahte isim üretilmez
    };

    let mut paths = Vec::with_capacity(files.len());
    for file in files {
        let abs = file.into_path().map_err(|_| SocialError::InvalidMediaFile)?;
        let path = abs.to_string_lossy().into_owned();
        let lower = path.to_ascii_lowercase();
        let is_video = [
            "mp4", "mov", "avi", "mkv", "webm", "m4v", "3gp", "mpg", "mpeg", "ogv", "ts", "wmv",
            "flv",
        ]
        .iter()
        .any(|ext| lower.ends_with(&format!(".{}", ext)));
        if is_video {
            media_validation::verify_video_file(&path)?;
        } else {
            media_validation::verify_image_or_photo_file(&path)?;
        }
        paths.push(path);
    }
    Ok(paths)
}

/// Platform kataloğunu ve destek durumlarını döndürür.
/// Gizli bilgi veya token döndürmez.
#[tauri::command]
pub fn social_platform_catalog() -> Vec<PlatformDefinition> {
    registry::platform_catalog()
}

/// Kalıcı veri katmanındaki gerçek hesap bağlantılarını döndürür.
/// Her kayıt için token_exists değeri güvenli depodan hesaplanır.
/// Hesap bulunmuyorsa boş liste döner; sahte hesap üretilmez.
#[tauri::command]
pub fn social_account_connections(
    app: AppHandle,
) -> Result<Vec<SocialAccountConnection>, SocialError> {
    let dir = data_dir(&app)?;
    let records = metadata_store::list_connections(&dir)?;

    let mut result = Vec::with_capacity(records.len());
    for record in records {
        let token_exists =
            credential_store::token_exists(&record.platform_id, &record.connection_id, TokenType::AccessToken)
                .map_err(|_| SocialError::CredentialStoreError)?;
        result.push(record.to_public(token_exists));
    }
    Ok(result)
}

/// Belirtilen bağlantı kaydını bulur.
/// token_exists değeri güvenli depodan hesaplanır; ham token döndürülmez.
/// Kayıt bulunamazsa kontrollü hata sonucu döner.
#[tauri::command]
pub fn social_account_status(
    app: AppHandle,
    connection_id: String,
) -> Result<SocialAccountConnection, SocialError> {
    if connection_id.trim().is_empty() {
        return Err(SocialError::InvalidConnection);
    }
    let dir = data_dir(&app)?;
    let record =
        metadata_store::get_connection(&dir, &connection_id)?.ok_or(SocialError::InvalidConnection)?;

    let token_exists =
        credential_store::token_exists(&record.platform_id, &record.connection_id, TokenType::AccessToken)
            .map_err(|_| SocialError::CredentialStoreError)?;
    Ok(record.to_public(token_exists))
}

/// Bağlantı kesme işleminin kontrollü sonuç durumu.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisconnectStatus {
    /// Bağlantı başarıyla kesildi.
    Disconnected,
    /// Hesap zaten bağlı değildi.
    NotConnected,
    /// Bağlantı kaydı bulunamadı.
    NotFound,
    /// İşlem başarısız oldu.
    Failed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectResult {
    pub status: DisconnectStatus,
}

/// Belirtilen connection_id'ye ait bağlantıyı keser.
///
/// Davranış kuralları:
/// - Bağlantı kaydı bulunmazsa sahte başarı döndürülmez (`not_found`).
/// - Hesap zaten bağlı değilse sahte bağlantı kesme başarısı döndürülmez (`not_connected`).
/// - Bağlantı kesilirken önce yalnız bu bağlantıya ait tüm token türleri silinir.
/// - Token silme veya metadata güncelleme başarısız olursa tam başarı döndürülmez (`failed`).
#[tauri::command]
pub fn social_disconnect_account(
    app: AppHandle,
    connection_id: String,
) -> Result<DisconnectResult, SocialError> {
    if connection_id.trim().is_empty() {
        // Boş id: kayıt yok anlamında kontrollü döndür
        return Ok(DisconnectResult {
            status: DisconnectStatus::NotFound,
        });
    }

    let dir = data_dir(&app)?;
    let record =
        match metadata_store::get_connection(&dir, &connection_id)? {
            Some(rec) => rec,
            None => {
                return Ok(DisconnectResult {
                    status: DisconnectStatus::NotFound,
                })
            }
        };

    // Hesap zaten bağlı değilse veya erişim tokenı mevcut değilse
    // işlem yapılmadan "not_connected" döndürülür (sahte başarı gösterilmez).
    let has_token = credential_store::token_exists(
        &record.platform_id,
        &connection_id,
        TokenType::AccessToken,
    )
    .map_err(|_| SocialError::CredentialStoreError)?;
    if record.connection_status == ConnectionStatus::Disconnected || !has_token {
        return Ok(DisconnectResult {
            status: DisconnectStatus::NotConnected,
        });
    }

    // Token silme hatası başarısızlık sayılır.
    if credential_store::delete_all_tokens(&record.platform_id, &connection_id).is_err() {
        return Ok(DisconnectResult {
            status: DisconnectStatus::Failed,
        });
    }

    // Metadata kaydını disconnected yap. Bu adım başarısız olursa tam başarı döndürme.
    if metadata_store::update_connection_status(&dir, &connection_id, ConnectionStatus::Disconnected)
        .is_err()
    {
        return Ok(DisconnectResult {
            status: DisconnectStatus::Failed,
        });
    }

    Ok(DisconnectResult {
        status: DisconnectStatus::Disconnected,
    })
}

/// YouTube OAuth bağlantı sonucu için serilerlenebilir sonuç görünümü.
/// İçeride yalnız kamuya açık bilgi barınır; ham token döndürülmez.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeConnectResult {
    pub connection: SocialAccountConnection,
}

/// YouTube'a gerçek OAuth akışıyla bağlanır.
///
/// - Google'ın masaüstü uygulama OAuth 2.0 akışını uygular (PKCE-S256, state).
/// - Tarayıcıyı resmî Tauri shell mekanizmasıyla açar.
/// - Sonuç gerçek bir `SocialAccountConnection` döndürür; hiçbir durumda
///   sahte bağlantı, sahte kanal adı veya yer tutucu üretilmez.
#[tauri::command]
pub fn youtube_connect(
    app: AppHandle,
) -> Result<YoutubeConnectResult, SocialError> {
    let connection = youtube::connect(&app)?;
    Ok(YoutubeConnectResult { connection })
}

/// Bir YouTube hesabına gerçek `videos.insert` (resumable) ile video yükler.
///
/// Zorunlu alanlar: bağlı bağlantı id, gerçek video dosya yolu, başlık,
/// gizlilik. Açıklama isteğe bağlıdır (boş olabilir). Gerçek video id döner.
///
/// Gizlilik kontrollü değerlerle gelir (`private`, `unlisted`, `public`);
/// JavaScript'ten serbest metinle belirlenmez.
#[tauri::command]
pub fn youtube_upload_video(
    app: AppHandle,
    connection_id: String,
    video_path: String,
    title: String,
    description: String,
    privacy: String,
) -> Result<String, SocialError> {
    let privacy_status = youtube::PrivacyStatus::parse(&privacy)
        .ok_or(SocialError::UnsupportedPostType)?;
    youtube::upload_video(&app, &connection_id, &video_path, &title, &description, privacy_status)
}

// ----------------------------------------------------------------------
// Meta (Facebook / Instagram) bağlantı ve yayın komutları
// ----------------------------------------------------------------------

/// Facebook OAuth bağlantı sonucu (başarı durumunda).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FacebookConnectResult {
    pub connection: SocialAccountConnection,
}

/// Meta OAuth akışını yürütür ve bağlı bağlantı kaydını döndürür.
///
/// Ortak akış adımları:
/// 1. Yapılandırılmış App ID / App Secret'ı çöz (güvenli depodan okunur; kaynağa gömülmez).
/// 2. Loopback listener açar, `state` üretir ve resmî Facebook yetkilendirme
///    sayfasını sistem tarayıcısında açar.
/// 3. Callback'ten yetkilendirme kodunu ve state'i alır; state eşleşmesini doğrular.
/// 4. Kodu gerçek Meta token endpoint'inde App Secret ile token'a çevirir.
/// 5. Token'la yönetilen Facebook Sayfalarını keşfeder.
///
/// `pin_instagram`: `true` ise Sayfaya bağlı Instagram profesyonel hesabı da bağlanır.
///
/// Başarısız adımlarda kontrollü hata kodu döner; sahte bağlantı/token üretilmez.
fn run_meta_connect_flow(
    app: &AppHandle,
    pin_instagram: bool,
) -> Result<SocialAccountConnection, SocialError> {
    // Ortak yapılandırma kapısı: App ID / App Secret eksikse akış, tarayıcı
    // açılmadan önce tek bir kontrollü hata koduyla durur (sahte bağlantı
    // üretilmez). Facebook ve Instagram bu kapıyı paylaşır.
    let (app_id, app_secret) =
        meta::assert_connect_ready(meta::resolved_app_id(), meta::resolved_app_secret())?;

    let listener = meta::bind_loopback()?;
    // Meta redirect URI sabittir ve App Dashboard'undaki "Valid OAuth Redirect
    // URIs" listesine bu tam adresle kaydedilir. Dinamik port kullanılmaz.
    // Meta, IP adresi içeren redirect URI'lerini (ör. `127.0.0.1`) reddeder;
    // bu yüzden etki adı `localhost` kullanılır (loopback dinleyici aynı hedef).
    let redirect_uri = "http://localhost:43123/meta-callback".to_string();
    let state = meta::generate_state()?;

    let scope = super::platforms::facebook::SCOPES;
    let auth_url = meta::build_authorize_url(
        &app_id,
        &redirect_uri,
        scope,
        &state,
        meta::meta_config_id(),
    );
    meta::open_browser(app, &auth_url)?;

    let (code, callback_state) = meta::wait_for_callback(&listener)?;
    if callback_state != state {
        return Err(SocialError::OauthStateMismatch);
    }

    let user_token = meta::exchange_code_real(&app_id, &redirect_uri, &code, &app_secret)?;

    let pages = meta::fetch_managed_pages(&user_token)?;
    let page = pages.first().ok_or(SocialError::NoManagedPage)?;

    if pin_instagram {
        let ig = meta::fetch_linked_instagram(&page.page_id, &page.page_access_token)?;
        meta::connect_instagram_account(app, &page.page_id, &ig, &page.page_access_token)
    } else {
        meta::connect_facebook_page(app, page)
    }
}

/// Facebook için gerçek Meta OAuth akışını başlatır ve Sayfa bağlantısını kurar.
///
/// Akış, ortak `meta` motorunu ve (varsa) güvenli depodaki App ID/Search
/// kullanır. App Secret saklanmamışsa `app_secret_required` kontrollü hatası
/// döner; tarayıcı anlamsız bir oturum için açılmaz. Başarıda gerçek `connection`
/// döner. Sahte bağlantı, sahte Sayfa adı veya yer tutucu token üretilmez.
#[tauri::command]
pub fn facebook_connect(app: AppHandle) -> Result<FacebookConnectResult, SocialError> {
    let connection = run_meta_connect_flow(&app, false)?;
    Ok(FacebookConnectResult { connection })
}

/// Instagram OAuth bağlantı sonucu (başarı durumunda).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstagramConnectResult {
    pub connection: SocialAccountConnection,
}

/// Instagram için gerçek Meta OAuth akışını başlatır ve Sayfaya bağlı
/// Instagram profesyonel hesabını bağlar.
///
/// Instagram, bağlı bir Facebook Sayfası üzerinden yönetilir; bu akış aynı
/// ortak `meta` motorunu kullanır. App Secret güvenli depoda yoksa
/// `app_secret_required` kontrollü hatası döner; başarıda gerçek `connection`
/// döner. Sahte bağlantı veya yer tutucu token üretilmez.
#[tauri::command]
pub fn instagram_connect(app: AppHandle) -> Result<InstagramConnectResult, SocialError> {
    let connection = run_meta_connect_flow(&app, true)?;
    Ok(InstagramConnectResult { connection })
}

/// Meta bağlantı yapılandırma durumu (gizli bilgi içermez).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaConfigStatus {
    pub app_id_configured: bool,
    pub app_secret_configured: bool,
}

/// Meta App ID / App Secret'in güvenli depoda yapılandırılıp yapılandırılmadığını
/// döndürür. Ham secret asla döndürülmez.
#[tauri::command]
pub fn meta_config_status() -> Result<MetaConfigStatus, SocialError> {
    let has_secret = meta::resolved_app_secret().is_some();
    let has_id = meta::resolved_app_id().is_some();
    Ok(MetaConfigStatus {
        app_id_configured: has_id,
        app_secret_configured: has_secret,
    })
}

/// Meta App ID ve App Secret'ı güvenli depoya (Windows Credential Manager)
/// yazar. Değerler kaynağa gömülmez; yalnız şifreli depoda saklanır. Boş değer
/// kabul edilmez.
#[tauri::command]
pub fn meta_set_config(
    app_id: String,
    app_secret: String,
) -> Result<MetaConfigStatus, SocialError> {
    if app_id.trim().is_empty() || app_secret.trim().is_empty() {
        return Err(SocialError::MetaNotConfigured);
    }
    meta::store_app_id(&app_id)?;
    meta::store_app_secret(&app_secret)?;
    Ok(MetaConfigStatus {
        app_id_configured: true,
        app_secret_configured: true,
    })
}

/// Güvenli depodaki Meta yapılandırmasını (App ID / App Secret) temizler.
#[tauri::command]
pub fn meta_clear_config() -> Result<(), SocialError> {
    credential_store::delete_all_tokens("meta", "_meta_app_config")
}

/// Facebook Sayfaya yayın yapar ve gerçek post kimliğini döndürür.
///
/// Girdiler mevcut yayın motorunun kontrollü veri modelinden gelir; JS'ten
/// serbest medya / platform metni kabul edilmez. `media_kind` ve `post_kind`
/// kontrollü değerlerdir. Yerel görsel/video `media_files` ile diskten multipart
/// yüklenir; herkese açık medya URL'si gerekmez.
#[tauri::command]
pub fn facebook_publish(
    app: AppHandle,
    connection_id: String,
    message: String,
    title: String,
    media_kind: String,
    media_files: Vec<String>,
) -> Result<String, SocialError> {
    let media_kind = if media_kind.trim().is_empty() {
        None
    } else {
        Some(meta::MediaKind::parse(&media_kind).ok_or(SocialError::UnsupportedPostType)?)
    };

    let input = facebook::FacebookPostInput {
        connection_id,
        message,
        media_kind,
        media_files,
        title,
    };
    facebook::publish(&app, &input)
}

/// Instagram hesabına yayın yapar ve gerçek medya kimliğini döndürür.
///
/// Instagram, medya container'ında herkese açık `image_url` / `video_url` ister.
/// Bu sunucusuz masaüstü mimarisinde herkese açık medya URL'si üreten bir
/// barındırma hizmeti olmadığı için `media_url_unavailable` döner (sahte URL
/// üretilmez, yeni sunucu kurulmaz).
#[tauri::command]
pub fn instagram_publish(
    app: AppHandle,
    connection_id: String,
    caption: String,
    media_kind: String,
    media_urls: Vec<String>,
    post_kind: String,
) -> Result<String, SocialError> {
    let media_kind = if media_kind.trim().is_empty() {
        None
    } else {
        Some(meta::MediaKind::parse(&media_kind).ok_or(SocialError::UnsupportedPostType)?)
    };
    let post_kind = if post_kind.trim().is_empty() {
        None
    } else {
        Some(meta::PostKind::parse(&post_kind).ok_or(SocialError::UnsupportedPostType)?)
    };

    let input = instagram::InstagramPostInput {
        connection_id,
        caption,
        media_kind,
        media_urls,
        post_kind,
    };
    instagram::publish(&app, &input)
}

// ----------------------------------------------------------------------
// TikTok (Content Posting API) bağlantı ve yapılandırma komutları
// ----------------------------------------------------------------------

/// TikTok OAuth bağlantı sonucu (başarı durumunda).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TiktokConnectResult {
    pub connection: SocialAccountConnection,
}

/// TikTok'a gerçek OAuth akışıyla bağlanır.
///
/// - TikTok Content Posting API'nin resmî OAuth akışını uygular.
/// - Tarayıcıyı resmî Tauri shell mekanizmasıyla açar.
/// - Sonuç gerçek bir `SocialAccountConnection` döndürür; hiçbir durumda
///   sahte bağlantı veya yer tutucu token üretilmez.
#[tauri::command]
pub fn tiktok_connect(app: AppHandle) -> Result<TiktokConnectResult, SocialError> {
    let connection = tiktok::connect(&app)?;
    Ok(TiktokConnectResult { connection })
}

/// TikTok bağlantı yapılandırma durumu (gizli bilgi içermez).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TiktokConfigStatus {
    pub client_key_configured: bool,
    pub client_secret_configured: bool,
}

/// TikTok Client Key / Client Secret'in güvenli depoda yapılandırılıp
/// yapılandırılmadığını döndürür. Ham secret asla döndürülmez.
#[tauri::command]
pub fn tiktok_config_status() -> Result<TiktokConfigStatus, SocialError> {
    let (has_key, has_secret) = tiktok::config_status()?;
    Ok(TiktokConfigStatus {
        client_key_configured: has_key,
        client_secret_configured: has_secret,
    })
}

/// TikTok Client Key ve Client Secret'ı güvenli depoya (Windows Credential
/// Manager) yazar. Değerler kaynağa gömülmez; yalnız şifreli depoda saklanır.
/// Boş değer kabul edilmez.
#[tauri::command]
pub fn tiktok_set_config(
    client_key: String,
    client_secret: String,
) -> Result<TiktokConfigStatus, SocialError> {
    if client_key.trim().is_empty() || client_secret.trim().is_empty() {
        return Err(SocialError::TiktokNotConfigured);
    }
    tiktok::store_client_key(&client_key)?;
    tiktok::store_client_secret(&client_secret)?;
    Ok(TiktokConfigStatus {
        client_key_configured: true,
        client_secret_configured: true,
    })
}

/// Güvenli depodaki TikTok yapılandırmasını (Client Key / Client Secret) temizler.
#[tauri::command]
pub fn tiktok_clear_config() -> Result<(), SocialError> {
    tiktok::clear_config()
}

/// TikTok bağlantısına gerçek Content Posting API ile video yayınlar.
///
/// Zorunlu alanlar: bağlı bağlantı id, gerçek video dosya yolu, başlık,
/// gizlilik düzeyi. Gizlilik kontrollü değerlerle gelir
/// (`PUBLIC_TO_EVERYONE` / `SELF_ONLY`); JavaScript'ten serbest metinle
/// belirlenmez. Gerçek yayın id döner; sahte veya yer tutucu üretilmez.
#[tauri::command]
pub fn tiktok_publish(
    app: AppHandle,
    connection_id: String,
    video_path: String,
    title: String,
    privacy_level: String,
) -> Result<String, SocialError> {
    let privacy = tiktok::PrivacyLevel::parse(&privacy_level)
        .ok_or(SocialError::UnsupportedPostType)?;
    tiktok::publish_video(&app, &connection_id, &video_path, &title, privacy)
}

// ----------------------------------------------------------------------
// X (Twitter) komutları
// ----------------------------------------------------------------------

/// X bağlantı sonucu (serileştirilebilir).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XConnectResult {
    pub connection: SocialAccountConnection,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XConfigStatus {
    pub consumer_key_configured: bool,
    pub consumer_secret_configured: bool,
}

/// X hesabına gerçek OAuth 1.0a akışıyla bağlanır.
#[tauri::command]
pub fn x_connect(app: AppHandle) -> Result<XConnectResult, SocialError> {
    let connection = x::connect(&app)?;
    Ok(XConnectResult { connection })
}

/// X Consumer Key / Secret güvenli depoda yapılandırılmış mı?
#[tauri::command]
pub fn x_config_status() -> Result<XConfigStatus, SocialError> {
    let (has_key, has_secret) = x::config_status()?;
    Ok(XConfigStatus {
        consumer_key_configured: has_key,
        consumer_secret_configured: has_secret,
    })
}

/// X Consumer Key ve Consumer Secret'i güvenli depoya yazar.
#[tauri::command]
pub fn x_set_config(consumer_key: String, consumer_secret: String) -> Result<XConfigStatus, SocialError> {
    if consumer_key.trim().is_empty() || consumer_secret.trim().is_empty() {
        return Err(SocialError::XNotConfigured);
    }
    x::store_consumer_key(&consumer_key)?;
    x::store_consumer_secret(&consumer_secret)?;
    Ok(XConfigStatus {
        consumer_key_configured: true,
        consumer_secret_configured: true,
    })
}

/// Güvenli depodaki X yapılandırmasını temizler.
#[tauri::command]
pub fn x_clear_config() -> Result<(), SocialError> {
    x::clear_config()
}

/// X hesabına gerçek API ile video/görsel yayınlar (media upload + tweet.create).
#[tauri::command]
pub fn x_publish(
    app: AppHandle,
    connection_id: String,
    video_path: String,
    title: String,
) -> Result<String, SocialError> {
    x::publish_video(&app, &connection_id, &video_path, &title)
}

// ----------------------------------------------------------------------
// LinkedIn (Posts API / Images API / Videos API) komutları
// ----------------------------------------------------------------------

/// LinkedIn bağlantı sonucu. Kişisel profil her zaman bağlanır; yayın
/// yapılabilir şirket sayfaları ayrı bağlantılar olarak eklenir.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedinConnectResult {
    pub connections: Vec<SocialAccountConnection>,
}

/// LinkedIn'e gerçek Native PKCE OAuth akışıyla bağlanır.
///
/// - Client Secret kullanılmaz; yalnız Client ID yeterlidir.
/// - Kişisel profile ek olarak, kullanıcının `ADMINISTRATOR`, `CONTENT_ADMIN`
///   veya `DIRECT_SPONSORED_CONTENT_POSTER` rolüne sahip olduğu şirket
///   sayfaları keşfedilir ve ayrı bağlantı olarak döndürülür.
#[tauri::command]
pub fn linkedin_connect(app: AppHandle) -> Result<LinkedinConnectResult, SocialError> {
    let connections = linkedin::connect(&app)?;
    Ok(LinkedinConnectResult { connections })
}

/// LinkedIn bağlantı yapılandırma durumu (gizli bilgi içermez).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedinConfigStatus {
    pub client_id_configured: bool,
}

/// LinkedIn Client ID'nin yapılandırılıp yapılandırılmadığını döndürür.
/// Bu entegrasyonda Client Secret kavramı yoktur; secret asla istenmez/saklanmaz.
#[tauri::command]
pub fn linkedin_config_status() -> Result<LinkedinConfigStatus, SocialError> {
    let has_id = linkedin::resolved_client_id().is_some();
    Ok(LinkedinConfigStatus {
        client_id_configured: has_id,
    })
}

/// LinkedIn Client ID'yi güvenli depoya (Windows Credential Manager) yazar.
/// Client ID gizli bilgi değildir; Client Secret alanı kasıtlı olarak yoktur.
/// Boş değer kabul edilmez.
#[tauri::command]
pub fn linkedin_set_config(client_id: String) -> Result<LinkedinConfigStatus, SocialError> {
    if client_id.trim().is_empty() {
        return Err(SocialError::LinkedinNotConfigured);
    }
    linkedin::store_client_id(&client_id)?;
    Ok(LinkedinConfigStatus {
        client_id_configured: true,
    })
}

/// Güvenli depodaki LinkedIn Client ID yapılandırmasını temizler.
#[tauri::command]
pub fn linkedin_clear_config() -> Result<(), SocialError> {
    linkedin::clear_config()
}

/// LinkedIn hedefine gerçek Posts API ile yayın yapar ve gerçek post
/// URN'sini döndürür.
///
/// Girdiler mevcut yayın motorunun kontrollü veri modelinden gelir; JS'ten
/// serbest medya / platform metni kabul edilmez. `media_kind` kontrollü
/// değerdir. Metin doğrudan `/rest/posts` ile; görsel Images API, video
/// Videos API (parçalı yükleme + işleme yoklaması) ile yüklenip aynı Posts
/// API'de yayınlanır. Eski UGC/Share API kullanılmaz.
#[tauri::command]
pub fn linkedin_publish(
    app: AppHandle,
    connection_id: String,
    message: String,
    title: String,
    media_kind: String,
    media_files: Vec<String>,
) -> Result<String, SocialError> {
    let media_kind = if media_kind.trim().is_empty() {
        None
    } else {
        Some(meta::MediaKind::parse(&media_kind).ok_or(SocialError::UnsupportedPostType)?)
    };

    let input = linkedin::LinkedinPostInput {
        connection_id,
        message,
        title,
        media_kind,
        media_files,
    };
    linkedin::publish(&app, &input)
}

// ----------------------------------------------------------------------
// Pinterest (API v5) komutları
// ----------------------------------------------------------------------

/// Pinterest bağlantı sonucu. Her pano ayrı bir hedef bağlantı olarak eklenir.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinterestConnectResult {
    pub connections: Vec<SocialAccountConnection>,
}

/// Pinterest'e gerçek OAuth akışıyla bağlanır.
///
/// - Client ID + Client Secret güvenli depoda (veya derleme zamanında) hazır
///   olmalıdır; ikisinden biri eksikse `pinterest_not_configured` döner.
/// - Bağlantıda hesabın panoları keşfedilir ve her pano ayrı bir hedef
///   bağlantı olarak döndürülür (dış hesap kimliği = pano id).
#[tauri::command]
pub fn pinterest_connect(app: AppHandle) -> Result<PinterestConnectResult, SocialError> {
    let connections = pinterest::connect(&app)?;
    Ok(PinterestConnectResult { connections })
}

/// Pinterest bağlantı yapılandırma durumu (gizli bilgi içermez).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinterestConfigStatus {
    pub client_id_configured: bool,
    pub client_secret_configured: bool,
}

/// Pinterest Client ID / Client Secret'in güvenli depoda yapılandırılıp
/// yapılandırılmadığını döndürür. Ham secret asla döndürülmez.
#[tauri::command]
pub fn pinterest_config_status() -> Result<PinterestConfigStatus, SocialError> {
    let (has_id, has_secret) = pinterest::config_status()?;
    Ok(PinterestConfigStatus {
        client_id_configured: has_id,
        client_secret_configured: has_secret,
    })
}

/// Pinterest Client ID ve Client Secret'ı güvenli depoya (Windows Credential
/// Manager) yazar. Değerler kaynağa gömülmez; yalnız şifreli depoda saklanır.
/// Boş değer kabul edilmez.
#[tauri::command]
pub fn pinterest_set_config(
    client_id: String,
    client_secret: String,
) -> Result<PinterestConfigStatus, SocialError> {
    if client_id.trim().is_empty() || client_secret.trim().is_empty() {
        return Err(SocialError::PinterestNotConfigured);
    }
    pinterest::store_client_id(&client_id)?;
    pinterest::store_client_secret(&client_secret)?;
    Ok(PinterestConfigStatus {
        client_id_configured: true,
        client_secret_configured: true,
    })
}

/// Güvenli depodaki Pinterest yapılandırmasını (Client ID / Client Secret) temizler.
#[tauri::command]
pub fn pinterest_clear_config() -> Result<(), SocialError> {
    pinterest::clear_config()
}

/// Pinterest panosuna gerçek v5 API ile pin oluşturur ve gerçek pin id döndürür.
///
/// Girdiler mevcut yayın motorunun kontrollü veri modelinden gelir; JS'ten
/// serbest medya kabul edilmez. `media_kind` kontrollü değerdir. Yerel JPG/PNG
/// `image_base64` ile, çoklu görsel `multiple_image_base64` ile, video ise
/// resmî media upload akışıyla doğrudan yüklenir; herkese açık URL gerekmez.
#[tauri::command]
pub fn pinterest_publish(
    app: AppHandle,
    connection_id: String,
    message: String,
    title: String,
    media_kind: String,
    media_files: Vec<String>,
) -> Result<String, SocialError> {
    let media_kind = if media_kind.trim().is_empty() {
        None
    } else {
        Some(meta::MediaKind::parse(&media_kind).ok_or(SocialError::UnsupportedPostType)?)
    };

    let input = pinterest::PinterestPostInput {
        connection_id,
        message,
        title,
        media_kind,
        media_files,
    };
    pinterest::publish(&app, &input)
}
