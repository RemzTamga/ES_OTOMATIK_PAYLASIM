//! Küçük, ortak media türü doğrulama katmanı.
//!
//! YouTube'a gidecek bir dosyanın "gerçek bir video" olduğunu yalnız uzantıya
//! güvenerek değil, dosya içeriğinin tanınmış video konteyner imzalarıyla
//! eşleşmesini de bekleyerek doğrular. Bu katman hiçbir kodcuyu açar,
//! bellek büyütmez ve dış servise bağlanmaz; yalnız başlık (magic) baytlarına
//! bakar. Amacı, "yalnız uzantıya güvenme" kuralına karşı minimum güvenilir
//! güvence sağlamaktır.
//!
//! Bu katman ortaktır (yalnızca YouTube'a bağlı değildir) ve ileride başka
//! platformların media doğrulaması için de kullanılabilir.

use super::models::SocialError;

/// Bilinen video dosya uzantıları.
const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mov", "m4v", "webm", "mkv", "avi", "ogv", "mpg", "mpeg", "3gp",
];

/// Bilinen video konteyner başlık imzaları.
///
/// - MP4 / QuickTime (.mp4, .mov, .m4v, .3gp): offset 4'te `ftyp`
/// - WebM / Matroska (.webm, .mkv): 0x1A 45 DF A3 (EBML başlığı)
/// - AVI (.avi): offset 8'de `AVI `
fn has_video_magic(bytes: &[u8]) -> bool {
    if bytes.len() < 12 {
        return false;
    }
    // ftyp (MP4 / QuickTime / 3GP)
    if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        return true;
    }
    // EBML (WebM / Matroska)
    if &bytes[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return true;
    }
    // AVI
    if bytes.len() >= 12 && &bytes[8..12] == b"AVI " {
        return true;
    }
    false
}

/// Dosyanın uzantısının bilinen bir video uzantısı olup olmadığını döner.
pub fn has_video_extension(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    VIDEO_EXTENSIONS
        .iter()
        .any(|ext| lower.ends_with(&format!(".{}", ext)))
}

/// Verilen dosya yolunun gerçek, okunabilir ve tanınan bir video dosyası
/// olup olmadığını doğrular.
///
/// Kurallar:
/// - Dosya mevcut değilse veya normal bir dosya değilse `file_not_found`.
/// - Uzantı bilinen bir video uzantısı değilse veya içerik imzası eşleşmezse
///   `invalid_video_file`.
pub fn verify_video_file(path: &str) -> Result<(), SocialError> {
    let meta = std::fs::metadata(path).map_err(|_| SocialError::FileNotFound)?;
    if !meta.is_file() {
        return Err(SocialError::FileNotFound);
    }

    if !has_video_extension(path) {
        return Err(SocialError::InvalidVideoFile);
    }

    // Başlık baytlarını oku (yalnız uzantıya güvenme).
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|_| SocialError::FileNotFound)?;
    let mut buf = [0u8; 12];
    let read = file.read_exact(&mut buf);
    let _ = read; // başlık kısayılır ve bu yeterlidir

    if !has_video_magic(&buf) {
        return Err(SocialError::InvalidVideoFile);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_check_is_case_insensitive() {
        assert!(has_video_extension("video.MP4"));
        assert!(has_video_extension("video.webm"));
        assert!(!has_video_extension("image.png"));
    }

    #[test]
    fn magic_detects_ftyp() {
        let mut data = [0u8; 12];
        data[4..8].copy_from_slice(b"ftyp");
        assert!(has_video_magic(&data));
    }

    #[test]
    fn magic_detects_ebml_webm() {
        let data = [0x1A, 0x45, 0xDF, 0xA3, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        assert!(has_video_magic(&data));
    }

    #[test]
    fn magic_detects_avi() {
        let mut data = [0u8; 12];
        data[8..12].copy_from_slice(b"AVI ");
        assert!(has_video_magic(&data));
    }

    #[test]
    fn magic_rejects_plain_data() {
        let data = [0u8; 12];
        assert!(!has_video_magic(&data));
        // Bir png imzası video imzası sayılmaz
        let mut data = [0u8; 12];
        data[0..4].copy_from_slice(&[0x89, 0x50, 0x4E, 0x47]);
        assert!(!has_video_magic(&data));
    }
}
