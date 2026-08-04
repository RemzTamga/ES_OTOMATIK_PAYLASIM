//! ES Merkez Lisanslama uyumlu lisans doğrulama modülü.
//!
//! ES OPS lisans doğrulaması tamamen bu modülde (Rust/Tauri) yapılır;
//! JavaScript yalnız sonucu görüntüler. ES Merkez'in `license_core.py`,
//! `key_manager.py` ve `machine_id.py` algoritmaları birebir uygulanır:
//!
//! - **Canonical JSON:** `signature` alanı hariç, anahtarlar alfabetik,
//!   ayraçlar `,` ve `:`, UTF-8 (non-ASCII kaçış yok) — ES Merkez'in
//!   `json.dumps(ensure_ascii=False, sort_keys=True, separators=(",", ":"))`
//!   çıktısıyla aynıdır.
//! - **İmza:** RSA-2048 / RSA-PSS / SHA-256, tuz uzunluğu 222 bayt
//!   (ES Merkez `PSS.MAX_LENGTH` kullanır: emLen(256) - hLen(32) - 2).
//!   İmza Base64 URL-safe (padding'li) kodlanmıştır.
//! - **Makine kodu:** ES Merkez `machine_id.py` ile aynı: CPU ProcessorId,
//!   disk SerialNumber ve `hostname-UuidCreateSequential` parçalarının
//!   SHA-256 özetinden `MID-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX` üretilir.
//!
//! Güvenlik modeli: Bu projede yalnız ES Merkez'in `public_key.pem` dosyası
//! bulunur (yalnız doğrulama için). Özel anahtar bu projeye hiçbir şekilde
//! dahil edilmez, alınmaz ve saklanmaz.

use std::process::Command;
use std::sync::{Mutex, OnceLock};

use base64::Engine;
use rsa::pss::Pss;
use rsa::pkcs8::DecodePublicKey;
use rsa::RsaPublicKey;
use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// ES OPS ürün kodu. ES Merkez'de tam olarak `002` olarak kayıtlıdır;
/// bundan farklı ürün kodu taşıyan lisanslar reddedilir.
pub const PRODUCT_CODE: &str = "002";

/// ES Merkez tarafından sağlanan doğrulama anahtarı. Yalnız doğrulama içindir.
const PUBLIC_KEY_PEM: &str = include_str!("license/public_key.pem");

/// ES Merkez `PSS.MAX_LENGTH` ile imzalar: emLen - hLen - 2 = 256 - 32 - 2.
const PSS_SALT_LEN: usize = 222;

/// ES OPS veritabanından/satıcıdan gelen bir lisansın içermesi zorunlu alanlar.
/// ES Merkez `REQUIRED_LICENSE_FIELDS` listesiyle birebir aynıdır.
const REQUIRED_FIELDS: [&str; 12] = [
    "license_id",
    "customer_no",
    "customer_name",
    "product_code",
    "product_name",
    "machine_id",
    "license_type",
    "license_policy",
    "issued_at",
    "license_expire_date",
    "support_expire_date",
    "status",
];

/// ES Merkez makine kodu bileşeni: wmic CPU ProcessorId bulunamazsa kullanılır.
const CPU_UNKNOWN: &str = "CPU_UNKNOWN";

/// ES Merkez makine kodu bileşeni: wmic disk SerialNumber bulunamazsa kullanılır.
const DISK_UNKNOWN: &str = "DISK_UNKNOWN";

/// Güvenli saklama anahtarı (Windows Credential Manager üzerinden keyring v1).
/// Lisansın ham JSON içeriği buraya yazılır; JavaScript asla ham imza/veri görmez.
const LICENSE_STORE_SERVICE: &str = "com.es.ops::license";
const LICENSE_STORE_USER: &str = "license_v1";

// ---------------------------------------------------------------------------
// Lisans durum modelleri (Tauri komutlarına dönen JSON).
// ---------------------------------------------------------------------------

/// Lisansın görüntülenebilen alanları. Ham içerik JS'e verilmez;
/// yalnız bu beyaz listedeki alanlar döndürülür.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LicenseInfo {
    pub license_id: String,
    pub customer_no: String,
    pub customer_name: String,
    pub product_code: String,
    pub product_name: String,
    pub machine_id: String,
    pub license_type: String,
    pub license_policy: String,
    pub issued_at: String,
    pub license_expire_date: Option<String>,
    pub support_expire_date: Option<String>,
    pub status: String,
    pub notes: String,
    pub transfer_count: i64,
    pub max_transfer_count: i64,
}

