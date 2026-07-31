//! Sosyal medya hesap bağlantı altyapısı.
//!
//! Bu modül şunları sağlar:
//! - Ortak bağlantı modelleri (`models`)
//! - Platform kataloğu (`registry`)
//! - Gizli olmayan bağlantı metadatasının yerel kalıcı deposu (`metadata_store`)
//! - Windows Credential Manager tabanlı güvenli token deposu (`credential_store`)
//! - Ortak Tauri komutları (`commands`)
//!
//! Modül sınırı, ileride farklı işletim sistemi veya farklı platform
//! doğrulama akışlarının eklenmesine engel olmayacak biçimde ayrılmıştır.
//! Gizli bilgiler (tokenlar) hiçbir koşulda bu modülün dışına sızdırılmaz.

pub mod commands;
pub mod credential_store;
pub mod metadata_store;
pub mod models;
pub mod registry;
