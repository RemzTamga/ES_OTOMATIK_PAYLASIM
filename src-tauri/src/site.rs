//! Web Sitesi yayÄ±n entegrasyonu.
//!
//! GerÃ§ek bir web API sÃ¶zleÅŸmesi bu projede tanÄ±mlÄ± DEÄÄ°LDÄ°R. Bu nedenle
//! sabit bir endpoint/biÃ§im varsayÄ±lmaz; tÃ¼m sÃ¶zleÅŸme kullanÄ±cÄ± tarafÄ±ndan
//! ayarlarda verilir (temel adres, kimlik tÃ¼rÃ¼, test/yayÄ±n/bÃ¶lÃ¼m endpointleri,
//! yanÄ±t alan yollarÄ±, gÃ¶nderim ÅŸablonu). VarsayÄ±m veya sahte endpoint Ã¼retilmez.
//!
//! GÃ¼venlik kurallarÄ±:
//! - API anahtarÄ±/token yalnÄ±z Windows Credential Manager'da (keyring) saklanÄ±r.
//! - Anahtar; JavaScript'e, localStorage'a, ayar dosyasÄ±na veya loglara
//!   dÃ¼z metin olarak yazÄ±lmaz, dÃ¶ndÃ¼rÃ¼lmez.
//! - YalnÄ±z gizli olmayan yapÄ±landÄ±rma JSON dosyasÄ±nda tutulur.
//!
//! HTTP istekleri gerÃ§ektir (reqwest blocking). BaÅŸarÄ±/baÅŸarÄ±sÄ±zlÄ±k sahte
//! Ã¼retilmez; her durum iÃ§in kullanÄ±cÄ± dostu TÃ¼rkÃ§e aÃ§Ä±klama dÃ¶ndÃ¼rÃ¼lÃ¼r.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

/// API anahtarÄ±nÄ±n Credential Manager'daki kayÄ±t anahtarÄ±.
const CRED_SERVICE: &str = "com.es.ops::website::default";
const CRED_USER: &str = "api_secret";
/// Gizli olmayan yapÄ±landÄ±rma dosyasÄ±.
const CONFIG_FILE: &str = "website_config.json";
/// YanÄ±t gÃ¶vdesi Ã¶nizlemeleri bu uzunlukta kesilir (log/gizlilik bilinÃ§li).
const BODY_PREVIEW_LIMIT: usize = 400;
/// JSON ÅŸablonunda desteklenen yer tutucular.
const PLACEHOLDERS: [&str; 5] = ["{{baslik}}", "{{icerik}}", "{{bolum_id}}", "{{bolum_adi}}", "{{medya_base64}}"];

// ---------------------------------------------------------------------------
// YapÄ±landÄ±rma (gizli olmayan)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct SiteConfig {
    /// Hedef web sitesi adresi (gÃ¶rÃ¼ntÃ¼leme amaÃ§lÄ±).
    pub site_url: String,
    /// API temel adresi, Ã¶r. https://api.ornek.com/v1
    pub api_base: String,
    /// "bearer" | "api_key" | "none"
    pub auth_style: String,
    /// API Key gÃ¶nderiminde baÅŸlÄ±k adÄ± (Ã¶rn. X-API-Key).
    pub auth_header_name: String,
    /// BaÄŸlantÄ± testi endpoint'i (tam URL veya temel adrese gÃ¶reli yol).
    pub test_endpoint: String,
    /// YayÄ±n endpoint'i.
    pub publish_endpoint: String,
    /// BÃ¶lÃ¼m listesi endpoint'i (opsiyonel; boÅŸ ise bÃ¶lÃ¼mler API'den alÄ±nmaz).
    pub sections_endpoint: String,
    /// BÃ¶lÃ¼m listesindeki dizinin yolu, Ã¶r. "data" veya "data.items".
    pub sections_path: String,
    /// BÃ¶lÃ¼m Ã¶ÄŸesinde kimlik alanÄ±nÄ±n yolu, Ã¶r. "id".
    pub section_id_path: String,
    /// BÃ¶lÃ¼m Ã¶ÄŸesinde ad alanÄ±nÄ±n yolu, Ã¶r. "name".
    pub section_name_path: String,
    /// YayÄ±n yanÄ±tÄ±nda iÃ§erik kimliÄŸi alanÄ±nÄ±n yolu, Ã¶r. "data.id".
    pub content_id_path: String,
    /// YayÄ±n yanÄ±tÄ±nda iÃ§erik URL alanÄ±nÄ±n yolu, Ã¶r. "data.url".
    pub content_url_path: String,
    /// Hata yanÄ±tÄ±nda mesaj alanÄ±nÄ±n yolu, Ã¶r. "error.message".
    pub error_message_path: String,
    /// Ã‡ok parÃ§alÄ± (multipart) gÃ¶nderimde medya alanÄ± adÄ±, Ã¶r. "image".
    pub media_field_name: String,
    /// true ise medya dosyasÄ± multipart ile gÃ¶nderilir.
    pub multipart: bool,
    /// HTTP zaman aÅŸÄ±mÄ± saniye.
    pub timeout_seconds: u64,
    /// GÃ¶nderim gÃ¶vdesi JSON ÅŸablonu. Yer tutucular: {{baslik}}, {{icerik}},
    /// {{bolum_id}}, {{bolum_adi}}, {{medya_base64}}. Multipart modunda ÅŸablon
    /// dÃ¼z (iÃ§ iÃ§e olmayan) bir nesne olmalÄ±; her alan bir form parÃ§asÄ± olur.
    pub payload_template: String,
}