/// Lisans doğrulama sonucu. `reason` kısa ve makinede okunabilir bir koddur:
/// `no_license | invalid_json | missing_fields | invalid_base64 |
/// invalid_signature | wrong_product | wrong_machine | not_active | expired`.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LicenseStatus {
    pub valid: bool,
    pub reason: Option<String>,
    pub license: Option<LicenseInfo>,
}

// ---------------------------------------------------------------------------
// Canonical JSON (ES Merkez `license_core.canonical_license_payload`).
// ---------------------------------------------------------------------------

/// İmza alanı hariç tüm alanları alfabetik anahtar sırasıyla, boşluksuz
/// ve UTF-8 olarak serileştirir. ES Merkez çıktısıyla birebir aynı baytları
/// üretmek zorunludur; aksi halde imza doğrulaması başarısız olur.
fn canonical_payload(data: &Value) -> Option<Vec<u8>> {
    let obj = data.as_object()?;

    let mut entries: Vec<(&str, &Value)> = obj
        .iter()
        .filter(|(key, _)| key.as_str() != "signature")
        .map(|(key, value)| (key.as_str(), value))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    let mut map = Map::new();
    for (key, value) in entries {
        map.insert(key.to_string(), value.clone());
    }
    serde_json::to_vec(&Value::Object(map)).ok()
}

// ---------------------------------------------------------------------------
// RSA-PSS imza doğrulama (ES Merkez `key_manager.verify_signature_bytes`).
// ---------------------------------------------------------------------------

/// ES Merkez public key'i ile PSS/SHA-256 imzasını doğrular.
pub fn verify_signature(payload: &[u8], signature: &[u8]) -> bool {
    let public_key = match RsaPublicKey::from_public_key_pem(PUBLIC_KEY_PEM) {
        Ok(key) => key,
        Err(_) => return false,
    };
    verify_signature_with(payload, signature, &public_key)
}

/// Belirli bir public key ile PSS/SHA-256 imzasını doğrular (testlerde kullanılır).
fn verify_signature_with(payload: &[u8], signature: &[u8], public_key: &RsaPublicKey) -> bool {
    let hashed = Sha256::digest(payload);
    public_key
        .verify(Pss::new_with_salt::<Sha256>(PSS_SALT_LEN), &hashed, signature)
        .is_ok()
}

// ---------------------------------------------------------------------------
// Makine kodu (ES Merkez `machine_id.generate_machine_id`).
// ---------------------------------------------------------------------------

/// ES Merkez `uuid.getnode()` ile birebir aynı donanım kimliği.
///
/// CPython 3.12 Windows'ta `UuidCreateSequential` (rpcrt4/ole32) çağırır ve
/// üretilen UUID'in son 6 baytını (Data4[2..8]) büyük-endian tamsayı olarak
/// döndürür. Dönüş kodu Python tarafında yok sayılır; burada da yok sayılır.
#[cfg(windows)]
fn windows_node_id() -> u64 {
    let mut guid = windows_sys::core::GUID {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
    unsafe {
        windows_sys::Win32::System::Rpc::UuidCreateSequential(&mut guid);
    }
    let bytes = guid.data4;
    u64::from_be_bytes([0, 0, bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]])
}

#[cfg(not(windows))]
fn windows_node_id() -> u64 {
    0
}

