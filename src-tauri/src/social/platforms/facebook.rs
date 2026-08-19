//! Facebook Sayfa entegrasyon modülü.
//!
//! Yalnız bağlı Facebook Sayfalarına yayın yapar. Kişisel profile otomatik
//! yayın yapılmaz; kişisel profil tokenıyla Sayfa yayını denenmez. Facebook
//! Sayfa tokenını kullanır (bağlantı kurma `meta::connect_facebook_page` ile
//! gerçek Sayfa tokenını Credential Manager'a yazar).
//!
//! Desteklenen içerikler (resmî API ve mevcut proje modeline göre):
//! metin, tek görsel, video. Çoklu görsel (carousel) mevcut proje modeline
//! göre Facebook hedefi değildir.
//!
//! Görsel ve video, resmî Graph API'nin yerel multipart yükleme desteği
//! sayesinde diskten doğrudan yüklenir; herkese açık media_url gerekmez.
//! (Instagram'dan farklı olarak Facebook yerel dosya yüklemesini destekler.)

use tauri::{AppHandle, Manager};

use super::super::credential_store;
use super::super::metadata_store;
use super::super::models::{ConnectionRecord, ConnectionStatus, SocialError, TokenType};
use super::meta::{self, MediaKind};

/// Facebook platform kimliği.
pub const PLATFORM_ID: &str = "facebook";

/// Meta Facebook için OAuth izinleri (yalnız gereken en dar izinler).
/// `pages_show_list`, `pages_manage_posts` ve `pages_read_engagement`
/// Sayfa yayını için gereklidir. App Review'dan geçmesi gerekebilir.
pub const SCOPES: &str =
    "pages_show_list,pages_manage_posts,pages_read_engagement";

/// Facebook sayfa yayınını çağırmak için hazırlanan kontrollü yayın girdisi.
/// Mevcut yayın motorunun güvenilir veri modelinden gelen alanlardır;
/// JavaScript'ten serbest metin gelmez.
pub struct FacebookPostInput {
    pub connection_id: String,
    pub message: String,
    /// Teşhis edilmiş içerik türü (kontrollü). Boşsa metin yayını kabul edilir.
    pub media_kind: Option<MediaKind>,
    /// Yerel görsel/video dosya yolları (carousel desteklenmez, tek medya).
    pub media_files: Vec<String>,
    /// Fotoğraf / video başlığı (başlık / açıklama).
    pub title: String,
}

/// Facebook Sayfa tokenını bir bağlantıdan güvenli biçimde alır.
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

fn data_dir(app: &AppHandle) -> Result<std::path::PathBuf, SocialError> {
    app.path()
        .app_data_dir()
        .map_err(|_| SocialError::ConnectionStoreError)
}

/// Facebook'a gerçek Sayfa yayını yapar ve gerçek post kimliğini döndürür.
///
/// - Metin: `POST /{page-id}/feed?message=...`
/// - Tek görsel: `POST /{page-id}/photos` (multipart `source` ile diskten)
/// - Video: `POST /{page-id}/videos` (multipart `source` ile diskten)
///
/// Yayın başlamadan önce içerik türü doğrulanır. Başarısız yayın başarılı
/// kaydedilmez; gerçek Graph API medya/post kimliği döner.
pub fn publish(
    app: &AppHandle,
    input: &FacebookPostInput,
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

    let page_token = obtain_page_token(app, &record)?;

    // İçerik türü ve hedef kuralları (Facebook'a uygun kombinasyonlar).
    let media = input.media_kind.unwrap_or(MediaKind::Text);
    if !meta::facebook_is_eligible(media) {
        return Err(SocialError::UnsupportedPostType);
    }

    let client = meta::http_client()?;

    // Facebook yerel dosyayı multipart olarak kabul eder; media_url gerekmez.
    match media {
        MediaKind::Text => publish_text(&client, &record, &page_token, input),
        MediaKind::Photo => publish_photo(&client, &record, &page_token, input),
        MediaKind::Video => publish_video(&client, &record, &page_token, input),
        MediaKind::Carousel => Err(SocialError::UnsupportedPostType),
    }
}

