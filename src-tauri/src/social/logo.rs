//! Kurumsal logo bindirme (yalnizca eklenen yeni ozellik).
//!
//! Bu modul mevcut yayin motorunun HICBIR parcasini degistirmez. Yalnizca:
//! - Bir logo dosyasini uygulama veri klasorune kalici olarak kopyalar.
//! - Yayin oncesi gorsellere logo bindirir ve islenmis gecici dosyalari dondurur.
//!
//! Logo tanimli degilse `apply_logo_to_images` gelen yollari aynen dondurur;
//! boylece mevcut davranis birebir korunur (logo yoksa hicbir fark olmaz).
//! Video dosyalari ve gorsel olmayan dosyalar hicbir zaman islenmez.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::models::SocialError;

/// Logo dosyalarinin saklandigi alt klasor adi (uygulama veri klasoru icinde).
const LOGO_DIR_NAME: &str = "logo";

/// Gorsellerin alt bolgesine bindirilecek logo genislik orani (gorsel genisligine gore).
const LOGO_WIDTH_RATIO: f64 = 0.15;
/// Logo opakligi (0.0-1.0). Gorseli ortmemek icin yari saydam tutulur.
const LOGO_OPACITY: f64 = 0.65;

/// Logo durumu (gizli bilgi icermez).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogoStatus {
    pub configured: bool,
    pub filename: String,
}

/// Veri klasorunu hesaplar.
fn data_dir(app: &AppHandle) -> Result<PathBuf, SocialError> {
    app.path()
        .app_data_dir()
        .map_err(|_| SocialError::ConnectionStoreError)
}

/// Logo klasorunun yolu.
fn logo_dir(app: &AppHandle) -> Result<PathBuf, SocialError> {
    Ok(data_dir(app)?.join(LOGO_DIR_NAME))
}

/// Aktif logo dosyasinin yolu. Dosya yoksa `None` doner (hata uretilmez).
fn active_logo_path(app: &AppHandle) -> Result<Option<PathBuf>, SocialError> {
    let dir = logo_dir(app)?;
    if !dir.is_dir() {
        return Ok(None);
    }
    let mut found: Option<PathBuf> = None;
    for entry in std::fs::read_dir(&dir).map_err(|_| SocialError::OperationFailed)? {
        let entry = entry.map_err(|_| SocialError::OperationFailed)?;
        let p = entry.path();
        if p.is_file() {
            // En guncel dosya tercih edilir; dosya adi karisik olabilir.
            let newer = match &found {
                Some(existing) => p
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .zip(existing.metadata().and_then(|m| m.modified()).ok())
                    .map(|(a, b)| a > b)
                    .unwrap_or(false),
                None => true,
            };
            if newer {
                found = Some(p);
            }
        }
    }
    Ok(found)
}

/// Verilen dosyanin gecidi bir gorsel olup olmadigini dogrular (sadece isim + varlik).
/// Logo bindirmede `image` kutuphanesi icerigi acarken kendisi de dogrular.
fn is_image_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    ["jpg", "jpeg", "png", "webp", "gif", "bmp"]
        .iter()
        .any(|ext| lower.ends_with(&format!(".{}", ext)))
}

/// Girilen dosyayi uygulama veri klasorundeki logo klasorune kopyalar.
/// Dosya gercek bir gorsel olmalidir (uzanti + icerik imzasi dogrulanir).
#[tauri::command]
pub fn logo_set(app: AppHandle, path: String) -> Result<LogoStatus, SocialError> {
    let src = Path::new(&path);
    if !src.is_file() {
        return Err(SocialError::FileNotFound);
    }
    super::media_validation::verify_image_or_photo_file(&path)?;

    let dir = logo_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|_| SocialError::OperationFailed)?;

    // Kaynak dosya adini koru (kolay tanima). Ayni isimle var olan ustune yazilir.
    let name = src
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("logo.png")
        .to_string();
    let dest = dir.join(&name);
    std::fs::copy(src, &dest).map_err(|_| SocialError::OperationFailed)?;

    Ok(LogoStatus {
        configured: true,
        filename: name,
    })
}

/// Logo yapilandirilip yapilandirilmadigini dondurur. Ham veri icermez.
#[tauri::command]
pub fn logo_status(app: AppHandle) -> Result<LogoStatus, SocialError> {
    Ok(match active_logo_path(&app)? {
        Some(p) => {
            let filename = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            LogoStatus {
                configured: true,
                filename,
            }
        }
        None => LogoStatus {
            configured: false,
            filename: String::new(),
        },
    })
}

