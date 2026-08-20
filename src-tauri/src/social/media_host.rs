//! Ücretsiz, anonim geçici medya barındırma katmanı (0x0.st).
//!
//! Instagram Graph API, yayın container'ında `image_url` / `video_url` olarak
//! *herkese açık* bir medya adresi ister. Bu masaüstü uygulamasının kalıcı bir
//! medya sunucusu yoktur; bu katman görseli/videoyu yayın anında 0x0.st'ye
//! yükleyip Instagram'a kısa ömürlü herkese açık bir URL verir.
//!
//! Neden 0x0.st: üyelik/kredi kartı/anahtar gerektirmez (anonim upload),
//! doğrudan ham dosya URL'si döndürür (Instagram'ın istediği tam budur),
//! upload sonrası `X-Token` başlığında silme yetkisi verir ve dosyaya
//! `expires` süresi verilebilir (dosya otomatik silinir).
//!
//! İki katmanlı garanti:
//! 1. Yayın başarılıysa dosya, upload'la birlikte gelen token ile hemen silinir.
//! 2. Yayın başarısızsa dosya serviste kalsa bile hem `expires=1` (saat)
//!    sonunda otomatik düşer hem de uygulama `pending` manifestini tarayıp
//!    yaşı TTL'i aşan dosyaları token ile bir sonraki fırsatta siler.
//!    Böylece her koşulda medya internette kalıcı olarak asılı kalmaz.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::models::SocialError;

/// Azami yaş (saniye): bu süreden eski bekleyen kayıtlar otomatik temizlenir.
pub const PENDING_MAX_AGE_SECS: u64 = 3600;

/// 0x0.st'ye verilen varsayılan dosya ömrü (saat). Yayın sonrası anında
/// silinmeyen dosyalar bu süre sonunda servis tarafından otomatik düşer.
pub const DEFAULT_EXPIRES_HOURS: u32 = 1;

/// Pending manifest dosya adı (uygulama veri klasörü içinde).
const PENDING_FILE: &str = "media_host_pending.json";

/// 0x0.st yükleme adresi.
const UPLOAD_URL: &str = "https://0x0.st";

/// 0x0.st yanıtında dönen silme yetki token'ı başlığı.
const TOKEN_HEADER: &str = "x-token";

/// Bekleyen (silinmemiş) yükleme kaydı. Anahtar = upload token'ıdır.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PendingRecord {
    url: String,
    uploaded_at_unix: u64,
}

/// Pending manifesti: token → kayıt.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct PendingManifest {
    records: BTreeMap<String, PendingRecord>,
}

// ---- Pending manifest (TTL garantisi) ----

fn pending_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("social")
}

fn pending_file(data_dir: &Path) -> PathBuf {
    pending_dir(data_dir).join(PENDING_FILE)
}

fn read_manifest(data_dir: &Path) -> Result<PendingManifest, SocialError> {
    let path = pending_file(data_dir);
    if !path.exists() {
        return Ok(PendingManifest::default());
    }
    let raw = fs::read_to_string(&path).map_err(|_| SocialError::OperationFailed)?;
    if raw.trim().is_empty() {
        return Ok(PendingManifest::default());
    }
    serde_json::from_str(&raw).map_err(|_| SocialError::OperationFailed)
}

fn write_manifest(data_dir: &Path, manifest: &PendingManifest) -> Result<(), SocialError> {
    let path = pending_file(data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| SocialError::OperationFailed)?;
    }
    let raw = serde_json::to_string_pretty(manifest).map_err(|_| SocialError::OperationFailed)?;
    fs::write(&path, raw).map_err(|_| SocialError::OperationFailed)
}

/// Yükleme sonrası manifeste kayıt ekler (başarısızlık durumunda TTL temizliği).
fn mark_pending(data_dir: &Path, token: &str, url: &str) -> Result<(), SocialError> {
    let mut manifest = read_manifest(data_dir)?;
    manifest.records.insert(
        token.to_string(),
        PendingRecord {
            url: url.to_string(),
            uploaded_at_unix: now_unix(),
        },
    );
    write_manifest(data_dir, &manifest)
}

/// Kaydı manifestten çıkarır.
fn remove_pending(data_dir: &Path, token: &str) -> Result<(), SocialError> {
    let mut manifest = read_manifest(data_dir)?;
    manifest.records.remove(token);
    write_manifest(data_dir, &manifest)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn http_client() -> Result<reqwest::blocking::Client, SocialError> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|_| SocialError::OperationFailed)
}