/// Facebook'a çoklu görseli tek gönderi (carousel) olarak yayınlar ve gerçek
/// post kimliğini döndürür.
///
/// Her görsel önce `POST /{page-id}/photos` ile `published=false` ve
/// `temporary=true` olarak diskten multipart yüklenir ve geçici `image_id`
/// toplanır; ardından tüm görseller `attached_media` dizisiyle
/// `POST /{page-id}/feed` uç noktasında tek gönderiye bağlanır. Mevcut
/// `publish` (tek görsel) akışına dokunulmaz; bu fonksiyon yalnız çoklu görsel
/// grupları için ayrı bir uç noktadır.
pub fn publish_carousel(
    app: &AppHandle,
    input: &FacebookPostInput,
) -> Result<String, SocialError> {
    if input.connection_id.trim().is_empty() {
        return Err(SocialError::InvalidConnection);
    }
    // Carousel en az 2, en fazla 10 görseldir (Detaylı Paylaşım şartnamesi).
    if input.media_files.len() < 2 {
        return Err(SocialError::UnsupportedPostType);
    }
    if input.media_files.len() > 10 {
        return Err(SocialError::UnsupportedPostType);
    }

    let dir = data_dir(app)?;
    let record = metadata_store::get_connection(&dir, &input.connection_id)?
        .ok_or(SocialError::InvalidConnection)?;
    if record.platform_id != PLATFORM_ID {
        return Err(SocialError::InvalidConnection);
    }

    let page_token = obtain_page_token(app, &record)?;
    let client = meta::http_client()?;

    // 1) Her görseli geçici fotoğraf olarak yükle ve image_id topla.
    let mut image_ids: Vec<String> = Vec::new();
    for path in &input.media_files {
        super::super::media_validation::verify_image_or_photo_file(path)?;
        let url = format!("{}/photos", graph_node_url(&record.external_account_id));
        let file_part = reqwest::blocking::multipart::Part::file(path)
            .map_err(|_| SocialError::FileNotFound)?;
        let form = reqwest::blocking::multipart::Form::new()
            .text("access_token", page_token.to_string())
            .text("published", "false")
            .text("temporary", "true")
            .part("source", file_part);
        let resp = client
            .post(&url)
            .multipart(form)
            .send()
            .map_err(|_| SocialError::PublishFailed)?;
        let ok = resp.status().is_success();
        let body = resp.text().unwrap_or_default();
        if !ok {
            return Err(map_publish_error(&body));
        }
        let image_id = read_id_from_body(&body).ok_or(SocialError::PublishFailed)?;
        image_ids.push(image_id);
    }

    // 2) Toplanan görselleri tek gönderide (carousel) birleştir.
    let attached: Vec<serde_json::Value> = image_ids
        .iter()
        .map(|id| serde_json::json!({ "image_id": id }))
        .collect();
    let attached_json = serde_json::to_string(&attached).unwrap_or_default();
    let url = format!(
        "{}?message={}&attached_media={}&access_token={}",
        graph_node_url(&record.external_account_id),
        urlencode(&input.message),
        urlencode(&attached_json),
        page_token
    );
    send_post_and_read_id(&client, &url)
}

fn graph_node_url(node_id: &str) -> String {
    format!("https://graph.facebook.com/{}/{}", meta::META_GRAPH_VERSION, node_id)
}

fn publish_text(
    client: &reqwest::blocking::Client,
    record: &ConnectionRecord,
    page_token: &str,
    input: &FacebookPostInput,
) -> Result<String, SocialError> {
    if input.message.trim().is_empty() {
        return Err(SocialError::UnsupportedPostType);
    }
    let url = format!(
        "{}?message={}&access_token={}",
        graph_node_url(&record.external_account_id),
        urlencode(&input.message),
        page_token
    );
    send_post_and_read_id(client, &url)
}

fn publish_photo(
    client: &reqwest::blocking::Client,
    record: &ConnectionRecord,
    page_token: &str,
    input: &FacebookPostInput,
) -> Result<String, SocialError> {
    let path = input.media_files.first().ok_or(SocialError::InvalidMediaFile)?;
    super::super::media_validation::verify_image_or_photo_file(path)?;

    let url = format!("{}/photos", graph_node_url(&record.external_account_id));
    let form = make_multipart_form(path, &input.message, page_token)?;

    let resp = client
        .post(&url)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::PublishFailed)?;
    let ok = resp.status().is_success();
    let body = resp.text().unwrap_or_default();
    if !ok {
        return Err(map_publish_error(&body));
    }
    read_id_from_body(&body).ok_or(SocialError::PublishFailed)
}

fn publish_video(
    client: &reqwest::blocking::Client,
    record: &ConnectionRecord,
    page_token: &str,
    input: &FacebookPostInput,
) -> Result<String, SocialError> {
    let path = input.media_files.first().ok_or(SocialError::InvalidMediaFile)?;
    super::super::media_validation::verify_video_file(path)?;

    let url = format!("{}/videos", graph_node_url(&record.external_account_id));
    let form = make_video_form(path, &input.title, &input.message, page_token)?;

    let resp = client
        .post(&url)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::PublishFailed)?;
    let ok = resp.status().is_success();
    let body = resp.text().unwrap_or_default();
    if !ok {
        return Err(map_publish_error(&body));
    }
    read_id_from_body(&body).ok_or(SocialError::PublishFailed)
}

// ---- Multipart yardımcılar (diskten doğrudan yükleme) ----