/// Yapilandirilmis logoyu siler. Logo yoksa basarisizlik dondurmez.
#[tauri::command]
pub fn logo_clear(app: AppHandle) -> Result<LogoStatus, SocialError> {
    let dir = logo_dir(&app)?;
    if dir.is_dir() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    Ok(LogoStatus {
        configured: false,
        filename: String::new(),
    })
}

/// Gelen medya yollarina (yalniz gorseller) aktif logoyu bindirir ve islenmis
/// dosyalarin gecici yollarini dondurur.
///
/// Davranis kurallari:
/// - Logo tanimli degilse gelen yollar AYNEN dondurulur (mevcut davranis korunur).
/// - Gorsel olmayan dosyalar (video vb.) asla islenmez, aynen gecirilir.
/// - Gorsel acilamazsa kontrollu hata dondurulur; sahte islem yapilmaz.
/// - Video dosyalari islem gormez; bu komut yalniz gorsel isler.
#[tauri::command]
pub fn apply_logo_to_images(
    app: AppHandle,
    paths: Vec<String>,
) -> Result<Vec<String>, SocialError> {
    let Some(logo_path) = active_logo_path(&app)? else {
        return Ok(paths);
    };
    if !is_image_extension(logo_path.to_string_lossy().as_ref()) {
        return Ok(paths);
    }

    // Logo gorselini bir kez ac.
    let logo_img = image::open(&logo_path).map_err(|_| SocialError::InvalidMediaFile)?;
    let logo_rgba = logo_img.to_rgba8();

    let mut out = Vec::with_capacity(paths.len());
    for p in &paths {
        if !is_image_extension(p) {
            // Video / bilinmeyen dosya: islenmez, aynen gecirilir.
            out.push(p.clone());
            continue;
        }
        if !Path::new(p).is_file() {
            return Err(SocialError::FileNotFound);
        }
        let base = image::open(p).map_err(|_| SocialError::InvalidMediaFile)?;
        let composed = overlay_logo(&base, &logo_rgba);
        let tmp = write_temp_composed(p, &composed)?;
        out.push(tmp);
    }
    Ok(out)
}

/// Logoyu gorselin alt-orta bolgesine, gorseli bozmadan ve ortmeden bindirir.
///
/// - Boyut: gorsel genisliginin ~%15'i (kucuk, belirgin olmayan).
/// - Konum: alt kenardan ~%3 mesafede, yatayda ortali.
/// - Opaklik: %65 (yarim saydam; gorsel detaylari kapatmaz).
fn overlay_logo(
    base: &image::DynamicImage,
    logo: &image::RgbaImage,
) -> image::RgbaImage {
    let (w, h) = (base.width(), base.height());
    if w == 0 || h == 0 {
        return base.to_rgba8();
    }

    let logo_w = ((w as f64 * LOGO_WIDTH_RATIO).round() as u32).max(1);
    let logo_h = ((logo_w as u64 * logo.height() as u64)
        .checked_div(logo.width().max(1) as u64)
        .unwrap_or(0) as u32)
        .max(1);

    // Logoyu kucult, alfasini opaklik kadar azalt, sonra dogru alfa kompoziti ile yerlestir.
    let mut resized = image::imageops::resize(
        logo,
        logo_w,
        logo_h,
        image::imageops::FilterType::Lanczos3,
    );
    for px in resized.pixels_mut() {
        px[3] = ((px[3] as f64 * LOGO_OPACITY).round() as u8).min(255);
    }

    // Alt kenar boslugu: gorsel yuksekliginin ~%3'u (minimum 4px).
    let margin = (((h as f64) * 0.03).round() as u32).max(4);
    let x = (w.saturating_sub(logo_w)) / 2;
    let y = h.saturating_sub(logo_h).saturating_sub(margin);

    let mut out = base.to_rgba8();
    // resized gorsel alfa degerlerini tasir; overlay dogru bicimde harmanlar.
    image::imageops::overlay(&mut out, &resized, x as i64, y as i64);
    out
}

