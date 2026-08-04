//! Merkezi log modülü.
//!
//! Rust ve JavaScript hatalarını uygulamanın veri klasöründeki
//! `logs/es-ops.log` dosyasına yazar. Güvenlik kuralları:
//!
//! - Token, parola, API anahtarı, Client Secret ve lisans kodu ASLA loglanmaz.
//!   Hem gelen mesaj hem de yazım öncesi içerik maskeleme fonksiyonundan
//!   geçirilir.
//! - Log dosyası boyutu sınırlıdır (`MAX_LOG_BYTES`); aşıldığında eski log
//!   `.1`, `.2` ... şeklinde döndürülür (rotasyon).
//!
//! Tauri komutları (`log_append`, `log_open_folder`, `log_export`) ön yüzün
//! JavaScript hatalarını kaydetmesini ve kullanıcının log klasörünü açmasını /
//! logları dışa aktarmasını sağlar.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tauri::{AppHandle, Manager};

/// Log dosyasının bulunduğu alt klasör (uygulama veri klasörü altında).
const LOG_DIR_NAME: &str = "logs";
/// Log dosyası adı.
const LOG_FILE_NAME: &str = "es-ops.log";
/// Tek bir log dosyasının azami boyutu (ör. 256 KB).
const MAX_LOG_BYTES: u64 = 256 * 1024;
/// Tutulacak döndürülmüş (rotasyon) log sayısı: es-ops.log.1 .. es-ops.log.3.
const MAX_ROTATED_FILES: u32 = 3;
/// Zaman damgası formatı (yerel saat).
const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Uygulamanın veri klasörü yolu (bellekte bir kez önbelleğe alınır).
static DATA_DIR: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();

/// Uygulamanın veri klasörünü önbelleğe alır (panic hook için).
pub fn set_data_dir(dir: PathBuf) {
    let _ = DATA_DIR.get_or_init(|| Mutex::new(None));
    if let Some(slot) = DATA_DIR.get() {
        *slot.lock().unwrap() = Some(dir);
    }
}

// ---------------------------------------------------------------------------
// Gizlilik: hassas değer maskeleme.
// ---------------------------------------------------------------------------

/// Görünen/gelen metni loglanabilir hale getirir: token, parola, api anahtarı,
/// client secret ve lisans koduna benzeyen değerleri `[GIZLI]` ile değiştirir.
///
/// Bilinçli bir karar gereği buluşsal (heuristic) maskeleme kullanılır:
/// kesin bilinen alan adlarının ardındaki değerler ve uzun "anahtar benzeri"
/// dizeler (ör. JWT / base64 bloklar) maskelenir. Yanlışlıkla secret sızması
/// ihtimali, bir metni gereksiz maskeleme ihtimalinden daha ciddidir.
pub fn redact_secrets(input: &str) -> String {
    // 1) Alan adı + ayraç + değer biçimleri: `api_key`, `client_secret`,
    //    `password`, `token`, `authorization`, `license_code` vb.
    let mut out = redact_key_values(input);
    // 2) JWT / token benzeri uzun dizeler: "eyJ..." ve 20+ karakterli
    //    base64/alnum blokları.
    out = redact_long_tokens(&out);
    // 3) `Bearer <token>` / `Bearer=<token>` kalıpları.
    out = redact_bearer(&out);
    out
}

