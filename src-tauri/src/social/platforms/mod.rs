//! Platforma özel entegrasyonlar.
//!
//! Ortak altyapı (`models`, `registry`, `credential_store`, `metadata_store`)
//! platformdan bağımsızdır. Platforma özgü OAuth, token yenileme, API çağrıları
//! ve yükleme kuralları platform alt modüllerinde tutulur; ortak `commands.rs`
//! içine dağıtılmaz.
//!
//! - YouTube ayrı bir platform motorudur (`youtube`).
//! - Facebook ve Instagram, ortak Meta çekirdeğini (`meta`) paylaşır; iki ayrı
//!   OAuth motoru oluşturulmaz. `meta` tek API sürümünü, OAuth akışını, token
//!   işlemlerini ve Sayfa / Instagram hesabı keşfini merkezileştirir.
//!   Facebook (`facebook`) ve Instagram (`instagram`) yalnız kendi içerik ve
//!   yayın kurallarını içerir.

pub mod meta;
pub mod youtube;

pub mod facebook;
pub mod instagram;
pub mod linkedin;
pub mod pinterest;
pub mod tiktok;
pub mod x;
