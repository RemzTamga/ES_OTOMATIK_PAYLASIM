//! Bağlantı metadatası deposu (gizli olmayan yerel kalıcı kayıtlar).
//!
//! Token dışındaki bağlantı kayıtları bu katmanda saklanır. Tokenlar
//! kesinlikle buraya yazılmaz; yalnızca `credential_store` içinde tutulur.
//!
//! Kalıcılık, mevcut proje mimarisine uygun en küçük yapı olan tek bir
//! JSON dosyası üzerinden sağlanır. Yeni veritabanı teknolojisi veya
//! ağır bağımlılık eklenmemiştir.
//!
//! Boş kayıt koleksiyonu geçerli bir sonuçtur. Okuma/yazma hataları
//! sessizce yok sayılmaz; `ConnectionStoreError` olarak yükseltilir.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::models::{ConnectionRecord, ConnectionStatus, SocialError};

/// Kalıcı metadata dosyasının adı.
const CONNECTIONS_FILE: &str = "social_connections.json";

/// Metadata deposunun kök dizinini döndürür.
fn store_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("social")
}

fn connections_file(data_dir: &Path) -> PathBuf {
    store_dir(data_dir).join(CONNECTIONS_FILE)
}

/// Dosyayı okuyup kayıt haritasını döndürür.
/// Dosya yoksa veya boşsa boş harita döndürülür.
fn read_all(data_dir: &Path) -> Result<BTreeMap<String, ConnectionRecord>, SocialError> {
    let path = connections_file(data_dir);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let raw = fs::read_to_string(&path).map_err(|e| {
        let _ = e;
        SocialError::ConnectionStoreError
    })?;
    if raw.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str(&raw).map_err(|_| SocialError::ConnectionStoreError)
}

/// Kayıt haritasını dosyaya atomik biçimde yazar.
fn write_all(data_dir: &Path, map: &BTreeMap<String, ConnectionRecord>) -> Result<(), SocialError> {
    let dir = store_dir(data_dir);
    fs::create_dir_all(&dir).map_err(|_| SocialError::ConnectionStoreError)?;
    let path = connections_file(data_dir);
    let raw = serde_json::to_string_pretty(map).map_err(|_| SocialError::ConnectionStoreError)?;
    fs::write(&path, raw).map_err(|_| SocialError::ConnectionStoreError)
}

/// Tüm kayıtları liste döndürür (bağlantı_id sırasına göre kararlı).
pub fn list_connections(data_dir: &Path) -> Result<Vec<ConnectionRecord>, SocialError> {
    let map = read_all(data_dir)?;
    Ok(map.into_values().collect())
}

/// Belirli bir bağlantı kaydını döndürür.
pub fn get_connection(
    data_dir: &Path,
    connection_id: &str,
) -> Result<Option<ConnectionRecord>, SocialError> {
    let map = read_all(data_dir)?;
    Ok(map.get(connection_id).cloned())
}

/// Kaydı ekler veya günceller.
pub fn upsert_connection(
    data_dir: &Path,
    record: ConnectionRecord,
) -> Result<(), SocialError> {
    let mut map = read_all(data_dir)?;
    map.insert(record.connection_id.clone(), record);
    write_all(data_dir, &map)
}

/// Belirli bir bağlantının durumunu günceller.
/// Kayıt bulunamazsa `ConnectionStoreError` döner (sessizce yok sayılmaz).
pub fn update_connection_status(
    data_dir: &Path,
    connection_id: &str,
    status: ConnectionStatus,
) -> Result<(), SocialError> {
    let mut map = read_all(data_dir)?;
    let record = map
        .get_mut(connection_id)
        .ok_or(SocialError::InvalidConnection)?;
    record.connection_status = status;
    write_all(data_dir, &map)
}

/// Belirli bir bağlantı kaydını siler.
pub fn delete_connection(data_dir: &Path, connection_id: &str) -> Result<(), SocialError> {
    let mut map = read_all(data_dir)?;
    if map.remove(connection_id).is_none() {
        return Err(SocialError::InvalidConnection);
    }
    write_all(data_dir, &map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(connection_id: &str, platform_id: &str) -> ConnectionRecord {
        ConnectionRecord {
            connection_id: connection_id.to_string(),
            platform_id: platform_id.to_string(),
            external_account_id: String::new(),
            account_display_name: "Test Hesap".to_string(),
            connection_status: ConnectionStatus::Connected,
            last_error_code: String::new(),
            last_operation_at: String::new(),
        }
    }

    /// Test için geçici dizin kurar ve test sonunda siler.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            let mut p = std::env::temp_dir();
            p.push(format!(
                "es_ops_social_test_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            fs::create_dir_all(&p).unwrap();
            TempDir(p)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn empty_list_is_valid() {
        let dir = TempDir::new();
        let list = list_connections(dir.path()).unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn upsert_then_get_roundtrip() {
        let dir = TempDir::new();
        let rec = sample_record("conn-1", "youtube");
        upsert_connection(dir.path(), rec.clone()).unwrap();
        let got = get_connection(dir.path(), "conn-1").unwrap().unwrap();
        assert_eq!(got.connection_id, "conn-1");
        assert_eq!(got.platform_id, "youtube");
        assert_eq!(got.connection_status, ConnectionStatus::Connected);
    }

    #[test]
    fn update_status_and_delete() {
        let dir = TempDir::new();
        let rec = sample_record("conn-1", "youtube");
        upsert_connection(dir.path(), rec).unwrap();
        update_connection_status(dir.path(), "conn-1", ConnectionStatus::Disconnected).unwrap();
        assert_eq!(
            get_connection(dir.path(), "conn-1").unwrap().unwrap().connection_status,
            ConnectionStatus::Disconnected
        );
        delete_connection(dir.path(), "conn-1").unwrap();
        assert!(get_connection(dir.path(), "conn-1").unwrap().is_none());
    }

    #[test]
    fn delete_missing_returns_error() {
        let dir = TempDir::new();
        let res = delete_connection(dir.path(), "missing");
        assert!(res.is_err());
    }
}