/// `key = value`, `key: value`, `"key":"value"` ve `key value` biçimlerindeki
/// bilinen hassas alan adlarının değerlerini maskeler.
fn redact_key_values(input: &str) -> String {
    const SENSITIVE_KEYS: &[&str] = &[
        "api_key",
        "apikey",
        "api key",
        "client_secret",
        "client secret",
        "consumer_secret",
        "consumer key",
        "access_token",
        "refresh_token",
        "authorization",
        "password",
        "passwd",
        "pwd",
        "token",
        "secret",
        "license_key",
        "license_code",
        "license code",
        "lisans_kodu",
        "license",
        "client_id",
        "app_secret",
        "app_id",
    ];

    let mut result = String::with_capacity(input.len());
    let lower = input.to_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;
    let n = bytes.len();

    while i < n {
        let mut matched = false;
        for key in SENSITIVE_KEYS {
            let kbytes = key.as_bytes();
            let klen = kbytes.len();
            if i + klen <= n && &lower[i..i + klen] == *key {
                // Anahtar sınırı: öncesi ve sonrası harf/rakam olmamalı
                // (böylece "api_key_2" gibi başka tanımlayıcılar etkilenmez).
                let before_ok = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                let next = i + klen;
                let after_ok = next >= n || !(bytes[next].is_ascii_alphanumeric() || bytes[next] == b'_');
                if before_ok && after_ok {
                    // Key'i aynen kopyala
                    result.push_str(key);
                    let mut j = next;
                    // Boşlukları atla
                    while j < n && (bytes[j] == b' ' || bytes[j] == b'\t') {
                        j += 1;
                    }
                    // Ayraç varsa kopyala (:, =, ") ve değere atla
                    if j < n && (bytes[j] == b':' || bytes[j] == b'=' || bytes[j] == b'"') {
                        let sep = bytes[j];
                        j += 1;
                        // Separatörün ardındaki boşlukları da korumak yerine
                        // maskeli değer eklerken net çıktı üretelim.
                        while j < n && (bytes[j] == b' ' || bytes[j] == b'\t') {
                            j += 1;
                        }
                        result.push(sep as char);
                        if j < n {
                            // Değerin bittiği yeri bul (boşluk, virgül, yeni satır, } ] ...)
                            let val_start = j;
                            while j < n
                                && !bytes[j].is_ascii_whitespace()
                                && bytes[j] != b','
                                && bytes[j] != b'}'
                                && bytes[j] != b']'
                                && bytes[j] != b')'
                            {
                                j += 1;
                            }
                            if j > val_start {
                                result.push_str("[GIZLI]");
                            }
                        }
                    }
                    i = j;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            // UTF-8 güvenli tek char kopyala
            let ch = input[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}

/// 20+ karakterli, yalnızca harf/rakam/`-`/`_`/`.`/`_` içeren blokları
/// (JWT, base64 token, oturum anahtarı) maskeler.
fn redact_long_tokens(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut run = String::new();

    fn flush(run: &mut String, result: &mut String) {
        if !run.is_empty() {
            let all_ident = run.chars().all(|c| c.is_ascii_lowercase() || c == '_');
            if run.len() >= 20 && !all_ident {
                result.push_str("[GIZLI]");
            } else {
                result.push_str(run);
            }
            run.clear();
        }
    }

    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            run.push(ch);
        } else {
            flush(&mut run, &mut result);
            result.push(ch);
        }
    }
    flush(&mut run, &mut result);
    result
}

/// `Bearer <token>` / `Bearer=<token>` kalıbını maskeler.
fn redact_bearer(input: &str) -> String {
    let lower = input.to_lowercase();
    if !lower.contains("bearer") {
        return input.to_string();
    }

    let mut result = String::with_capacity(input.len());
    let mut rest = input;
    let mut lrest = lower.as_str();
    while let Some(idx) = lrest.find("bearer") {
        result.push_str(&rest[..idx]);
        let after = &rest[idx + 6..];
        let bs = after.as_bytes();
        // Boşluk ve '=' ayraçlarını say
        let mut k = 0;
        while k < bs.len() && (bs[k] == b' ' || bs[k] == b'=' || bs[k] == b'\t') {
            k += 1;
        }
        result.push_str(&input[idx..idx + 6]);
        // Değerin sonunu bul
        let mut e = k;
        while e < bs.len()
            && !bs[e].is_ascii_whitespace()
            && bs[e] != b','
            && bs[e] != b';'
            && bs[e] != b')'
        {
            e += 1;
        }
        if e > k {
            // Ayraç yoksa bile değer gelmiş (ama boşluk ayracıysa koru)
            if k == 0 {
                result.push(' ');
            }
            result.push_str("[GIZLI]");
        }
        let consumed = 6 + e;
        rest = &rest[idx + consumed..];
        lrest = &lrest[idx + consumed..];
    }
    result.push_str(rest);
    result
}

// ---------------------------------------------------------------------------
// Dosya yazma ve rotasyon.
// ---------------------------------------------------------------------------

/// Uygulama veri klasörünü döndürür ve yoksa oluşturur.
fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|_| "Uygulama veri klasoru bulunamadi".to_string())?;
    fs::create_dir_all(&base).map_err(|e| format!("Veri klasoru olusturulamadi: {}", e))?;
    Ok(base)
}

/// Log dosyasının tam yolunu döndürür (log klasörünü oluşturur).
fn log_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = data_dir(app)?.join(LOG_DIR_NAME);
    fs::create_dir_all(&dir).map_err(|e| format!("Log klasoru olusturulamadi: {}", e))?;
    Ok(dir.join(LOG_FILE_NAME))
}

