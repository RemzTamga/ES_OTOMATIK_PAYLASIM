//! Ortak Tauri komutları.
//!
//! Yalnız ortak, platforma özel olmayan işlemleri içerir. Platforma özel
//! OAuth endpointleri, token yenileme kuralları veya paylaşım kuralları
//! bu dosyaya dağıtılmaz. Yeni bir platform eklemek için ortak modellerin
//! baştan yazılması gerekmez.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::credential_store;
use super::models::{
    ConnectionStatus, PlatformDefinition, SocialAccountConnection, SocialError, TokenType,
};
use super::platforms::youtube;
use super::{metadata_store, registry};

/// Metadata deposunun kök dizinini uygulama veri klasörü üzerinden hesaplar.
fn data_dir(app: &AppHandle) -> Result<PathBuf, SocialError> {
    app.path()
        .app_data_dir()
        .map_err(|_| SocialError::ConnectionStoreError)
}

/// Platform kataloğunu ve destek durumlarını döndürür.
/// Gizli bilgi veya token döndürmez.
#[tauri::command]
pub fn social_platform_catalog() -> Vec<PlatformDefinition> {
    registry::platform_catalog()
}

/// Kalıcı veri katmanındaki gerçek hesap bağlantılarını döndürür.
/// Her kayıt için token_exists değeri güvenli depodan hesaplanır.
/// Hesap bulunmuyorsa boş liste döner; sahte hesap üretilmez.
#[tauri::command]
pub fn social_account_connections(
    app: AppHandle,
) -> Result<Vec<SocialAccountConnection>, SocialError> {
    let dir = data_dir(&app)?;
    let records = metadata_store::list_connections(&dir)?;

    let mut result = Vec::with_capacity(records.len());
    for record in records {
        let token_exists =
            credential_store::token_exists(&record.platform_id, &record.connection_id, TokenType::AccessToken)
                .map_err(|_| SocialError::CredentialStoreError)?;
        result.push(record.to_public(token_exists));
    }
    Ok(result)
}

/// Belirtilen bağlantı kaydını bulur.
/// token_exists değeri güvenli depodan hesaplanır; ham token döndürülmez.
/// Kayıt bulunamazsa kontrollü hata sonucu döner.
#[tauri::command]
pub fn social_account_status(
    app: AppHandle,
    connection_id: String,
) -> Result<SocialAccountConnection, SocialError> {
    if connection_id.trim().is_empty() {
        return Err(SocialError::InvalidConnection);
    }
    let dir = data_dir(&app)?;
    let record =
        metadata_store::get_connection(&dir, &connection_id)?.ok_or(SocialError::InvalidConnection)?;

    let token_exists =
        credential_store::token_exists(&record.platform_id, &record.connection_id, TokenType::AccessToken)
            .map_err(|_| SocialError::CredentialStoreError)?;
    Ok(record.to_public(token_exists))
}

/// Bağlantı kesme işleminin kontrollü sonuç durumu.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DisconnectStatus {
    /// Bağlantı başarıyla kesildi.
    Disconnected,
    /// Hesap zaten bağlı değildi.
    NotConnected,
    /// Bağlantı kaydı bulunamadı.
    NotFound,
    /// İşlem başarısız oldu.
    Failed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectResult {
    pub status: DisconnectStatus,
}

/// Belirtilen connection_id'ye ait bağlantıyı keser.
///
/// Davranış kuralları:
/// - Bağlantı kaydı bulunmazsa sahte başarı döndürülmez (`not_found`).
/// - Hesap zaten bağlı değilse sahte bağlantı kesme başarısı döndürülmez (`not_connected`).
/// - Bağlantı kesilirken önce yalnız bu bağlantıya ait tüm token türleri silinir.
/// - Token silme veya metadata güncelleme başarısız olursa tam başarı döndürülmez (`failed`).
#[tauri::command]
pub fn social_disconnect_account(
    app: AppHandle,
    connection_id: String,
) -> Result<DisconnectResult, SocialError> {
    if connection_id.trim().is_empty() {
        // Boş id: kayıt yok anlamında kontrollü döndür
        return Ok(DisconnectResult {
            status: DisconnectStatus::NotFound,
        });
    }

    let dir = data_dir(&app)?;
    let record =
        match metadata_store::get_connection(&dir, &connection_id)? {
            Some(rec) => rec,
            None => {
                return Ok(DisconnectResult {
                    status: DisconnectStatus::NotFound,
                })
            }
        };

    // Hesap zaten bağlı değilse veya erişim tokenı mevcut değilse
    // işlem yapılmadan "not_connected" döndürülür (sahte başarı gösterilmez).
    let has_token = credential_store::token_exists(
        &record.platform_id,
        &connection_id,
        TokenType::AccessToken,
    )
    .map_err(|_| SocialError::CredentialStoreError)?;
    if record.connection_status == ConnectionStatus::Disconnected || !has_token {
        return Ok(DisconnectResult {
            status: DisconnectStatus::NotConnected,
        });
    }

    // Token silme hatası başarısızlık sayılır.
    if credential_store::delete_all_tokens(&record.platform_id, &connection_id).is_err() {
        return Ok(DisconnectResult {
            status: DisconnectStatus::Failed,
        });
    }

    // Metadata kaydını disconnected yap. Bu adım başarısız olursa tam başarı döndürme.
    if metadata_store::update_connection_status(&dir, &connection_id, ConnectionStatus::Disconnected)
        .is_err()
    {
        return Ok(DisconnectResult {
            status: DisconnectStatus::Failed,
        });
    }

    Ok(DisconnectResult {
        status: DisconnectStatus::Disconnected,
    })
}

/// YouTube OAuth bağlantı sonucu için serilerlenebilir sonuç görünümü.
/// İçeride yalnız kamuya açık bilgi barınır; ham token döndürülmez.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeConnectResult {
    pub connection: SocialAccountConnection,
}

/// YouTube'a gerçek OAuth akışıyla bağlanır.
///
/// - Google'ın masaüstü uygulama OAuth 2.0 akışını uygular (PKCE-S256, state).
/// - Tarayıcıyı resmî Tauri shell mekanizmasıyla açar.
/// - Sonuç gerçek bir `SocialAccountConnection` döndürür; hiçbir durumda
///   sahte bağlantı, sahte kanal adı veya yer tutucu üretilmez.
#[tauri::command]
pub fn youtube_connect(
    app: AppHandle,
) -> Result<YoutubeConnectResult, SocialError> {
    let connection = youtube::connect(&app)?;
    Ok(YoutubeConnectResult { connection })
}

/// Bir YouTube hesabına gerçek `videos.insert` (resumable) ile video yükler.
///
/// Zorunlu alanlar: bağlı bağlantı id, gerçek video dosya yolu, başlık,
/// gizlilik. Açıklama isteğe bağlıdır (boş olabilir). Gerçek video id döner.
///
/// Gizlilik kontrollü değerlerle gelir (`private`, `unlisted`, `public`);
/// JavaScript'ten serbest metinle belirlenmez.
#[tauri::command]
pub fn youtube_upload_video(
    app: AppHandle,
    connection_id: String,
    video_path: String,
    title: String,
    description: String,
    privacy: String,
) -> Result<String, SocialError> {
    let privacy_status = youtube::PrivacyStatus::parse(&privacy)
        .ok_or(SocialError::UnsupportedPostType)?;
    youtube::upload_video(&app, &connection_id, &video_path, &title, &description, privacy_status)
}
