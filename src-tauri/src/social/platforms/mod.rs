//! Platforma özel entegrasyonlar.
//!
//! Ortak altyapı (`models`, `registry`, `credential_store`, `metadata_store`)
//! platformdan bağımsızdır. Platforma özgü OAuth, token yenileme, API çağrıları
//! ve yükleme kuralları platform alt modüllerinde tutulur; ortak `commands.rs`
//! içine dağıtılmaz.
//!
//! Şu an yalnız YouTube entegrasyonu bulunur; yeni platformlar bu dizine eklenir.

pub mod youtube;