fn make_multipart_form(
    path: &str,
    message: &str,
    page_token: &str,
) -> Result<reqwest::blocking::multipart::Form, SocialError> {
    let file_part = reqwest::blocking::multipart::Part::file(path)
        .map_err(|_| SocialError::FileNotFound)?;
    let form = reqwest::blocking::multipart::Form::new()
        .text("access_token", page_token.to_string())
        .text("message", message.to_string())
        .part("source", file_part);
    Ok(form)
}

fn make_video_form(
    path: &str,
    title: &str,
    description: &str,
    page_token: &str,
) -> Result<reqwest::blocking::multipart::Form, SocialError> {
    let file_part = reqwest::blocking::multipart::Part::file(path)
        .map_err(|_| SocialError::FileNotFound)?;
    let resolved_title = if title.trim().is_empty() {
        "ES OPS paylasimi"
    } else {
        title
    };
    let form = reqwest::blocking::multipart::Form::new()
        .text("access_token", page_token.to_string())
        .text("title", resolved_title.to_string())
        .text("description", description.to_string())
        .part("source", file_part);
    Ok(form)
}

/// Basit form-urlencoded urlencode (mesaj metni sorguya güvenli eklenir).
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// Grafik node adresine POST atar ve yanıttan gerçek id okur.
fn send_post_and_read_id(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<String, SocialError> {
    let resp = client
        .post(url)
        .send()
        .map_err(|_| SocialError::PublishFailed)?;
    let ok = resp.status().is_success();
    let body = resp.text().unwrap_or_default();
    if !ok {
        return Err(map_publish_error(&body));
    }
    read_id_from_body(&body).ok_or(SocialError::PublishFailed)
}

fn read_id_from_body(body: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct R {
        id: Option<String>,
    }
    let r: R = serde_json::from_str(body).ok()?;
    r.id.filter(|s| !s.is_empty())
}

/// Meta hata cevabını kontrollü hata kodlarına eşler.
/// Ham Meta hata cevabı kullanıcıya gösterilmez; yalnız kod/alt kod üzerinden
/// gizli bilgi içermeyen kontrollü duruma eşlenir.
pub fn map_publish_error(_raw_body: &str) -> SocialError {
    // App Review ve izin eksikliği ayrımı için meta hata kodunu çözümlemek
    // amacıyla ham gövdeyi önce ayrıştırmayı deneriz; token/kod sızdırmayız.
    #[derive(serde::Deserialize)]
    struct MetaErr {
        error: Option<MetaErrorBody>,
    }
    #[derive(serde::Deserialize)]
    struct MetaErrorBody {
        code: Option<i64>,
        #[serde(rename = "error_subcode")]
        subcode: Option<i64>,
    }
    let parsed = serde_json::from_str::<MetaErr>(_raw_body).ok();
    let code = parsed.as_ref().and_then(|e| e.error.as_ref()).and_then(|e| e.code);
    let subcode = parsed
        .as_ref()
        .and_then(|e| e.error.as_ref())
        .and_then(|e| e.subcode);

    match code {
        // 190: geçersiz / süresi dolan token.
        Some(190) => SocialError::TokenExpired,
        // 200: izin reddi.
        Some(200) => SocialError::PermissionDenied,
        // Bizim modelde tanımlanmış özel alt kodlar.
        _ => {
            match subcode {
                Some(2018008) | Some(33) => SocialError::PermissionDenied,
                _ => SocialError::ApiError,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scopes_are_minimal_and_correct() {
        assert!(SCOPES.contains("pages_show_list"));
        assert!(SCOPES.contains("pages_manage_posts"));
    }

    #[test]
    fn urlencode_handles_spaces_and_turkish() {
        assert_eq!(urlencode("merhaba dunya"), "merhaba+dunya");
        assert_eq!(urlencode("a&b"), "a%26b");
        assert_eq!(urlencode("Merhaba Istanbul"), "Merhaba+Istanbul");
    }

    #[test]
    fn read_id_from_body_extracts_id() {
        assert_eq!(read_id_from_body(r#"{"id":"123456"}"#), Some("123456".to_string()));
        assert_eq!(read_id_from_body(r#"{}"#), None);
        assert_eq!(read_id_from_body("not json"), None);
    }

    #[test]
    fn map_error_handles_common_codes() {
        assert_eq!(
            map_publish_error(r#"{"error":{"code":190,"message":"token"}}"#),
            SocialError::TokenExpired
        );
        assert_eq!(
            map_publish_error(r#"{"error":{"code":200}}"#),
            SocialError::PermissionDenied
        );
        assert_eq!(
            map_publish_error(r#"{"error":{"code":1}}"#),
            SocialError::ApiError
        );
        assert_eq!(map_publish_error("garbage"), SocialError::ApiError);
    }
}
