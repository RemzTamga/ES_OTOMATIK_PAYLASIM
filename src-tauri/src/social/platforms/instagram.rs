//! Instagram (Business/Creator) entegrasyon modülü.
//!
//! Yalnız bağlı Instagram işletme/yazar (profesyonel) hesaplarına yayın yapar.
//! Kişisel Instagram profillerine ve medyasız (yalnız metin) içeriğe yayın
//! yapılmaz. Instagram hedefi yalnız gerçek medya içeren içerik için geçerlidir.
//!
//! Medya barındırma: Instagram Graph API, `/media` container'ında `image_url` /
//! `video_url` olarak *herkese açık* bir medya URL'si ister; yerel diskten
//! doğrudan dosya yüklemesini resmî olarak desteklemez. Bu uygulamanın kalıcı
//! bir medya sunucusu yoktur; görsel/video, yayın anında ücretsiz anonim
//! barındırmaya (0x0.st) yüklenir (`media_host` modülü), yayın sonrası token
//! ile hemen silinir ve `expires=1` saati + TTL temizliği ikinci güvence olur.
//!
//! Teşhis, hat eşleştirme, container adımları ve sıra koruması gerçek API
//! yapısına uygun olarak burada tanımlıdır. Kullanıcı herkese açık bir URL
//! verirse doğrudan kullanılır; ne URL ne yerel dosya varsa `MediaUrlUnavailable`
//! döner (sahte yayın başarısı üretilmez).

use tauri::{AppHandle, Manager};

use super::super::credential_store;
use super::super::metadata_store;
use super::super::models::{ConnectionRecord, ConnectionStatus, SocialError, TokenType};
use super::meta::{self, MediaKind, PostKind};

/// Instagram platform kimliği.
pub const PLATFORM_ID: &str = "instagram";

/// Meta Instagram için OAuth izinleri (yalnız gereken en dar izinler).
/// Sayfaya bağlı Instagram işletme hesabı için `instagram_basic`,
/// `instagram_content_publish` ve `pages_show_list` gerekir.
pub const SCOPES: &str =
    "instagram_basic,instagram_content_publish,pages_show_list";

/// Instagram içerik yayın girdisi. Yalnız mevcut yayın motorunun kontrollü
/// modelinden alınır; JavaScript'ten serbest metin gelmez.
pub struct InstagramPostInput {
    pub connection_id: String,
    pub caption: String,
    pub media_kind: Option<MediaKind>,
    /// MediaContainer'da kullanılacak herkese açık medya URL'leri.
    /// Boşsa ve yerel medya dosyası da yoksa `media_url_unavailable` döner.
    pub media_urls: Vec<String>,
    /// Yerel diskteki gerçek medya dosya yolları. Doluysa dosya geçici
    /// barındırmaya (0x0.st) yüklenip herkese açık URL üretilir (sunucusuz çözüm).
    pub media_files: Vec<String>,
    /// Mevcut proje modelindeki kontrollü paylaşım türü (hedef seçiminde).
    pub post_kind: Option<PostKind>,
}

fn data_dir(app: &AppHandle) -> Result<std::path::PathBuf, SocialError> {
    app.path()
        .app_data_dir()
        .map_err(|_| SocialError::ConnectionStoreError)
}

fn obtain_page_token(
    _app: &AppHandle,
    record: &ConnectionRecord,
) -> Result<String, SocialError> {
    if record.connection_status != ConnectionStatus::Connected {
        return Err(SocialError::InvalidConnection);
    }
    credential_store::get_token(
        &record.platform_id,
        &record.connection_id,
        TokenType::AccessToken,
    )
    .map_err(|_| SocialError::CredentialStoreError)?
    .filter(|t| !t.is_empty())
    .ok_or(SocialError::TokenMissing)
}