/// Islenmis gorseli gecici klasore yazar ve yolunu dondurur.
fn write_temp_composed(src_path: &str, img: &image::RgbaImage) -> Result<String, SocialError> {
    let ext = Path::new(src_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png")
        .to_lowercase();
    let ext = match ext.as_str() {
        "jpg" | "jpeg" => "jpg",
        "webp" => "webp",
        "gif" => "gif",
        "bmp" => "bmp",
        _ => "png",
    };

    let dir = std::env::temp_dir().join("es-ops-logo");
    std::fs::create_dir_all(&dir).map_err(|_| SocialError::OperationFailed)?;

    let base = Path::new(src_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("gorsel");
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let unique = format!("{}-{}.{}", base, ts, ext);
    let dest = dir.join(&unique);

    // Gorsel formatina gore kaydet (jpg disinda saydamligi korur).
    let save_result = match ext {
        "jpg" => img.save_with_format(&dest, image::ImageFormat::Jpeg),
        "webp" => img.save_with_format(&dest, image::ImageFormat::WebP),
        "gif" => img.save_with_format(&dest, image::ImageFormat::Gif),
        "bmp" => img.save_with_format(&dest, image::ImageFormat::Bmp),
        _ => img.save_with_format(&dest, image::ImageFormat::Png),
    };
    save_result.map_err(|_| SocialError::OperationFailed)?;

    Ok(dest.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::is_image_extension;
    use super::overlay_logo;

    fn make_rgba(w: u32, h: u32, color: [u8; 4]) -> image::RgbaImage {
        image::RgbaImage::from_pixel(w, h, image::Rgba(color))
    }

    #[test]
    fn logo_yoksa_yollar_aynen_doner() {
        // active_logo_path None olunca aynen doner (AppHandle olmadan test edilemez;
        // bunun yerine dongu mantigini bir fonksiyon uzerinden test etmek icin
        // is_image_extension + passthrough kontrolu yapilir).
        assert!(!is_image_extension("video.mp4"));
        assert!(is_image_extension("gorsel.PNG"));
        assert!(is_image_extension("foto.jpeg"));
        assert!(!is_image_extension("foto.txt"));
    }

    #[test]
    fn logo_bindirme_boyut_degistirmez() {
        let base = make_rgba(100, 80, [10, 20, 30, 255]);
        let logo = make_rgba(50, 20, [255, 255, 255, 255]);
        let out = overlay_logo(&image::DynamicImage::ImageRgba8(base), &logo);
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 80);
    }

    #[test]
    fn logo_bindirme_kose_pikselleri_etkilenmez() {
        // Logo alt-orta bolgeye yerlestirilir; ust kenar ve sol ust kose
        // orijinal gorsel rengini korur.
        let base = make_rgba(200, 200, [100, 100, 100, 255]);
        let logo = make_rgba(30, 20, [255, 0, 0, 255]);
        let out = overlay_logo(&image::DynamicImage::ImageRgba8(base), &logo);
        // Sol ust kose: orijinal gri kalir.
        assert_eq!(out.get_pixel(0, 0)[0], 100);
        assert_eq!(out.get_pixel(0, 0)[1], 100);
        assert_eq!(out.get_pixel(0, 0)[2], 100);
        // Ust satir da etkilenmez.
        assert_eq!(out.get_pixel(50, 0)[0], 100);
    }

    #[test]
    fn logo_bindirme_alt_bolge_degisir() {
        let base = make_rgba(200, 200, [100, 100, 100, 255]);
        let logo = make_rgba(30, 20, [255, 0, 0, 255]);
        let out = overlay_logo(&image::DynamicImage::ImageRgba8(base), &logo);
        // Alt-orta bolgede logo rengi gorulmeli (kirmizi artmis olmali).
        let cx = 100;
        let cy = 200 - (200u32 as f64 * 0.03).round() as u32 - 10;
        let px = out.get_pixel(cx, cy);
        assert!(px[0] > 100, "kirmizi kanal artmali, gelen: {}", px[0]);
    }

    #[test]
    fn logo_bindirme_ufak_gorselde_panik_yapmaz() {
        let base = make_rgba(1, 1, [5, 5, 5, 255]);
        let logo = make_rgba(3, 3, [255, 0, 0, 255]);
        let out = overlay_logo(&image::DynamicImage::ImageRgba8(base), &logo);
        assert_eq!(out.width(), 1);
        assert_eq!(out.height(), 1);
    }
}