/// Dosya boyutu sınırı aşıldıysa rotasyon yapar: `es-ops.log.3` silinir,
/// `.2` -> `.3`, `.1` -> `.2`, `es-ops.log` -> `.1`.
fn rotate_if_needed(path: &Path) {
    let Ok(meta) = fs::metadata(path) else {
        return;
    };
    if meta.len() < MAX_LOG_BYTES {
        return;
    }
    let parent = match path.parent() {
        Some(p) => p.to_path_buf(),
        None => return,
    };
    // En eski dosyayı sil
    let oldest = parent.join(format!("{}.{}", LOG_FILE_NAME, MAX_ROTATED_FILES));
    let _ = fs::remove_file(&oldest);
    // Rotasyonla kaydır
    for i in (1..MAX_ROTATED_FILES).rev() {
        let from = parent.join(format!("{}.{}", LOG_FILE_NAME, i));
        let to = parent.join(format!("{}.{}", LOG_FILE_NAME, i + 1));
        if from.exists() {
            let _ = fs::rename(&from, &to);
        }
    }
    let to = parent.join(format!("{}.1", LOG_FILE_NAME));
    let _ = fs::rename(path, &to);
}

/// Tek bir satırı log dosyasına ekler. Gizli bilgi içermez (önceden maskelenmeli).
fn write_line(path: &Path, level: &str, message: &str) -> Result<(), String> {
    rotate_if_needed(path);
    let timestamp = chrono::Local::now().format(TIME_FORMAT);
    let line = format!("[{}] [{}] {}\n", timestamp, level, message);
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("Log dosyasi acilamadi: {}", e))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("Log yazilamadi: {}", e))
}

// ---------------------------------------------------------------------------
// Panic yakalama (Rust panics).
// ---------------------------------------------------------------------------

/// Rust panic bilgisini uygun düzeyde log dosyasına yazar (mümkünse) ve
/// kullanıcıya Türkçe/kısa bir bildirim gösterir. Sessiz kapanmayı önler.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        previous(panic_info);
        let msg = panic_info.to_string();
        // Panic anında Tauri AppHandle erişilemez; önceden önbelleğe alınan
        // veri klasörü varsa kullanılır.
        if let Some(dir) = DATA_DIR.get().and_then(|m| m.lock().unwrap().clone()) {
            let path = dir.join(LOG_DIR_NAME).join(LOG_FILE_NAME);
            let safe = redact_secrets(&msg);
            let _ = write_line(&path, "PANIC", &safe);
        }
        // Kullanıcı dostu uyarı: masaüstünde görünür.
        crate::logging::show_panic_dialog(&msg);
    }));
}

/// Panic penceresi (yalnız Windows; Tauri'nin dialog plugin'ine bağımlı değildir).
#[cfg(windows)]
fn show_panic_dialog(msg: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

    let title: Vec<u16> = "ES OPS - Beklenmeyen Hata".encode_utf16().collect();
    let body = format!(
        "ES OPS beklenmeyen bir hata ile karşılaştı ve kapatılması gerekebilir.\n\nHata özeti aşağıdadır. Destek alırken log dosyalarını paylaşmanız istenebilir.\n\n{}",
        redact_secrets(msg)
    );
    let text: Vec<u16> = body.encode_utf16().collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(not(windows))]
fn show_panic_dialog(_msg: &str) {
    eprintln!("ES OPS panik: {}", redact_secrets(_msg));
}

// ---------------------------------------------------------------------------
// Tauri komutları.
// ---------------------------------------------------------------------------

/// JavaScript tarafından gönderilen log kaydını dosyaya yazar.
/// `message` gizlilik filtrelerinden geçirilir.
#[tauri::command]
pub fn log_append(app: AppHandle, level: String, message: String) -> Result<(), String> {
    let lvl = if level.is_empty() { "INFO".to_string() } else { level.to_uppercase() };
    let path = log_path(&app)?;
    write_line(&path, &lvl, &redact_secrets(&message))
}