/// Instagram yayını: media container oluşturur ve yayınlar.
///
/// Gerçek Graph API akışı:
/// 1. `POST /{ig-id}/media` → container id (image_url / video_url gerekir)
/// 2. Yayına hazır olması beklenir (tarama)
/// 3. `POST /{ig-id}/media_publish` → gerçek medya id
///
/// Herkese açık medya URL'si (resim/video) `image_url`/`video_url` parametresiyle
/// gerekir. Bu mimaride herkese açık URL üretilemediği için ilgili adım
/// `MediaUrlUnavailable` ile döner; sahte medya id üretilmez.
pub fn publish(
    app: &AppHandle,
    input: &InstagramPostInput,
) -> Result<String, SocialError> {
    if input.connection_id.trim().is_empty() {
        return Err(SocialError::InvalidConnection);
    }

    let dir = data_dir(app)?;
    let record = metadata_store::get_connection(&dir, &input.connection_id)?
        .ok_or(SocialError::InvalidConnection)?;
    if record.platform_id != PLATFORM_ID {
        return Err(SocialError::InvalidConnection);
    }

    // Instagram yalnız gerçek medya içeriğini hedefler; medyasız metin (örn.
    // duyuru/ilan) Instagram'a gönderilmez.
    let media = input.media_kind.unwrap_or(MediaKind::Text);
    if !meta::instagram_is_eligible(media) {
        return Err(SocialError::UnsupportedPostType);
    }

    let access_token = match obtain_page_token(app, &record) {
        Ok(t) => t,
        Err(e) => {
            update_status(app, &record.connection_id, ConnectionStatus::TokenExpired);
            return Err(e);
        }
    };

    let ig_user_id = &record.external_account_id;

    let client = meta::http_client()?;

    // Medya kaynağını belirle: önce yerel dosya → geçici barındırma (0x0.st),
    // yoksa kullanıcı tarafından verilen herkese açık URL.
    let data_dir = data_dir(app)?;
    let (media_url, uploaded_token) = resolve_media_url(&data_dir, input, media)?;

    let container_id = create_container(&client, ig_user_id, &media_url, &input.caption, media, &access_token)?;

    // Container hazır olana kadar durum yoklanır; hata olursa kontrollü döner.
    let _ = wait_container_ready(&client, &container_id, &access_token)?;

    let publish_url = format!(
        "https://graph.facebook.com/{}/{}/media_publish",
        meta::META_GRAPH_VERSION,
        ig_user_id
    );
    let form = reqwest::blocking::multipart::Form::new()
        .text("creation_id", container_id.clone())
        .text("access_token", access_token.clone());
    let resp = client
        .post(&publish_url)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::PublishFailed)?;
    let ok = resp.status().is_success();
    let body = resp.text().unwrap_or_default();
    if !ok {
        return Err(map_container_error(&body));
    }

    // Yayın başarılı: geçici dosya artık gerekmez; upload token'ı ile hemen
    // silinir. (İkinci güvence: silme başarısız olsa bile `expires=1` saati ve
    // TTL temizliği devreye girer.)
    if let Some(token) = uploaded_token {
        cleanup_uploaded(&data_dir, &token);
    }
    read_id_from_body(&body).ok_or(SocialError::PublishFailed)
}

/// Yerel dosyayı geçici barındırmaya (0x0.st) yükleyip herkese açık URL üretir;
/// kullanıcı URL'si varsa onu kullanır. Dönen ikinci değer, temizlenecek
/// upload token'ıdır (barındırma kullanılmadıysa `None`).
fn resolve_media_url(
    data_dir: &std::path::Path,
    input: &InstagramPostInput,
    media: MediaKind,
) -> Result<(String, Option<String>), SocialError> {
    // 1) Kullanıcı herkese açık URL verdiyse doğrudan kullan.
    if let Some(u) = input.media_urls.first() {
        let trimmed = u.trim().to_string();
        if !trimmed.is_empty() {
            return Ok((trimmed, None));
        }
    }

    // 2) Yerel dosya varsa 0x0.st'ye yükle.
    let path = match input.media_files.first() {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => return Err(SocialError::MediaUrlUnavailable),
    };

    // Dosya gerçekten geçerli bir görsel/video mu (yalnız uzantıya güvenme).
    match media {
        MediaKind::Photo => super::super::media_validation::verify_image_or_photo_file(&path)?,
        MediaKind::Video => super::super::media_validation::verify_video_file(&path)?,
        _ => return Err(SocialError::UnsupportedPostType),
    }

    // Geçmiş denemelerden kalan eski geçici dosyaları önce temizle (ikinci güvence).
    let _ = super::super::media_host::cleanup_stale(data_dir);

    let (url, token) = super::super::media_host::upload_media(data_dir, &path, None)
        .map_err(|_| SocialError::MediaHostUploadFailed)?;
    Ok((url, Some(token)))
}