impl Default for SiteConfig {
    fn default() -> Self {
        SiteConfig {
            site_url: String::new(),
            api_base: String::new(),
            auth_style: "bearer".to_string(),
            auth_header_name: "X-API-Key".to_string(),
            test_endpoint: String::new(),
            publish_endpoint: String::new(),
            sections_endpoint: String::new(),
            sections_path: "data".to_string(),
            section_id_path: "id".to_string(),
            section_name_path: "name".to_string(),
            content_id_path: "data.id".to_string(),
            content_url_path: "data.url".to_string(),
            error_message_path: "error.message".to_string(),
            media_field_name: "image".to_string(),
            multipart: false,
            timeout_seconds: 15,
            payload_template: "{\"title\":\"{{baslik}}\",\"content\":\"{{icerik}}\",\"section_id\":\"{{bolum_id}}\",\"section_name\":\"{{bolum_adi}}\"}"
                .to_string(),
        }
    }
}

/// Ã–n yÃ¼ze dÃ¶ndÃ¼rÃ¼len yapÄ±landÄ±rma gÃ¶rÃ¼nÃ¼mÃ¼. Anahtar/token iÃ§ermez.
#[derive(Serialize, Clone, Debug)]
pub struct SiteConfigView {
    pub configured: bool,
    pub has_credential: bool,
    #[serde(flatten)]
    pub cfg: SiteConfig,
    pub last_test: String,
}

/// YapÄ±landÄ±rma yazma isteÄŸi. `secret` yalnÄ±z yazma anÄ±nda kullanÄ±lÄ±r ve
/// hiÃ§bir Ã§Ä±ktÄ±ya/kayda dahil edilmez.
#[derive(Deserialize, Debug)]
pub struct SiteConfigInput {
    pub site_url: String,
    pub api_base: String,
    pub auth_style: String,
    pub auth_header_name: String,
    pub test_endpoint: String,
    pub publish_endpoint: String,
    pub sections_endpoint: String,
    pub sections_path: String,
    pub section_id_path: String,
    pub section_name_path: String,
    pub content_id_path: String,
    pub content_url_path: String,
    pub error_message_path: String,
    pub media_field_name: String,
    pub multipart: bool,
    pub timeout_seconds: u64,
    pub payload_template: String,
    /// BoÅŸ deÄŸilse Credential Manager'a yazÄ±lÄ±r; boÅŸsa mevcut anahtar korunur.
    pub secret: String,
}

/// YayÄ±n isteÄŸi. Gizli bilgi iÃ§ermez; anahtar Rust tarafÄ±ndan depodan okunur.
#[derive(Deserialize, Debug)]
pub struct SitePublishInput {
    pub title: String,
    pub content: String,
    pub section_id: String,
    pub section_name: String,
    pub media_path: String,
}

/// Test/bÃ¶lÃ¼m/yayÄ±n iÅŸlemlerinin ortak sonuÃ§ yapÄ±sÄ±.
#[derive(Serialize, Clone, Debug)]
pub struct SiteOutcome {
    pub ok: bool,
    pub code: String,
    pub turkce: String,
    pub http_status: Option<u16>,
    pub content_id: String,
    pub content_url: String,
    pub body_preview: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct SiteSection {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct SiteSectionsOutcome {
    pub ok: bool,
    pub turkce: String,
    pub sections: Vec<SiteSection>,
    pub body_preview: String,
}

// ---------------------------------------------------------------------------
// KalÄ±cÄ±lÄ±k (yalnÄ±z gizli olmayan yapÄ±landÄ±rma)
// ---------------------------------------------------------------------------

fn config_file(data_dir: &Path) -> PathBuf {
    data_dir.join("website").join(CONFIG_FILE)
}

fn load_config(data_dir: &Path) -> Result<SiteConfig, String> {
    let path = config_file(data_dir);
    if !path.exists() {
        return Ok(SiteConfig::default());
    }
    let raw = fs::read_to_string(&path).map_err(|_| "Yapilandirma dosyasi okunamadi.".to_string())?;
    serde_json::from_str(&raw).map_err(|_| "Yapilandirma dosyasi bozuk.".to_string())
}

fn save_config(data_dir: &Path, cfg: &SiteConfig) -> Result<(), String> {
    let dir = data_dir.join("website");
    fs::create_dir_all(&dir).map_err(|_| "Yapilandirma klasoru olusturulamadi.".to_string())?;
    let raw = serde_json::to_string_pretty(cfg).map_err(|_| "Yapilandirma yazilamadi.".to_string())?;
    fs::write(config_file(data_dir), raw).map_err(|_| "Yapilandirma yazilamadi.".to_string())
}

// ---------------------------------------------------------------------------
// GÃ¼venli kimlik deposu (Windows Credential Manager)
// ---------------------------------------------------------------------------

fn store_secret(secret: &str) -> Result<(), String> {
    if secret.trim().is_empty() {
        return Err("API anahtari bos olamaz.".to_string());
    }
    keyring::v1::Entry::new(CRED_SERVICE, CRED_USER)
        .map_err(|_| "Guvenli depo kullanilamiyor.".to_string())?
        .set_password(secret)
        .map_err(|_| "API anahtari guvenli depoya yazilamadi.".to_string())
}

fn load_secret() -> Option<String> {
    let entry = keyring::v1::Entry::new(CRED_SERVICE, CRED_USER).ok()?;
    match entry.get_password() {
        Ok(secret) if !secret.is_empty() => Some(secret),
        _ => None,
    }
}

fn clear_secret() {
    if let Ok(entry) = keyring::v1::Entry::new(CRED_SERVICE, CRED_USER) {
        let _ = entry.delete_credential();
    }
}

// ---------------------------------------------------------------------------
// HTTP yardÄ±mcÄ±larÄ±
// ---------------------------------------------------------------------------

fn base_join(base: &str, endpoint: &str) -> Result<String, String> {
    let base = base.trim().trim_end_matches('/');
    let endpoint = endpoint.trim();
    if base.is_empty() {
        return Err("API temel adresi bos.".to_string());
    }
    if endpoint.is_empty() {
        return Err("Endpoint adresi bos.".to_string());
    }
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return Ok(endpoint.to_string());
    }
    Ok(format!("{}/{}", base, endpoint.trim_start_matches('/')))
}

fn auth_header(cfg: &SiteConfig, secret: &str) -> Option<(String, String)> {
    match cfg.auth_style.as_str() {
        "bearer" => Some(("Authorization".to_string(), format!("Bearer {}", secret))),
        "api_key" => {
            let name = if cfg.auth_header_name.trim().is_empty() {
                "X-API-Key"
            } else {
                cfg.auth_header_name.trim()
            };
            Some((name.to_string(), secret.to_string()))
        }
        _ => None,
    }
}

fn client(cfg: &SiteConfig) -> reqwest::blocking::Client {
    let secs = if cfg.timeout_seconds == 0 { 15 } else { cfg.timeout_seconds };
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(secs))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

/// JSON iÃ§inde noktalÄ± yol ile deÄŸer arar: "data.items[0].name".
fn json_path_value<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path.split('.') {
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }
        // [i] indekslerini destekle: "items[0]"
        let bracket = seg.find('[');
        let (name, idx) = match bracket {
            Some(b) => {
                let head = &seg[..b];
                let rest = &seg[b..];
                let idx_str = rest
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .trim();
                let parsed: Option<usize> = idx_str.parse().ok();
                (head, parsed)
            }
            None => (seg, None),
        };
        cur = match (name, idx) {
            ("", Some(i)) => cur.get(i)?,
            (n, None) => cur.get(n)?,
            (n, Some(i)) => {
                let arr = cur.get(n)?;
                arr.get(i)?
            }
        };
    }
    Some(cur)
}