fn mime_for(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        _ => "application/octet-stream",
    }
}

/// Dosyayı 0x0.st'ye yükler; ham medya URL'si döndürür. İkinci dönüş değeri,
/// yayın sonrası silme için gerekli token'dır.
pub fn upload_media(
    data_dir: &Path,
    file_path: &str,
    expires_hours: Option<u32>,
) -> Result<(String, String), SocialError> {
    let bytes = fs::read(file_path).map_err(|_| SocialError::FileNotFound)?;
    let file_name = Path::new(file_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "media".to_string());

    let mut form = reqwest::blocking::multipart::Form::new()
        .part(
            "file",
            reqwest::blocking::multipart::Part::bytes(bytes)
                .file_name(file_name)
                .mime_str(mime_for(file_path))
                .map_err(|_| SocialError::UploadFailed)?,
        )
        // Tahmin edilmesi zor (uzun) URL üretmek için boş `secret` alanı.
        .text("secret", "");
    let hours = expires_hours.unwrap_or(DEFAULT_EXPIRES_HOURS);
    if hours > 0 {
        form = form.text("expires", hours.to_string());
    }

    let client = http_client()?;
    let resp = client
        .post(UPLOAD_URL)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::UploadFailed)?;

    if !resp.status().is_success() {
        return Err(SocialError::UploadFailed);
    }

    // Başlıktaki silme yetki token'ı, `text()` body'yi tükettiği için önce alınır.
    let token = resp
        .headers()
        .get(TOKEN_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .trim()
        .to_string();
    if token.is_empty() {
        return Err(SocialError::UploadFailed);
    }

    let url = resp.text().unwrap_or_default().trim().to_string();
    if url.is_empty() || !url.starts_with("https://") {
        return Err(SocialError::UploadFailed);
    }

    mark_pending(data_dir, &token, &url)?;
    Ok((url, token))
}

/// Token ile yüklenen dosyayı siler ve manifestten çıkarır. 0x0.st silme
/// isteği yanıt kodu 2xx ya da 404 (zaten yok) olabilir; ikisi de başarıdır.
pub fn delete_media(data_dir: &Path, token: &str) -> Result<(), SocialError> {
    let manifest = read_manifest(data_dir)?;
    let Some(record) = manifest.records.get(token).cloned() else {
        return Ok(());
    };

    let client = http_client()?;
    let form = reqwest::blocking::multipart::Form::new()
        .text("token", token.to_string())
        .text("delete", "");
    let resp = client
        .post(&record.url)
        .multipart(form)
        .send()
        .map_err(|_| SocialError::OperationFailed)?;

    if resp.status().is_success() || resp.status().as_u16() == 404 {
        let _ = remove_pending(data_dir, token);
        Ok(())
    } else {
        Err(SocialError::OperationFailed)
    }
}

/// TTL'i aşan bekleyen yüklemeleri token ile siler (ikinci güvence).
/// Yayın öncesi ve uygulama başlangıcında çağrılabilir.
pub fn cleanup_stale(data_dir: &Path) -> Result<(), SocialError> {
    let manifest = read_manifest(data_dir)?;
    let now = now_unix();
    let mut stale: Vec<String> = Vec::new();
    for (token, record) in &manifest.records {
        if now.saturating_sub(record.uploaded_at_unix) >= PENDING_MAX_AGE_SECS {
            stale.push(token.clone());
        }
    }
    for token in stale {
        let _ = delete_media(data_dir, &token);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_for_known_extensions() {
        assert_eq!(mime_for("a.jpg"), "image/jpeg");
        assert_eq!(mime_for("b.PNG"), "image/png");
        assert_eq!(mime_for("c.mp4"), "video/mp4");
        assert_eq!(mime_for("d"), "application/octet-stream");
    }

    #[test]
    fn pending_manifest_roundtrip_is_inert() {
        // Dosyaya dokunmadan salt bellek akışı bozulmamalıdır.
        let mut m = PendingManifest::default();
        m.records.insert(
            "tok".into(),
            PendingRecord { url: "https://0x0.st/x".into(), uploaded_at_unix: 1 },
        );
        assert!(m.records.contains_key("tok"));
    }

    #[test]
    fn default_expiry_is_one_hour() {
        assert_eq!(DEFAULT_EXPIRES_HOURS, 1);
    }
}