/// Dış komut çalıştırıp stdout'unu döndürür; komut başarısızsa boş dize.
fn run_command(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

/// `wmic <alias> get <field> /value` çıktısından ilk değeri okur.
/// ES Merkez `_read_wmic_value` ile aynı davranış.
fn read_wmic_value(alias: &str, field: &str) -> String {
    let upper = field.to_ascii_uppercase();
    let prefix = format!("{}=", upper);
    for line in run_command("wmic", &[alias, "get", field, "/value"]).lines() {
        let line = line.trim();
        if line.to_ascii_uppercase().starts_with(&prefix) {
            return line[prefix.len()..].trim().to_string();
        }
    }
    String::new()
}

/// ES Merkez `get_machine_components` ile aynı üç bileşeni üretir.
fn machine_components() -> (String, String, String) {
    let cpu = read_wmic_value("cpu", "ProcessorId");
    let disk = read_wmic_value("diskdrive", "SerialNumber");
    let hostname = std::env::var("COMPUTERNAME").unwrap_or_default();
    let cpu = if cpu.is_empty() { CPU_UNKNOWN.to_string() } else { cpu };
    let disk = if disk.is_empty() { DISK_UNKNOWN.to_string() } else { disk };
    let fallback = format!("{}-{}", hostname, windows_node_id());
    (cpu, disk, fallback)
}

/// ES Merkez `normalize_product_code` ile aynı: `[^A-Z0-9_]` -> `_`, üst,
/// kenarlardaki `_` atılır.
fn normalize_product_code(product_code: &str) -> String {
    let mut normalized = String::new();
    for ch in product_code.chars() {
        for upper in ch.to_uppercase() {
            if upper.is_ascii_alphanumeric() || upper == '_' {
                normalized.push(upper);
            } else {
                normalized.push('_');
            }
        }
    }
    normalized.trim_matches('_').to_string()
}

fn hex_upper(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// ES Merkez `generate_machine_id` ile aynı makine kodunu üretir.
pub fn compute_machine_id_from_parts(
    cpu_id: &str,
    disk_serial: &str,
    fallback: &str,
    product_code: &str,
) -> String {
    let normalized = normalize_product_code(product_code);
    let raw = format!("{}|{}|{}|{}", cpu_id, disk_serial, fallback, normalized);
    let digest = hex_upper(&Sha256::digest(raw.as_bytes()));
    let groups: Vec<String> = (0..5)
        .map(|index| digest[index * 5..index * 5 + 5].to_string())
        .collect();
    format!("MID-{}", groups.join("-"))
}

/// Bu bilgisayarın makine kodunu üretir (bellekte bir kez önbelleğe alınır).
fn current_machine_id() -> Result<String, String> {
    static CACHE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if let Some(id) = cache.lock().unwrap().as_ref() {
        return Ok(id.clone());
    }
    let (cpu, disk, fallback) = machine_components();
    let id = compute_machine_id_from_parts(&cpu, &disk, &fallback, PRODUCT_CODE);
    *cache.lock().unwrap() = Some(id.clone());
    Ok(id)
}

// ---------------------------------------------------------------------------
// Lisans doğrulama.
// ---------------------------------------------------------------------------

/// Süreli lisanslarda bitiş tarihi kontrolü. ES Merkez `date.today()` ile
/// karşılaştırır; bitiş bugünden önce ise lisans süresi dolmuştur.
/// Geçerli bir `YYYY-MM-DD` tarihi olmayan değer süreli lisans kabul edilir.
fn is_expired(expire_date: &str) -> bool {
    let today = chrono::Local::now().date_naive();
    match chrono::NaiveDate::parse_from_str(expire_date, "%Y-%m-%d") {
        Ok(expire) => expire < today,
        Err(_) => true,
    }
}

fn str_field(data: &Value, key: &str) -> String {
    data.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn int_field(data: &Value, key: &str) -> i64 {
    data.get(key).and_then(Value::as_i64).unwrap_or(0)
}

fn license_info(data: &Value) -> LicenseInfo {
    LicenseInfo {
        license_id: str_field(data, "license_id"),
        customer_no: str_field(data, "customer_no"),
        customer_name: str_field(data, "customer_name"),
        product_code: str_field(data, "product_code"),
        product_name: str_field(data, "product_name"),
        machine_id: str_field(data, "machine_id"),
        license_type: str_field(data, "license_type"),
        license_policy: str_field(data, "license_policy"),
        issued_at: str_field(data, "issued_at"),
        license_expire_date: data.get("license_expire_date").and_then(Value::as_str).map(str::to_string),
        support_expire_date: data.get("support_expire_date").and_then(Value::as_str).map(str::to_string),
        status: str_field(data, "status"),
        notes: str_field(data, "notes"),
        transfer_count: int_field(data, "transfer_count"),
        max_transfer_count: int_field(data, "max_transfer_count"),
    }
}

/// Tüm doğrulama kurallarını uygular. Hata durumunda neden kodu döner.
///
/// Sıra ES Merkez `verify_license` ve ES OPS gereksinimleriyle aynıdır:
/// zorunlu alanlar -> imza -> ürün kodu (`002`) -> makine kodu ->
/// durum (`ACTIVE`) -> bitiş tarihi.
fn verify_license_data_with_key(data: &Value, machine_id: &str, public_key: &RsaPublicKey) -> Result<LicenseInfo, String> {
    let obj = data.as_object().ok_or("invalid_json")?;
    for field in REQUIRED_FIELDS {
        if !obj.contains_key(field) {
            return Err("missing_fields".to_string());
        }
    }

    let signature_b64 = data
        .get("signature")
        .and_then(Value::as_str)
        .ok_or("missing_fields")?;
    if signature_b64.is_empty() {
        return Err("missing_fields".to_string());
    }
    let signature = base64::engine::general_purpose::URL_SAFE
        .decode(signature_b64)
        .map_err(|_| "invalid_base64".to_string())?;
    if signature.len() != 256 {
        return Err("invalid_signature".to_string());
    }

    let payload = canonical_payload(data).ok_or("invalid_json")?;
    if !verify_signature_with(&payload, &signature, public_key) {
        return Err("invalid_signature".to_string());
    }

    if data.get("product_code").and_then(Value::as_str) != Some(PRODUCT_CODE) {
        return Err("wrong_product".to_string());
    }
    if data.get("machine_id").and_then(Value::as_str) != Some(machine_id) {
        return Err("wrong_machine".to_string());
    }
    if data.get("status").and_then(Value::as_str) != Some("ACTIVE") {
        return Err("not_active".to_string());
    }
    if let Some(expire) = data.get("license_expire_date").and_then(Value::as_str) {
        if !expire.is_empty() && is_expired(expire) {
            return Err("expired".to_string());
        }
    }

    Ok(license_info(data))
}

/// Gömülü ES Merkez public key'i ile geçerli makine kimliğine göre doğrular.
fn verify_license_data(data: &Value, machine_id: &str) -> Result<LicenseInfo, String> {
    let public_key = RsaPublicKey::from_public_key_pem(PUBLIC_KEY_PEM)
        .map_err(|_| "invalid_key".to_string())?;
    verify_license_data_with_key(data, machine_id, &public_key)
}

// ---------------------------------------------------------------------------
// Güvenli depolama (Windows Credential Manager üzerinden keyring v1).
// ---------------------------------------------------------------------------

fn store_entry() -> Result<keyring::v1::Entry, String> {
    keyring::v1::Entry::new(LICENSE_STORE_SERVICE, LICENSE_STORE_USER)
        .map_err(|_| "store_unavailable".to_string())
}

fn store_license(content: &str) -> Result<(), String> {
    store_entry()?
        .set_password(content)
        .map_err(|_| "store_write_failed".to_string())
}

fn load_license() -> Result<Option<String>, String> {
    match store_entry()?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::v1::Error::NoEntry) => Ok(None),
        Err(_) => Err("store_read_failed".to_string()),
    }
}

fn clear_license() -> Result<(), String> {
    match store_entry()?.delete_credential() {
        Ok(()) | Err(keyring::v1::Error::NoEntry) => Ok(()),
        Err(_) => Err("store_delete_failed".to_string()),
    }
}

// ---------------------------------------------------------------------------
// Tauri komutları.
// ---------------------------------------------------------------------------

/// Lisans dosyasının içeriğini alır, tüm kurallarla doğrular ve geçerliyse
/// güvenli depoya yazar. Geçersizse depolama yapılmaz; neden kodu döner.
#[tauri::command]
pub async fn license_install(content: String) -> Result<LicenseStatus, String> {
    let machine_id = current_machine_id()?;
    let data: Value = serde_json::from_str(&content).map_err(|_| "invalid_json".to_string())?;
    match verify_license_data(&data, &machine_id) {
        Ok(info) => {
            store_license(&content)?;
            Ok(LicenseStatus { valid: true, reason: None, license: Some(info) })
        }
        Err(reason) => Ok(LicenseStatus { valid: false, reason: Some(reason), license: None }),
    }
}

/// Güvenli depoda saklı lisansı her çağrıda yeniden doğrular.
/// Geçersiz hale gelen lisans depodan silinir.
#[tauri::command]
pub async fn license_status() -> Result<LicenseStatus, String> {
    let machine_id = current_machine_id()?;
    let content = match load_license()? {
        Some(content) => content,
        None => {
            return Ok(LicenseStatus {
                valid: false,
                reason: Some("no_license".to_string()),
                license: None,
            })
        }
    };
    let data: Value = serde_json::from_str(&content).map_err(|_| "invalid_json".to_string())?;
    match verify_license_data(&data, &machine_id) {
        Ok(info) => Ok(LicenseStatus { valid: true, reason: None, license: Some(info) }),
        Err(reason) => {
            let _ = clear_license();
            Ok(LicenseStatus { valid: false, reason: Some(reason), license: None })
        }
    }
}

/// Bu bilgisayarın ES Merkez formatındaki makine kodunu döndürür.
#[tauri::command]
pub async fn license_machine_id() -> Result<String, String> {
    current_machine_id()
}

/// Güvenli depoda saklı lisansı siler. Kayıt yoksa başarı sayılır.
#[tauri::command]
pub async fn license_clear() -> Result<(), String> {
    clear_license()
}

// ---------------------------------------------------------------------------
// Testler.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    /// ES Merkez'in ürettiği gerçek örnek lisans dosyası
    /// (`ES_MERKEZ_LISANSLAMA_BELGELERI\LIC-20260701-000001.lic`).
    const SAMPLE_LICENSE: &str = r#"{
      "customer_name": "fatih EFE",
      "customer_no": "M000001",
      "issued_at": "2026-07-01",
      "license_expire_date": null,
      "license_id": "LIC-20260701-000001",
      "license_policy": "PERPETUAL",
      "license_type": "FULL",
      "machine_id": "MID-F15F7-D9812-929D4-C10AE-DDF0B",
      "max_transfer_count": 3,
      "notes": "",
      "product_code": "VIRÜS1",
      "product_name": "es_antivirüs",
      "signature": "SI8mKuTlwXF2xJInG2aWVwOeTeeWlw9MG2r37W15rcnGzZ29buQKFQWIw5QxsW5PlFdu7zBDKh7gsA9Nxmi4fFLQE0g5VxbWv2hh2pVcoJdASDXoT71e7utda57z4ewuZxCPksh2-Mjt_wxGldrJWLiy4_G3jRo4o54_HwZpBYzLD7rKDtpcV_HZHn1iSiMyadP19Le7c-CcZx-PjMHV5HD1opRC4aBUg_pzeunVuvdoh0o-VY8iaIw_52zfHlFZbcq5jRskrYpRDLE38oDoZeDZD225jnn43ROWW7LcO74oOVwXB4hWjYCd2OhdLzLffIjf1xf7nn50kFAkFdu7fQ==",
      "status": "ACTIVE",
      "support_expire_date": null,
      "transfer_count": 0
    }"#;

    /// ES Merkez `canonical_license_payload` çıktısı (py-3.12 ile birebir alındı).
    const SAMPLE_CANONICAL: &str = "{\"customer_name\":\"fatih EFE\",\"customer_no\":\"M000001\",\"issued_at\":\"2026-07-01\",\"license_expire_date\":null,\"license_id\":\"LIC-20260701-000001\",\"license_policy\":\"PERPETUAL\",\"license_type\":\"FULL\",\"machine_id\":\"MID-F15F7-D9812-929D4-C10AE-DDF0B\",\"max_transfer_count\":3,\"notes\":\"\",\"product_code\":\"VIRÜS1\",\"product_name\":\"es_antivirüs\",\"status\":\"ACTIVE\",\"support_expire_date\":null,\"transfer_count\":0}";

    fn test_keypair() -> (rsa::RsaPrivateKey, RsaPublicKey) {
        let mut rng = rand::thread_rng();
        let private_key = rsa::RsaPrivateKey::new(&mut rng, 2048).expect("keygen");
        let public_key = RsaPublicKey::from(&private_key);
        (private_key, public_key)
    }

    fn test_public_key_pem(public_key: &RsaPublicKey) -> String {
        use rsa::pkcs8::{EncodePublicKey, LineEnding};
        public_key.to_public_key_pem(LineEnding::LF).expect("pem")
    }

    fn sign_payload(private_key: &rsa::RsaPrivateKey, payload: &[u8]) -> Vec<u8> {
        let hashed = Sha256::digest(payload);
        private_key
            .sign_with_rng(
                &mut rand::thread_rng(),
                Pss::new_with_salt::<Sha256>(PSS_SALT_LEN),
                &hashed,
            )
            .expect("sign")
    }

    fn b64url(input: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE.encode(input)
    }

    fn make_signed_license(
        private_key: &rsa::RsaPrivateKey,
        product_code: &str,
        machine_id: &str,
        status: &str,
        expire: Option<&str>,
    ) -> Value {
        let mut license = json!({
            "license_id": "LIC-TEST-000001",
            "customer_no": "M000001",
            "customer_name": "Test Musteri",
            "product_code": product_code,
            "product_name": "ES Otomatik Paylasim Sistemi",
            "machine_id": machine_id,
            "license_type": "FULL",
            "license_policy": "PERPETUAL",
            "issued_at": "2026-01-01",
            "license_expire_date": null,
            "support_expire_date": null,
            "status": status,
            "notes": "",
            "transfer_count": 0,
            "max_transfer_count": 3
        });
        if let Some(date) = expire {
            license["license_expire_date"] = Value::String(date.to_string());
        }
        let payload = canonical_payload(&license).expect("canonical");
        license["signature"] = Value::String(b64url(&sign_payload(private_key, &payload)));
        license
    }

    fn verify_with(public_key_pem: &str, data: &Value, machine_id: &str) -> Result<LicenseInfo, String> {
        let public_key = RsaPublicKey::from_public_key_pem(public_key_pem).expect("parse pem");
        verify_license_data_with_key(data, machine_id, &public_key)
    }

    // -- Canonical JSON --

    #[test]
    fn canonical_payload_matches_es_merkez() {
        let data: Value = serde_json::from_str(SAMPLE_LICENSE).unwrap();
        let canonical = canonical_payload(&data).unwrap();
        assert_eq!(String::from_utf8(canonical).unwrap(), SAMPLE_CANONICAL);
    }

    // -- Gerçek ES Merkez imzası --

    #[test]
    fn real_es_merkez_sample_signature_is_accepted() {
        let data: Value = serde_json::from_str(SAMPLE_LICENSE).unwrap();
        let payload = canonical_payload(&data).unwrap();
        let signature =
            base64::engine::general_purpose::URL_SAFE.decode(data["signature"].as_str().unwrap()).unwrap();
        assert_eq!(signature.len(), 256);
        assert!(verify_signature(&payload, &signature));
    }

    #[test]
    fn tampered_signature_is_rejected() {
        let data: Value = serde_json::from_str(SAMPLE_LICENSE).unwrap();
        let payload = canonical_payload(&data).unwrap();
        let mut signature =
            base64::engine::general_purpose::URL_SAFE.decode(data["signature"].as_str().unwrap()).unwrap();
        signature[0] ^= 0x01;
        assert!(!verify_signature(&payload, &signature));
    }

    #[test]
    fn tampered_payload_is_rejected() {
        let data: Value = serde_json::from_str(SAMPLE_LICENSE).unwrap();
        let mut tampered = data.clone();
        tampered["customer_name"] = Value::String("degistirilmis".to_string());
        let payload = canonical_payload(&tampered).unwrap();
        let signature =
            base64::engine::general_purpose::URL_SAFE.decode(data["signature"].as_str().unwrap()).unwrap();
        assert!(!verify_signature(&payload, &signature));
    }

    // -- Tüm kurallar (test anahtarıyla) --

    #[test]
    fn valid_license_is_accepted() {
        let (private_key, public_key) = test_keypair();
        let pem = test_public_key_pem(&public_key);
        let machine_id = "MID-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE";
        let license = make_signed_license(&private_key, PRODUCT_CODE, machine_id, "ACTIVE", None);
        assert!(verify_with(&pem, &license, machine_id).is_ok());
    }

    #[test]
    fn wrong_product_code_is_rejected() {
        let (private_key, public_key) = test_keypair();
        let pem = test_public_key_pem(&public_key);
        let machine_id = "MID-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE";
        let license = make_signed_license(&private_key, "999", machine_id, "ACTIVE", None);
        assert_eq!(verify_with(&pem, &license, machine_id).unwrap_err(), "wrong_product");
    }

    #[test]
    fn wrong_machine_id_is_rejected() {
        let (private_key, public_key) = test_keypair();
        let pem = test_public_key_pem(&public_key);
        let license = make_signed_license(
            &private_key,
            PRODUCT_CODE,
            "MID-F1111-22222-33333-44444-55555",
            "ACTIVE",
            None,
        );
        assert_eq!(
            verify_with(&pem, &license, "MID-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE").unwrap_err(),
            "wrong_machine"
        );
    }

    #[test]
    fn non_active_license_is_rejected() {
        let (private_key, public_key) = test_keypair();
        let pem = test_public_key_pem(&public_key);
        let machine_id = "MID-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE";
        let license = make_signed_license(&private_key, PRODUCT_CODE, machine_id, "SUSPENDED", None);
        assert_eq!(verify_with(&pem, &license, machine_id).unwrap_err(), "not_active");
    }

    #[test]
    fn expired_license_is_rejected() {
        let (private_key, public_key) = test_keypair();
        let pem = test_public_key_pem(&public_key);
        let machine_id = "MID-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE";
        let license = make_signed_license(&private_key, PRODUCT_CODE, machine_id, "ACTIVE", Some("2020-01-01"));
        assert_eq!(verify_with(&pem, &license, machine_id).unwrap_err(), "expired");
    }

    #[test]
    fn license_expiring_today_is_still_valid() {
        let (private_key, public_key) = test_keypair();
        let pem = test_public_key_pem(&public_key);
        let machine_id = "MID-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE";
        let today = chrono::Local::now().date_naive().format("%Y-%m-%d").to_string();
        let license = make_signed_license(&private_key, PRODUCT_CODE, machine_id, "ACTIVE", Some(&today));
        assert!(verify_with(&pem, &license, machine_id).is_ok());
    }

    #[test]
    fn missing_required_field_is_rejected() {
        let (private_key, public_key) = test_keypair();
        let pem = test_public_key_pem(&public_key);
        let machine_id = "MID-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE";
        let mut license = make_signed_license(&private_key, PRODUCT_CODE, machine_id, "ACTIVE", None);
        license.as_object_mut().unwrap().remove("license_id");
        assert_eq!(verify_with(&pem, &license, machine_id).unwrap_err(), "missing_fields");
    }

    #[test]
    fn invalid_signature_base64_is_rejected() {
        let (private_key, public_key) = test_keypair();
        let pem = test_public_key_pem(&public_key);
        let machine_id = "MID-AAAAA-BBBBB-CCCCC-DDDDD-EEEEE";
        let mut license = make_signed_license(&private_key, PRODUCT_CODE, machine_id, "ACTIVE", None);
        license["signature"] = Value::String("!!!gecersiz!!!".to_string());
        assert_eq!(verify_with(&pem, &license, machine_id).unwrap_err(), "invalid_base64");
    }

    // -- Makine kodu --

    #[test]
    fn machine_id_is_deterministic_and_depends_on_product_code() {
        let a = compute_machine_id_from_parts("CPU1", "DISK1", "HOST-1", "002");
        let b = compute_machine_id_from_parts("CPU1", "DISK1", "HOST-1", "002");
        let c = compute_machine_id_from_parts("CPU1", "DISK1", "HOST-1", "003");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn machine_id_format() {
        let id = compute_machine_id_from_parts("CPU1", "DISK1", "HOST-1", "002");
        // "MID-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX": 4 + 25 + 4 = 33 karakter.
        assert_eq!(id.len(), 33);
        assert!(id.starts_with("MID-"));
        assert_eq!(id.matches('-').count(), 5);
    }

    #[test]
    fn normalize_product_code_same_as_es_merkez() {
        assert_eq!(normalize_product_code("Virüs1"), "VIR_S1");
        assert_eq!(normalize_product_code("---002---"), "002");
        assert_eq!(normalize_product_code("002"), "002");
    }

    #[test]
    fn machine_id_matches_es_merkez_on_this_pc() {
        // ES Merkez `generate_machine_id("002")` bu bilgisayarda bilindiği üzere:
        let (cpu, disk, fallback) = machine_components();
        let id = compute_machine_id_from_parts(&cpu, &disk, &fallback, PRODUCT_CODE);
        assert_eq!(id, "MID-98FAF-9CCC3-0582B-97FAA-8CBD0");
    }

    // -- Güvenli depolama --

    #[test]
    fn license_store_roundtrip() {
        let content = "{\"test\": \"gecici\"}";
        let store_result = store_license(content);
        // Depo ortamda erişilemezse (ör. CI'da) testi başarısız saymıyoruz;
        // Windows Credential Manager erişilebilirse gerçek yazma/okuma/silme doğrulanır.
        match store_result {
            Ok(()) => {
                assert_eq!(load_license().unwrap().as_deref(), Some(content));
                clear_license().unwrap();
                assert!(load_license().unwrap().is_none());
            }
            Err(_) => {}
        }
    }
}