/// Yayın sonrası geçici dosyayı token ile siler; hatalar sessizce yok sayılır
/// (TTL temizliği yedek güvence olduğu için yayın akışını bozmaz).
fn cleanup_uploaded(data_dir: &std::path::Path, token: &str) {
    let _ = super::super::media_host::delete_media(data_dir, token);
}

/// Container yayına hazır olana kadar durum yoklar (videolar için bekleme
/// gerekebilir; görsellerde genellikle anında hazır olur).
fn wait_container_ready(
    client: &reqwest::blocking::Client,
    container_id: &str,
    access_token: &str,
) -> Result<(), SocialError> {
    use std::thread::sleep;
    let url = format!(
        "https://graph.facebook.com/{}/{}?fields=status_code",
        meta::META_GRAPH_VERSION,
        container_id
    );
    for _ in 0..30 {
        let resp = client
            .get(&url)
            .query(&[("access_token", access_token)])
            .send()
            .map_err(|_| SocialError::MediaContainerFailed)?;
        let body = resp.text().unwrap_or_default();
        if body.contains("FINISHED") {
            return Ok(());
        }
        if body.contains("ERROR") || body.contains("EXPIRED") {
            return Err(SocialError::MediaContainerFailed);
        }
        sleep(std::time::Duration::from_secs(2));
    }
    Err(SocialError::MediaProcessingTimeout)
}

/// `POST /{ig-id}/media` ile bir container oluşturur.
fn create_container(
    client: &reqwest::blocking::Client,
    ig_user_id: &str,
    media_url: &str,
    caption: &str,
    media: MediaKind,
    access_token: &str,
) -> Result<String, SocialError> {
    let url = format!(
        "https://graph.facebook.com/{}/{}/media",
        meta::META_GRAPH_VERSION,
        ig_user_id
    );

    let kind_params = match media {
        MediaKind::Photo => vec![("image_url", media_url.to_string())],
        MediaKind::Video => vec![("video_url", media_url.to_string())],
        _ => {
            // Tek medya dışı (metin/carousel) container oluşturulmaz.
            return Err(SocialError::UnsupportedPostType);
        }
    };

    let mut form = reqwest::blocking::multipart::Form::new()
        .text("caption", caption.to_string())
        .text("access_token", access_token.to_string());
    for (k, v) in kind_params {
        form = form.text(k, v);
    }

    let resp = client
        .post(&url)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::MediaContainerFailed)?;
    let ok = resp.status().is_success();
    let body = resp.text().unwrap_or_default();
    if !ok {
        return Err(map_container_error(&body));
    }

    let container_id = read_id_from_body(&body).ok_or(SocialError::MediaContainerFailed)?;
    Ok(container_id)
}

fn read_id_from_body(body: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct R {
        id: Option<String>,
    }
    let r: R = serde_json::from_str(body).ok()?;
    r.id.filter(|s| !s.is_empty())
}

/// Meta hata cevabını kontrollü hata kodlarına eşler (token/kod sızdırmaz).
pub fn map_container_error(_raw_body: &str) -> SocialError {
    #[derive(serde::Deserialize)]
    struct MetaErr {
        error: Option<MetaErrorBody>,
    }
    #[derive(serde::Deserialize)]
    struct MetaErrorBody {
        code: Option<i64>,
        #[serde(rename = "error_subcode")]
        subcode: Option<i64>,
        #[serde(rename = "error_user_msg")]
        user_msg: Option<String>,
        message: Option<String>,
    }
    let parsed = serde_json::from_str::<MetaErr>(_raw_body).ok();
    let e = parsed.as_ref().and_then(|e| e.error.as_ref());
    let code = e.and_then(|e| e.code);
    let user_msg = e.and_then(|e| e.user_msg.clone());
    let message = e.and_then(|e| e.message.clone());
    let text = user_msg.or(message).unwrap_or_default().to_lowercase();

    match code {
        Some(190) => SocialError::TokenExpired,
        Some(200) => SocialError::PermissionDenied,
        _ => {
            // Instagram işletme hesabı gerekliliği ve içerik izni mesajları.
            if text.contains("business") || text.contains("creator") {
                SocialError::InstagramProfessionalAccountRequired
            } else if text.contains("permission") || text.contains("review") {
                SocialError::AppReviewRequired
            } else {
                SocialError::ApiError
            }
        }
    }
}