/// Belirli bir dosya yoluna (gizli bilgi maskelemeden geçirilmiş) satır yazar.
/// Uygulama başlangıç bilgisini yazmak için kullanılır.
pub fn log_append_to_path(path: &std::path::Path, level: &str, message: &str) -> Result<(), String> {
    write_line(path, level, &redact_secrets(message))
}

/// Uygulamanın log klasörünü işletim sisteminde açar ve yolunu döndürür.
/// Açma başarısız olursa bile yol döndürülür (kullanıcı elle gidebilir).
#[tauri::command]
pub fn log_open_folder(app: AppHandle) -> Result<String, String> {
    let path = log_path(&app)?;
    let dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Log klasoru bulunamadi".to_string())?;
    // İşletim sisteminde klasörü aç (Explorer). İzin gerektirmez; başarısızsa sessiz.
    use tauri_plugin_shell::ShellExt;
    let _ = app.shell().open(dir.to_string_lossy().to_string(), None);
    Ok(dir.to_string_lossy().into_owned())
}

/// Log dosyasını (varsa) kullanıcının seçtiği hedefe kopyalar.
/// Kullanıcı seçim yapmazsa boş dize döner; hata durumunda açıklama döner.
#[tauri::command]
pub fn log_export_to(app: AppHandle, destination: String) -> Result<String, String> {
    let path = log_path(&app)?;
    if !path.exists() {
        return Err("Henuz log dosyasi olusturulmadi".to_string());
    }
    let dest = PathBuf::from(destination.trim());
    if dest.as_os_str().is_empty() {
        return Err("Hedef yol bos".to_string());
    }
    fs::copy(&path, &dest).map_err(|e| format!("Log kopyalanamadi: {}", e))?;
    Ok(dest.to_string_lossy().into_owned())
}

// ---------------------------------------------------------------------------
// Testler.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_masks_api_key_value() {
        let out = redact_secrets("api_key = sk-1234567890abcdef sonraki");
        assert!(!out.contains("sk-1234567890abcdef"), "api key sizdi: {}", out);
        assert!(out.contains("[GIZLI]"));
    }

    #[test]
    fn redact_masks_client_secret() {
        let out = redact_secrets("client_secret: SUPERGIZLISECRET123");
        assert!(!out.contains("SUPERGIZLISECRET123"), "client secret sizdi: {}", out);
        assert!(out.contains("[GIZLI]"));
    }

    #[test]
    fn redact_masks_password_and_token() {
        let out = redact_secrets("password=gecerliParola123 access_token=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
        assert!(!out.contains("gecerliParola123"), "parola sizdi: {}", out);
        assert!(!out.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"), "token sizdi: {}", out);
    }

    #[test]
    fn redact_masks_license_code() {
        let out = redact_secrets("license_code = LIC-20260701-000001");
        assert!(!out.contains("LIC-20260701-000001"), "lisans kodu sizdi: {}", out);
        assert!(out.contains("[GIZLI]"));
    }

    #[test]
    fn redact_masks_bearer() {
        let out = redact_secrets("Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.abc.def");
        assert!(!out.contains("eyJhbGciOiJIUzI1NiJ9"), "bearer sizdi: {}", out);
    }

    #[test]
    fn redact_keeps_normal_text() {
        let out = redact_secrets("Baglanti basarili: Instagram hesap acildi.");
        assert!(out.contains("Instagram hesap acildi"));
        assert!(!out.contains("[GIZLI]"));
    }

    #[test]
    fn redact_keeps_error_codes() {
        let out = redact_secrets("youtube_not_configured: YouTube kimlikleri eksik");
        assert!(out.contains("youtube_not_configured"));
        assert!(out.contains("YouTube kimlikleri eksik"));
    }

    #[test]
    fn write_line_appends_and_creates_dir() {
        let tmp = std::env::temp_dir().join(format!("es_ops_log_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let path = tmp.join("logs").join(LOG_FILE_NAME);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_line(&path, "INFO", "ilk").unwrap();
        write_line(&path, "ERROR", "ikinci").unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("[INFO] ilk"));
        assert!(content.contains("[ERROR] ikinci"));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn write_line_masks_secrets() {
        let tmp = std::env::temp_dir().join(format!("es_ops_log_secret_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        let path = tmp.join("logs").join(LOG_FILE_NAME);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_line(&path, "ERROR", &redact_secrets("token = TPK-1234567890")).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("TPK-1234567890"));
        let _ = fs::remove_dir_all(&tmp);
    }
}