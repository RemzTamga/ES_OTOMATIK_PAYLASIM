//! Güvenli token deposu.
//!
//! Windows'ta Windows Credential Manager üzerinden çalışır.
//! Bu modül, ham tokenı yalnız güvenli depoda saklar; hiçbir koşulda
//! tokenı düz metin, JSON, ayar dosyası veya JavaScript'e aktarmaz.
//!
//! Anahtar üretimi tek bir merkezi yardımcı fonksiyonla (`build_credential_service`)
//! yapılır; farklı dosyalarda manuel anahtar birleştirme yapılmaz.
//!
//! keyring kütüphanesi Windows'ta Credential Manager'ı kullanır.
//! Service olarak `app_id::platform_id::connection_id` birleşik anahtarı,
//! user olarak token türü kullanılır. Böylece dört bileşen (uygulama kimliği,
//! platform, bağlantı, token türü) keyring'in service/user yapısına temiz biçimde
//! yansıtılır ve modül sınırı ileride diğer işletim sistemi uygulamalarının
//! eklenmesine engel olmaz.

use super::models::{SocialError, TokenType};

/// Uygulama kimliği. Mevcut Tauri bundle identifier'ı (`com.es.ops`) kullanılır.
/// Yeni uygulama kimliği üretilmez.
pub const APP_ID: &str = "com.es.ops";

/// Credential Manager kaydı için birleşik anahtar üretimi.
/// Yalnız bu fonksiyon anahtar formatından sorumludur.
///
/// Bileşenler gerektiğinde normalize edilir (trim) ve `::` ayracıyla birleştirilir.
/// Ayraç karakteri platform_id ve connection_id içinde bulunmamalıdır.
fn build_credential_service(platform_id: &str, connection_id: &str) -> String {
    let platform = platform_id.trim();
    let conn = connection_id.trim();
    // Normalize: connection_id içerisinde doğrudan `::` bulunmasını engelle
    let conn = conn.replace("::", "_");
    format!("{}::{}::{}", APP_ID, platform, conn)
}

/// keyring v1 girişini oluşturur.
fn entry(platform_id: &str, connection_id: &str, token_type: TokenType) -> Result<keyring::v1::Entry, SocialError> {
    let service = build_credential_service(platform_id, connection_id);
    keyring::v1::Entry::new(&service, token_type.as_str())
        .map_err(|_| SocialError::CredentialStoreError)
}

/// Erişim tokenını güvenli depoya yazar.
pub fn store_token(
    platform_id: &str,
    connection_id: &str,
    token_type: TokenType,
    secret: &str,
) -> Result<(), SocialError> {
    if secret.is_empty() {
        return Err(SocialError::OperationFailed);
    }
    let entry = entry(platform_id, connection_id, token_type)?;
    entry
        .set_password(secret)
        .map_err(|_| SocialError::CredentialStoreError)
}

/// Güvenli depodan tokenı okur. İç araçtır; doğrudan Tauri komutuna açılmaz.
///
/// Kayıt bulunamazsa `Ok(None)` döner. Bu da "token mevcut değil" anlamına gelir.
pub fn get_token(
    platform_id: &str,
    connection_id: &str,
    token_type: TokenType,
) -> Result<Option<String>, SocialError> {
    let entry = entry(platform_id, connection_id, token_type)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::v1::Error::NoEntry) => Ok(None),
        Err(_) => Err(SocialError::CredentialStoreError),
    }
}

/// Güvenli depodan tokenı siler.
///
/// Kayıt bulunamazsa başarı kabul edilir (idempotent silme).
pub fn delete_token(
    platform_id: &str,
    connection_id: &str,
    token_type: TokenType,
) -> Result<(), SocialError> {
    let entry = entry(platform_id, connection_id, token_type)?;
    match entry.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::v1::Error::NoEntry) => Ok(()),
        Err(_) => Err(SocialError::CredentialStoreError),
    }
}

/// Belirtilen token türünün depoda var olup olmadığını denetler.
///
/// Yalnız access_token varlığı amacımızdır; ham token döndürülmez.
pub fn token_exists(
    platform_id: &str,
    connection_id: &str,
    token_type: TokenType,
) -> Result<bool, SocialError> {
    let entry = entry(platform_id, connection_id, token_type)?;
    match entry.get_password() {
        Ok(_) => Ok(true),
        Err(keyring::v1::Error::NoEntry) => Ok(false),
        Err(_) => Err(SocialError::CredentialStoreError),
    }
}

/// Yalnız bu bağlantıya ait bütün desteklenen token türlerini siler.
/// Bulunamayan token silme işlemi başarıyla tamamlanmış sayılır.
pub fn delete_all_tokens(
    platform_id: &str,
    connection_id: &str,
) -> Result<(), SocialError> {
    for token_type in TokenType::ALL {
        delete_token(platform_id, connection_id, token_type)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_service_key_is_stable_and_namespaced() {
        let a = build_credential_service("instagram", "conn-1");
        let b = build_credential_service("instagram", "conn-1");
        let c = build_credential_service("x", "conn-1");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("com.es.ops::"));
        assert!(a.contains("::instagram::"));
    }

    #[test]
    fn credential_service_key_removes_double_separator() {
        // Bileşen içinde ayraç kullanımı normalize edilir
        let k = build_credential_service("youtube", "a::b");
        assert!(!k.contains("::a::b"));
        assert!(k.contains("a_b"));
    }

    #[test]
    fn token_type_strings_are_controlled() {
        assert_eq!(TokenType::AccessToken.as_str(), "access_token");
        assert_eq!(TokenType::RefreshToken.as_str(), "refresh_token");
    }
}