fn update_status(app: &AppHandle, connection_id: &str, status: ConnectionStatus) {
    if let Ok(dir) = data_dir(app) {
        let _ = metadata_store::update_connection_status(&dir, connection_id, status);
    }
}

// ---- Testler ----

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_are_minimal_and_correct() {
        assert!(SCOPES.contains("instagram_basic"));
        assert!(SCOPES.contains("instagram_content_publish"));
        assert!(SCOPES.contains("pages_show_list"));
    }

    #[test]
    fn read_id_from_body_extracts_id() {
        assert_eq!(read_id_from_body(r#"{"id":"1789"}"#), Some("1789".to_string()));
        assert_eq!(read_id_from_body(r#"{}"#), None);
    }

    #[test]
    fn container_error_maps_token_and_permission() {
        assert_eq!(
            map_container_error(r#"{"error":{"code":190}}"#),
            SocialError::TokenExpired
        );
        assert_eq!(
            map_container_error(r#"{"error":{"code":200}}"#),
            SocialError::PermissionDenied
        );
        assert_eq!(
            map_container_error(r#"{"error":{"error_user_msg":"Must connect a business"}}"#),
            SocialError::InstagramProfessionalAccountRequired
        );
        assert_eq!(
            map_container_error(r#"{"error":{"message":"requires app review"}}"#),
            SocialError::AppReviewRequired
        );
        assert_eq!(
            map_container_error(r#"{"error":{"code":1}}"#),
            SocialError::ApiError
        );
    }

    #[test]
    fn publish_requires_some_media_source() {
        // Ne herkese açık URL ne yerel dosya varsa yayın `media_url_unavailable`
        // döner (sahte URL üretilmez).
        let kind = MediaKind::Photo;
        assert!(kind.has_media());
        let input = InstagramPostInput {
            connection_id: "c".into(),
            caption: "cap".into(),
            media_kind: Some(kind),
            media_urls: vec![],
            media_files: vec![],
            post_kind: None,
        };
        let dir = std::path::Path::new(".");
        // Dosya yok → barındırma yoluna girmeden önce `media_url_unavailable` beklenir.
        assert_eq!(
            resolve_media_url(dir, &input, kind).unwrap_err(),
            SocialError::MediaUrlUnavailable
        );
    }

    #[test]
    fn publish_uses_public_url_directly() {
        // Kullanıcı herkese açık URL verdiyse barındırmaya dokunulmaz.
        let input = InstagramPostInput {
            connection_id: "c".into(),
            caption: "cap".into(),
            media_kind: Some(MediaKind::Photo),
            media_urls: vec!["https://ornek.com/foto.jpg".into()],
            media_files: vec![],
            post_kind: None,
        };
        let dir = std::path::Path::new(".");
        let (url, token) = resolve_media_url(dir, &input, MediaKind::Photo).unwrap();
        assert_eq!(url, "https://ornek.com/foto.jpg");
        assert!(token.is_none());
    }

    #[test]
    fn missing_local_file_fails_validation() {
        // Yerel dosya verildi ama diskte yoksa kontrollü hata döner.
        let input = InstagramPostInput {
            connection_id: "c".into(),
            caption: "cap".into(),
            media_kind: Some(MediaKind::Photo),
            media_urls: vec![],
            media_files: vec!["C:\\yok\\foto.jpg".into()],
            post_kind: None,
        };
        let dir = std::path::Path::new(".");
        assert!(resolve_media_url(dir, &input, MediaKind::Photo).is_err());
    }

    #[test]
    fn text_only_is_not_instagram_eligible() {
        // Medyasız metin (ör. duyuru) Instagram hedefi olamaz.
        assert!(!meta::instagram_is_eligible(MediaKind::Text));
        let kind = PostKind::parse("duyuru").unwrap();
        assert!(kind.is_announcement());
    }
}