fn json_path_string(root: &Value, path: &str) -> Option<String> {
    let v = json_path_value(root, path)?;
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Åablon iÃ§indeki yer tutucularÄ± deÄŸiÅŸtirir; JSON dizesi olarak gÃ¼venli.
/// DeÄŸerler JSON iÃ§ine kodlanarak yerleÅŸtirilir (tÄ±rnak/kaÃ§Ä±ÅŸ sorunu yok).
fn fill_template(template: &str, values: &[(&str, String)]) -> Result<Value, String> {
    let mut root: Value =
        serde_json::from_str(template).map_err(|e| format!("Gonderim sablonu gecersiz JSON: {}", e))?;
    fill_value(&mut root, values);
    Ok(root)
}

fn fill_value(node: &mut Value, values: &[(&str, String)]) {
    match node {
        Value::String(s) => {
            for (ph, v) in values {
                if s.contains(ph) {
                    *s = s.replace(ph, v);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                fill_value(item, values);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                fill_value(v, values);
            }
        }
        _ => {}
    }
}

/// Multipart modu: ÅŸablon dÃ¼z bir nesne olmalÄ±; her dize alanÄ± bir form parÃ§asÄ±.
fn template_to_form_parts(template: &str, values: &[(&str, String)]) -> Result<Vec<(String, String)>, String> {
    let filled = fill_template(template, values)?;
    let obj = filled
        .as_object()
        .ok_or("Multipart modunda gonderim sablonu nesne olmalidir.".to_string())?;
    let mut parts = Vec::new();
    for (key, val) in obj {
        let s = match val {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            _ => {
                return Err("Multipart modunda sablon alanlari dize olmalidir.".to_string());
            }
        };
        parts.push((key.clone(), s));
    }
    Ok(parts)
}

fn body_preview(body: &str) -> String {
    let t: String = body.chars().take(BODY_PREVIEW_LIMIT).collect();
    t
}

// ---------------------------------------------------------------------------
// Hata kodu -> kullanÄ±cÄ± dostu TÃ¼rkÃ§e mesaj
// ---------------------------------------------------------------------------

fn status_turkce(status: u16, detail: &str) -> (String, String) {
    let code = match status {
        401 => "http_401",
        403 => "http_403",
        404 => "http_404",
        409 => "http_409",
        422 => "http_422",
        400 => "http_400",
        s if (500..=599).contains(&s) => "http_5xx",
        _ => "http_other",
    };
    let mesaj = match status {
        401 => "Kimlik dogrulama basarisiz (401). API anahtari/token gecersiz veya suresi dolmus olabilir. Ayarlardan anahtari yenileyin."
            .to_string(),
        403 => "Yetki reddedildi (403). Bu API anahtari ile yayin yapma izniniz yok."
            .to_string(),
        404 => "Hedef adres bulunamadi (404). Endpoint adresi ayarlarda yanlis olabilir; API dokumantasyonu ile karsilastirin."
            .to_string(),
        409 => "Cakisma olustu (409). Ayni icerik daha once yayinlanmis olabilir veya hedef bolum durumu uyusmuyor."
            .to_string(),
        422 => "Icerik reddedildi (422). Gonderilen alanlar API sozlesmesine uymuyor. Sablon ve alan yollarini API dokumantasyonu ile kontrol edin."
            .to_string(),
        400 => "Gecersiz istek (400). Gonderilen veri API tarafindan kabul edilmedi."
            .to_string(),
        s if (500..=599).contains(&s) => {
            format!("Sunucu hatasi ({}). Web sitesi tarafinda gecici bir sorun olabilir; daha sonra tekrar deneyin.", s)
        }
        _ => format!("Beklenmeyen yanit ({}).", status),
    };
    let mut final_msg = mesaj;
    if !detail.is_empty() {
        final_msg.push_str(" Sunucu aciklamasi: ");
        final_msg.push_str(&detail);
    }
    (code.to_string(), final_msg)
}

fn network_turkce(err: &reqwest::Error) -> (String, String) {
    if err.is_timeout() {
        (
            "network_timeout".to_string(),
            "Baglanti zamani asildi: sunucu belirlenen sure icinde yanit vermedi. API adresini ve ag baglantinizi kontrol edin."
                .to_string(),
        )
    } else if err.is_connect() {
        (
            "network_connect".to_string(),
            "Baglanti kurulamadi: sunucuya ulasilamadi veya baglanti reddedildi. API temel adresini kontrol edin."
                .to_string(),
        )
    } else if err.is_request() {
        (
            "network_request".to_string(),
            "Istek gonderilemedi: adres gecersiz veya ag baglantisi yok. API temel adresini kontrol edin.".to_string(),
        )
    } else if err.is_decode() || err.is_body() {
        (
            "network_body".to_string(),
            "Sunucu yaniti okunamadi: yanit beklenen bicimde degil.".to_string(),
        )
    } else {
        (
            "network_error".to_string(),
            "Ag hatasi olustu. Ayarlardaki API bilgilerini ve internet baglantinizi kontrol edin.".to_string(),
        )
    }
}

// ---------------------------------------------------------------------------
// GerÃ§ek iÅŸlemler (saf fonksiyonlar; test edilebilir)
// ---------------------------------------------------------------------------

fn read_media_base64(path: &str) -> Result<Option<String>, String> {
    if path.trim().is_empty() {
        return Ok(None);
    }
    let bytes = fs::read(path).map_err(|_| "Medya dosyasi okunamadi. Dosya silinmis veya tasinmis olabilir.".to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("Medya dosyasi 8 MB uzerinde; API anahtari olmadan gonderilmesi onerilmez.".to_string());
    }
    use base64::Engine;
    Ok(Some(base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// BaÄŸlantÄ± testi: gerÃ§ek GET isteÄŸi. Sahte sonuÃ§ Ã¼retilmez.
pub fn run_test(cfg: &SiteConfig, secret: &str, timeout: Duration) -> SiteOutcome {
    let url = match base_join(&cfg.api_base, &cfg.test_endpoint) {
        Ok(u) => u,
        Err(e) => {
            return SiteOutcome {
                ok: false,
                code: "config".to_string(),
                turkce: e,
                http_status: None,
                content_id: String::new(),
                content_url: String::new(),
                body_preview: String::new(),
            };
        }
    };
    let mut req = client_timeout(cfg, timeout).get(&url);
    if let Some((name, val)) = auth_header(cfg, secret) {
        req = req.header(name, val);
    }
    match req.send() {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().unwrap_or_default();
            let preview = body_preview(&body);
            if (200..300).contains(&status) {
                let server_msg = json_path_string(&serde_json::from_str::<Value>(&body).unwrap_or(Value::Null), &cfg.error_message_path)
                    .unwrap_or_default();
                let mut turkce = "Baglanti basarili.".to_string();
                if !server_msg.is_empty() {
                    turkce.push_str(&format!(" Sunucu mesaji: {}", server_msg));
                }
                SiteOutcome {
                    ok: true,
                    code: "ok".to_string(),
                    turkce,
                    http_status: Some(status),
                    content_id: String::new(),
                    content_url: String::new(),
                    body_preview: preview,
                }
            } else {
                let detail = extract_error_detail(&cfg.error_message_path, &body);
                let (code, turkce) = status_turkce(status, &detail);
                SiteOutcome {
                    ok: false,
                    code,
                    turkce,
                    http_status: Some(status),
                    content_id: String::new(),
                    content_url: String::new(),
                    body_preview: preview,
                }
            }
        }
        Err(e) => {
            let (code, turkce) = network_turkce(&e);
            SiteOutcome {
                ok: false,
                code,
                turkce,
                http_status: None,
                content_id: String::new(),
                content_url: String::new(),
                body_preview: String::new(),
            }
        }
    }
}

fn client_timeout(cfg: &SiteConfig, timeout: Duration) -> reqwest::blocking::Client {
    let t = if !timeout.is_zero() {
        timeout
    } else if cfg.timeout_seconds > 0 {
        Duration::from_secs(cfg.timeout_seconds)
    } else {
        Duration::from_secs(15)
    };
    reqwest::blocking::Client::builder()
        .timeout(t)
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

fn extract_error_detail(path: &str, body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let p = if path.trim().is_empty() { "error.message" } else { path };
    if let Some(m) = json_path_string(&v, p) {
        return m;
    }
    if let Some(m) = json_path_string(&v, "message") {
        return m;
    }
    if let Some(m) = json_path_string(&v, "error") {
        return m;
    }
    String::new()
}

/// YayÄ±n: gerÃ§ek HTTP isteÄŸi (ÅŸablon tabanlÄ±; multipart veya JSON).
pub fn run_publish(
    cfg: &SiteConfig,
    secret: &str,
    input: &SitePublishInput,
    timeout: Duration,
) -> SiteOutcome {
    let url = match base_join(&cfg.api_base, &cfg.publish_endpoint) {
        Ok(u) => u,
        Err(e) => {
            return SiteOutcome {
                ok: false,
                code: "config".to_string(),
                turkce: e,
                http_status: None,
                content_id: String::new(),
                content_url: String::new(),
                body_preview: String::new(),
            };
        }
    };

    let mut values: Vec<(&str, String)> = vec![
        ("{{baslik}}", input.title.clone()),
        ("{{icerik}}", input.content.clone()),
        ("{{bolum_id}}", input.section_id.clone()),
        ("{{bolum_adi}}", input.section_name.clone()),
    ];
    if cfg.payload_template.contains("{{medya_base64}}") {
        match read_media_base64(&input.media_path) {
            Ok(Some(b64)) => values.push(("{{medya_base64}}", b64)),
            Ok(None) => values.push(("{{medya_base64}}", String::new())),
            Err(e) => {
                return SiteOutcome {
                    ok: false,
                    code: "media".to_string(),
                    turkce: e,
                    http_status: None,
                    content_id: String::new(),
                    content_url: String::new(),
                    body_preview: String::new(),
                };
            }
        }
    }

    let client = client_timeout(cfg, timeout);
    let mut builder = client.post(&url);
    if let Some((name, val)) = auth_header(cfg, secret) {
        builder = builder.header(name, val);
    }

    let prepared = if cfg.multipart {
        let parts = match template_to_form_parts(&cfg.payload_template, &values) {
            Ok(p) => p,
            Err(e) => {
                return SiteOutcome {
                    ok: false,
                    code: "template".to_string(),
                    turkce: e,
                    http_status: None,
                    content_id: String::new(),
                    content_url: String::new(),
                    body_preview: String::new(),
                };
            }
        };
        let mut form = reqwest::blocking::multipart::Form::new();
        for (k, v) in parts {
            form = form.text(k, v);
        }
        if !input.media_path.trim().is_empty() {
            let media_field = if cfg.media_field_name.trim().is_empty() {
                "image".to_string()
            } else {
                cfg.media_field_name.trim().to_string()
            };
            match reqwest::blocking::multipart::Part::file(&input.media_path) {
                Ok(part) => {
                    form = form.part(media_field, part);
                }
                Err(e) => {
                    return SiteOutcome {
                        ok: false,
                        code: "media".to_string(),
                        turkce: format!("Medya dosyasi eklenemedi: {}", e),
                        http_status: None,
                        content_id: String::new(),
                        content_url: String::new(),
                        body_preview: String::new(),
                    };
                }
            }
        }
        builder.multipart(form)
    } else {
        let body = match fill_template(&cfg.payload_template, &values) {
            Ok(b) => b,
            Err(e) => {
                return SiteOutcome {
                    ok: false,
                    code: "template".to_string(),
                    turkce: e,
                    http_status: None,
                    content_id: String::new(),
                    content_url: String::new(),
                    body_preview: String::new(),
                };
            }
        };
        builder.json(&body)
    };

    match prepared.send() {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().unwrap_or_default();
            let preview = body_preview(&body);
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            if (200..300).contains(&status) {
                let content_id = json_path_string(&v, &cfg.content_id_path).unwrap_or_default();
                let content_url = json_path_string(&v, &cfg.content_url_path).unwrap_or_default();
                let server_msg = extract_error_detail(&cfg.error_message_path, &body);
                let mut turkce = "Yayin basarili.".to_string();
                if !content_id.is_empty() {
                    turkce.push_str(&format!(" Icerik ID: {}", content_id));
                }
                if !content_url.is_empty() {
                    turkce.push_str(&format!(" Adres: {}", content_url));
                }
                if !server_msg.is_empty() && content_id.is_empty() && content_url.is_empty() {
                    turkce.push_str(&format!(" Sunucu mesaji: {}", server_msg));
                }
                SiteOutcome {
                    ok: true,
                    code: "ok".to_string(),
                    turkce,
                    http_status: Some(status),
                    content_id,
                    content_url,
                    body_preview: preview,
                }
            } else {
                let detail = extract_error_detail(&cfg.error_message_path, &body);
                let (code, turkce) = status_turkce(status, &detail);
                SiteOutcome {
                    ok: false,
                    code,
                    turkce,
                    http_status: Some(status),
                    content_id: String::new(),
                    content_url: String::new(),
                    body_preview: preview,
                }
            }
        }
        Err(e) => {
            let (code, turkce) = network_turkce(&e);
            SiteOutcome {
                ok: false,
                code,
                turkce,
                http_status: None,
                content_id: String::new(),
                content_url: String::new(),
                body_preview: String::new(),
            }
        }
    }
}

/// BÃ¶lÃ¼m listesi: API destekliyorsa gerÃ§ek istekle alÄ±nÄ±r.
pub fn run_sections(cfg: &SiteConfig, secret: &str, timeout: Duration) -> SiteSectionsOutcome {
    if cfg.sections_endpoint.trim().is_empty() {
        return SiteSectionsOutcome {
            ok: false,
            turkce: "Bolum listesi endpoint'i tanimli degil. Bolum kimligini elle girebilirsiniz.".to_string(),
            sections: Vec::new(),
            body_preview: String::new(),
        };
    }
    let url = match base_join(&cfg.api_base, &cfg.sections_endpoint) {
        Ok(u) => u,
        Err(e) => {
            return SiteSectionsOutcome {
                ok: false,
                turkce: e,
                sections: Vec::new(),
                body_preview: String::new(),
            };
        }
    };
    let mut req = client_timeout(cfg, timeout).get(&url);
    if let Some((name, val)) = auth_header(cfg, secret) {
        req = req.header(name, val);
    }
    match req.send() {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body = resp.text().unwrap_or_default();
            let preview = body_preview(&body);
            if (200..300).contains(&status) {
                let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
                let list_path = if cfg.sections_path.trim().is_empty() { "data" } else { cfg.sections_path.trim() };
                let arr = json_path_value(&v, list_path)
                    .and_then(|x| x.as_array());
                let mut sections = Vec::new();
                if let Some(items) = arr {
                    for item in items {
                        let id = json_path_string(item, &cfg.section_id_path).unwrap_or_default();
                        let name = json_path_string(item, &cfg.section_name_path).unwrap_or_default();
                        if !id.is_empty() {
                            sections.push(SiteSection { id: id.clone(), name: if name.is_empty() { id.clone() } else { name } });
                        }
                    }
                }
                SiteSectionsOutcome {
                    ok: true,
                    turkce: if sections.is_empty() {
                        "Bolum listesi bos geldi; bolum kimligini elle girebilirsiniz.".to_string()
                    } else {
                        format!("{} bolum bulundu.", sections.len())
                    },
                    sections,
                    body_preview: preview,
                }
            } else {
                let detail = extract_error_detail(&cfg.error_message_path, &body);
                let (_, turkce) = status_turkce(status, &detail);
                SiteSectionsOutcome {
                    ok: false,
                    turkce,
                    sections: Vec::new(),
                    body_preview: preview,
                }
            }
        }
        Err(e) => {
            let (_, turkce) = network_turkce(&e);
            SiteSectionsOutcome {
                ok: false,
                turkce,
                sections: Vec::new(),
                body_preview: String::new(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tauri komutlarÄ±
// ---------------------------------------------------------------------------

fn data_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|_| "Uygulama veri klasoru bulunamadi.".to_string())
}

fn secret_or_error(cfg: &SiteConfig) -> Result<String, SiteOutcome> {
    if cfg.auth_style != "none" {
        if let Some(secret) = load_secret() {
            return Ok(secret);
        }
        return Err(SiteOutcome {
            ok: false,
            code: "credential_missing".to_string(),
            turkce: "API anahtari/token saklanmamis. Ayarlardan anahtari girip Kaydet butonuna basin.".to_string(),
            http_status: None,
            content_id: String::new(),
            content_url: String::new(),
            body_preview: String::new(),
        });
    }
    Ok(String::new())
}

#[tauri::command]
pub fn website_config_get(app: AppHandle) -> Result<SiteConfigView, String> {
    let dir = data_dir(&app)?;
    let cfg = load_config(&dir)?;
    let has_credential = load_secret().is_some();
    let configured = !cfg.api_base.trim().is_empty() && !cfg.test_endpoint.trim().is_empty();
    Ok(SiteConfigView {
        configured,
        has_credential,
        cfg,
        last_test: String::new(),
    })
}

#[tauri::command]
pub fn website_config_save(app: AppHandle, input: SiteConfigInput) -> Result<SiteConfigView, String> {
    let dir = data_dir(&app)?;
    let cfg = SiteConfig {
        site_url: input.site_url.trim().to_string(),
        api_base: input.api_base.trim().to_string(),
        auth_style: if input.auth_style.is_empty() { "bearer".to_string() } else { input.auth_style },
        auth_header_name: input.auth_header_name.trim().to_string(),
        test_endpoint: input.test_endpoint.trim().to_string(),
        publish_endpoint: input.publish_endpoint.trim().to_string(),
        sections_endpoint: input.sections_endpoint.trim().to_string(),
        sections_path: if input.sections_path.trim().is_empty() { "data".to_string() } else { input.sections_path.trim().to_string() },
        section_id_path: if input.section_id_path.trim().is_empty() { "id".to_string() } else { input.section_id_path.trim().to_string() },
        section_name_path: if input.section_name_path.trim().is_empty() { "name".to_string() } else { input.section_name_path.trim().to_string() },
        content_id_path: if input.content_id_path.trim().is_empty() { "data.id".to_string() } else { input.content_id_path.trim().to_string() },
        content_url_path: if input.content_url_path.trim().is_empty() { "data.url".to_string() } else { input.content_url_path.trim().to_string() },
        error_message_path: if input.error_message_path.trim().is_empty() { "error.message".to_string() } else { input.error_message_path.trim().to_string() },
        media_field_name: input.media_field_name.trim().to_string(),
        multipart: input.multipart,
        timeout_seconds: if input.timeout_seconds == 0 { 15 } else { input.timeout_seconds },
        payload_template: input.payload_template,
    };
    if !input.secret.trim().is_empty() {
        store_secret(&input.secret)?;
    }
    save_config(&dir, &cfg)?;
    let has_credential = load_secret().is_some();
    Ok(SiteConfigView {
        configured: !cfg.api_base.is_empty() && !cfg.test_endpoint.is_empty(),
        has_credential,
        cfg,
        last_test: String::new(),
    })
}

#[tauri::command]
pub fn website_config_clear(app: AppHandle) -> Result<(), String> {
    clear_secret();
    let dir = data_dir(&app)?;
    let _ = fs::remove_file(config_file(&dir));
    Ok(())
}

#[tauri::command]
pub fn website_test(app: AppHandle) -> Result<SiteOutcome, String> {
    let dir = data_dir(&app)?;
    let cfg = load_config(&dir)?;
    let secret = secret_or_error(&cfg).map_err(|o| o.turkce)?;
    Ok(run_test(&cfg, &secret, Duration::ZERO))
}

#[tauri::command]
pub fn website_publish(app: AppHandle, input: SitePublishInput) -> Result<SiteOutcome, String> {
    let dir = data_dir(&app)?;
    let cfg = load_config(&dir)?;
    let secret = secret_or_error(&cfg).map_err(|o| o.turkce)?;
    Ok(run_publish(&cfg, &secret, &input, Duration::ZERO))
}

#[tauri::command]
pub fn website_sections(app: AppHandle) -> Result<SiteSectionsOutcome, String> {
    let dir = data_dir(&app)?;
    let cfg = load_config(&dir)?;
    let secret = secret_or_error(&cfg).map_err(|o| o.turkce)?;
    Ok(run_sections(&cfg, &secret, Duration::ZERO))
}

// ---------------------------------------------------------------------------
// Testler: gerÃ§ek HTTP akÄ±ÅŸÄ± std::net ile kurulan yerel mock sunucu Ã¼zerinden
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    /// Ã‡ok basit, baÄŸÄ±mlÄ±lÄ±ksÄ±z mock HTTP sunucusu.
    /// Kural: satÄ±r isteÄŸin ilk satÄ±rÄ±ndaki "GET /path" veya "POST /path".
    struct MockServer {
        addr: String,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        handle: Option<std::thread::JoinHandle<()>>,
    }

    impl MockServer {
        fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stop2 = stop.clone();
            let handle = std::thread::spawn(move || {
                listener.set_nonblocking(true).ok();
                let mut conns: Vec<TcpStream> = Vec::new();
                loop {
                    if stop2.load(std::sync::atomic::Ordering::Relaxed) {
                        break;
                    }
                    match listener.accept() {
                        Ok((stream, _)) => {
                            stream.set_nonblocking(true).ok();
                            conns.push(stream);
                        }
                        Err(_) => {}
                    }
                    let mut i = 0;
                    while i < conns.len() {
                        let mut buf = [0u8; 4096];
                        match conns[i].read(&mut buf) {
                            Ok(0) | Err(_) => {
                                i += 1;
                                continue;
                            }
                            Ok(n) => {
                                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                                let mut lines = req.lines();
                                let first = lines.next().unwrap_or("GET /").to_string();
                                let (method, path) = split_method_path(&first);
                                let resp = route(&method, &path);
                                let _ = conns[i].write_all(resp.as_bytes());
                                conns.remove(i);
                                break;
                            }
                        }
                        i += 1;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            });
            MockServer {
                addr: format!("http://{}", addr),
                stop,
                handle: Some(handle),
            }
        }

        fn base(&self) -> String {
            self.addr.clone()
        }
    }

    impl Drop for MockServer {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            if let Some(h) = self.handle.take() {
                let _ = h.join();
            }
        }
    }

    fn split_method_path(first: &str) -> (String, String) {
        let mut it = first.split_whitespace();
        let method = it.next().unwrap_or("GET").to_string();
        let path = it.next().unwrap_or("/").to_string();
        (method, path)
    }

    fn response(status: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            status,
            body.len(),
            body
        )
    }

    fn route(method: &str, path: &str) -> String {
        match (method, path) {
            ("GET", "/ok") => response("200 OK", r#"{"ok":true}"#),
            ("GET", "/auth401") => response("401 Unauthorized", r#"{"error":{"message":"invalid token"}}"#),
            ("GET", "/forbidden403") => response("403 Forbidden", r#"{"error":{"message":"no permission"}}"#),
            ("GET", "/notfound404") => response("404 Not Found", r#"{"error":{"message":"no such route"}}"#),
            ("GET", "/conflict409") => response("409 Conflict", r#"{"error":{"message":"already exists"}}"#),
            ("GET", "/validate422") => response("422 Unprocessable Entity", r#"{"error":{"message":"field required"}}"#),
            ("GET", "/server500") => response("500 Internal Server Error", r#"{"error":{"message":"boom"}}"#),
            ("GET", "/sections") => response(
                "200 OK",
                r#"{"data":[{"id":"s1","name":"Haberler"},{"id":"s2","name":"Blog"}]}"#,
            ),
            ("POST", "/publish") => response(
                "201 Created",
                r#"{"data":{"id":"p-77","url":"https://site.test/yazi/77"}}"#,
            ),
            ("POST", "/publish-fail") => response(
                "422 Unprocessable Entity",
                r#"{"error":{"message":"baslik alani zorunlu"}}"#,
            ),
            ("GET", "/timeout") => {
                std::thread::sleep(std::time::Duration::from_millis(2000));
                response("200 OK", r#"{"ok":true}"#)
            }
            _ => response("404 Not Found", r#"{}"#),
        }
    }

    fn cfg_for(base: &str, endpoint: &str) -> SiteConfig {
        let mut cfg = SiteConfig::default();
        cfg.api_base = base.to_string();
        cfg.test_endpoint = endpoint.to_string();
        cfg
    }

    #[test]
    fn json_path_extracts_nested_and_indexed() {
        let v: Value = serde_json::from_str(r#"{"data":{"items":[{"id":"x1"}]},"n":42}"#).unwrap();
        assert_eq!(json_path_string(&v, "data.items[0].id").unwrap(), "x1");
        assert_eq!(json_path_string(&v, "n").unwrap(), "42");
        assert!(json_path_string(&v, "yok.yok").is_none());
    }

    #[test]
    fn template_fills_and_escapes_values() {
        let t = r#"{"title":"{{baslik}}","content":"{{icerik}}"}"#;
        let v = fill_template(t, &[("{{baslik}}", "Deneme \"X\"".into()), ("{{icerik}}", "Satir\nyeni".into())]).unwrap();
        assert_eq!(v["title"].as_str().unwrap(), "Deneme \"X\"");
        assert_eq!(v["content"].as_str().unwrap(), "Satir\nYeni".replace("Yeni", "yeni"));
    }

    #[test]
    fn connect_test_ok() {
        let srv = MockServer::start();
        let cfg = cfg_for(&srv.base(), "/ok");
        let out = run_test(&cfg, "gizli", Duration::from_secs(5));
        assert!(out.ok, "{}", out.turkce);
        assert_eq!(out.http_status, Some(200));
        assert!(out.turkce.contains("Baglanti basarili"));
    }

    #[test]
    fn connect_test_401() {
        let srv = MockServer::start();
        let cfg = cfg_for(&srv.base(), "/auth401");
        let out = run_test(&cfg, "yanlis-anahtar", Duration::from_secs(5));
        assert!(!out.ok);
        assert_eq!(out.code, "http_401");
        assert!(out.turkce.contains("Kimlik dogrulama basarisiz"));
    }

    #[test]
    fn connect_test_500() {
        let srv = MockServer::start();
        let cfg = cfg_for(&srv.base(), "/server500");
        let out = run_test(&cfg, "gizli", Duration::from_secs(5));
        assert!(!out.ok);
        assert_eq!(out.code, "http_5xx");
        assert!(out.turkce.contains("Sunucu hatasi"));
    }

    #[test]
    fn connect_test_timeout() {
        let srv = MockServer::start();
        let cfg = cfg_for(&srv.base(), "/timeout");
        let out = run_test(&cfg, "gizli", Duration::from_millis(400));
        assert!(!out.ok);
        assert_eq!(out.code, "network_timeout");
        assert!(out.turkce.contains("zamani"));
    }

    #[test]
    fn publish_success_returns_id_and_url() {
        let srv = MockServer::start();
        let mut cfg = cfg_for(&srv.base(), "/ok");
        cfg.publish_endpoint = "/publish".to_string();
        let input = SitePublishInput {
            title: "Baslik".to_string(),
            content: "Icerik".to_string(),
            section_id: "s1".to_string(),
            section_name: "Haberler".to_string(),
            media_path: String::new(),
        };
        let out = run_publish(&cfg, "gizli", &input, Duration::from_secs(5));
        assert!(out.ok, "{}", out.turkce);
        assert_eq!(out.content_id, "p-77");
        assert_eq!(out.content_url, "https://site.test/yazi/77");
        assert!(out.turkce.contains("Icerik ID: p-77"));
    }

    #[test]
    fn publish_failure_422_turkce_message() {
        let srv = MockServer::start();
        let mut cfg = cfg_for(&srv.base(), "/ok");
        cfg.publish_endpoint = "/publish-fail".to_string();
        let input = SitePublishInput {
            title: String::new(),
            content: "x".to_string(),
            section_id: "".to_string(),
            section_name: "".to_string(),
            media_path: String::new(),
        };
        let out = run_publish(&cfg, "gizli", &input, Duration::from_secs(5));
        assert!(!out.ok);
        assert_eq!(out.code, "http_422");
        assert!(out.turkce.contains("Icerik reddedildi"));
        assert!(out.turkce.contains("baslik alani zorunlu"));
    }

    #[test]
    fn sections_parsed_from_api() {
        let srv = MockServer::start();
        let mut cfg = cfg_for(&srv.base(), "/ok");
        cfg.sections_endpoint = "/sections".to_string();
        let out = run_sections(&cfg, "gizli", Duration::from_secs(5));
        assert!(out.ok);
        assert_eq!(out.sections.len(), 2);
        assert_eq!(out.sections[0].id, "s1");
        assert_eq!(out.sections[0].name, "Haberler");
    }

    #[test]
    fn status_mapping_covers_all_codes() {
        for (code, expected) in [
            (401, "Kimlik dogrulama"),
            (403, "Yetki reddedildi"),
            (404, "bulunamadi"),
            (409, "Cakisma"),
            (422, "reddedildi"),
            (500, "Sunucu hatasi"),
            (502, "Sunucu hatasi"),
        ] {
            let (_, msg) = status_turkce(code, "");
            assert!(msg.contains(expected), "{} icin: {}", code, msg);
        }
    }

    #[test]
    fn config_roundtrip_persists_only_nonsecret() {
        let dir = std::env::temp_dir().join(format!("es_site_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut cfg = SiteConfig::default();
        cfg.api_base = "https://api.ornek.test".to_string();
        cfg.publish_endpoint = "/posts".to_string();
        save_config(&dir, &cfg).unwrap();
        let loaded = load_config(&dir).unwrap();
        assert_eq!(loaded.api_base, "https://api.ornek.test");
        assert_eq!(loaded.publish_endpoint, "/posts");
        // Gizli bilgi yapÄ±landÄ±rma dosyasÄ±na yazÄ±lamaz: SiteConfig'te secret alanÄ± yoktur.
        let raw = fs::read_to_string(config_file(&dir)).unwrap();
        assert!(!raw.contains("gizli"));
        let _ = fs::remove_dir_all(&dir);
    }
}
