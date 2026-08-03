use serde::{Deserialize, Serialize};

/// Platform destek durumu. Kontrollü değerlerden oluşur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupportStatus {
    /// Platform tam entegre ve kullanılabilir.
    Supported,
    /// Entegrasyon bekliyor.
    Planned,
    /// Resmî kısıtlar nedeniyle bekliyor.
    Restricted,
    /// Resmî doğrulama bekliyor.
    VerificationPending,
    /// Mevcut sunucusuz mimaride desteklenmiyor.
    Unsupported,
}

/// Hesap bağlantı durumu. Kontrollü değerlerden oluşur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionStatus {
    Disconnected,
    Connected,
    TokenExpired,
    Error,
}

/// Bir sosyal medya platformunun statik teknik tanımı.
/// Bu yapı gizli bilgi veya token içermez.
#[derive(Debug, Clone, Serialize)]
pub struct PlatformDefinition {
    pub platform_id: &'static str,
    pub display_name: &'static str,
    pub support_status: SupportStatus,
}

/// Kalıcı deposuna yazılan (token içermeyen) gizli olmayan bağlantı kaydı.
/// token_exists burada yer almaz; her sorguda güvenli token deposundan hesaplanır.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionRecord {
    pub connection_id: String,
    pub platform_id: String,
    #[serde(default)]
    pub external_account_id: String,
    #[serde(default)]
    pub account_display_name: String,
    pub connection_status: ConnectionStatus,
    #[serde(default)]
    pub last_error_code: String,
    #[serde(default)]
    pub last_operation_at: String,
}

/// JavaScript'e döndürülen genişletilmiş bağlantı görünümü.
/// token_exists, kalıcı metadatadan değil güvenli token deposundan
/// her sorguda hesaplanan türetilmiş bilgidir.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SocialAccountConnection {
    pub connection_id: String,
    pub platform_id: String,
    pub external_account_id: String,
    pub account_display_name: String,
    pub connection_status: ConnectionStatus,
    pub token_exists: bool,
    pub last_error_code: String,
    pub last_operation_at: String,
}

impl ConnectionRecord {
    /// Kalıcı kaydı JavaScript görünümüne dönüştürür ve token varlığını ekler.
    pub fn to_public(&self, token_exists: bool) -> SocialAccountConnection {
        SocialAccountConnection {
            connection_id: self.connection_id.clone(),
            platform_id: self.platform_id.clone(),
            external_account_id: self.external_account_id.clone(),
            account_display_name: self.account_display_name.clone(),
            connection_status: self.connection_status,
            token_exists,
            last_error_code: self.last_error_code.clone(),
            last_operation_at: self.last_operation_at.clone(),
        }
    }
}

/// Güvenli token deposunda saklanabilecek token türleri.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    AccessToken,
    RefreshToken,
}

impl TokenType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenType::AccessToken => "access_token",
            TokenType::RefreshToken => "refresh_token",
        }
    }

    pub const ALL: [TokenType; 2] = [TokenType::AccessToken, TokenType::RefreshToken];
}

/// Rust tarafından makine tarafından işlenebilir kısa teknik hata kodları.
/// Bu yapı yalnız kısa kod taşır; token veya gizli bilgi içermez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SocialError {
    InvalidPlatform,
    InvalidConnection,
    CredentialNotFound,
    CredentialStoreError,
    ConnectionStoreError,
    UnsupportedPlatform,
    OperationFailed,
    YoutubeNotConfigured,
    OauthCancelled,
    OauthTimeout,
    OauthStateMismatch,
    OauthExchangeFailed,
    ChannelLookupFailed,
    TokenExpired,
    TokenRefreshFailed,
    FileNotFound,
    InvalidVideoFile,
    UnsupportedContentType,
    UnsupportedPostType,
    UploadSessionFailed,
    UploadFailed,
    ApiError,
    // Meta / Facebook / Instagram kontrollü hatalar.
    MetaNotConfigured,
    AppSecretRequired,
    PermissionDenied,
    AppReviewRequired,
    NoManagedPage,
    PageNotFound,
    InstagramAccountNotFound,
    InstagramProfessionalAccountRequired,
    TokenMissing,
    ReauthorizationRequired,
    InvalidMediaFile,
    MediaUrlUnavailable,
    MediaContainerFailed,
    MediaProcessingTimeout,
    PublishFailed,
    TiktokNotConfigured,
    XNotConfigured,
}

impl std::fmt::Display for SocialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            SocialError::InvalidPlatform => "invalid_platform",
            SocialError::InvalidConnection => "invalid_connection",
            SocialError::CredentialNotFound => "credential_not_found",
            SocialError::CredentialStoreError => "credential_store_error",
            SocialError::ConnectionStoreError => "connection_store_error",
            SocialError::UnsupportedPlatform => "unsupported_platform",
            SocialError::OperationFailed => "operation_failed",
            SocialError::YoutubeNotConfigured => "youtube_not_configured",
            SocialError::OauthCancelled => "oauth_cancelled",
            SocialError::OauthTimeout => "oauth_timeout",
            SocialError::OauthStateMismatch => "oauth_state_mismatch",
            SocialError::OauthExchangeFailed => "oauth_exchange_failed",
            SocialError::ChannelLookupFailed => "channel_lookup_failed",
            SocialError::TokenExpired => "token_expired",
            SocialError::TokenRefreshFailed => "token_refresh_failed",
            SocialError::FileNotFound => "file_not_found",
            SocialError::InvalidVideoFile => "invalid_video_file",
            SocialError::UnsupportedContentType => "unsupported_content_type",
            SocialError::UnsupportedPostType => "unsupported_post_type",
            SocialError::UploadSessionFailed => "upload_session_failed",
            SocialError::UploadFailed => "upload_failed",
            SocialError::ApiError => "api_error",
            SocialError::MetaNotConfigured => "meta_not_configured",
            SocialError::AppSecretRequired => "app_secret_required",
            SocialError::PermissionDenied => "permission_denied",
            SocialError::AppReviewRequired => "app_review_required",
            SocialError::NoManagedPage => "no_managed_page",
            SocialError::PageNotFound => "page_not_found",
            SocialError::InstagramAccountNotFound => "instagram_account_not_found",
            SocialError::InstagramProfessionalAccountRequired => "instagram_professional_account_required",
            SocialError::TokenMissing => "token_missing",
            SocialError::ReauthorizationRequired => "reauthorization_required",
            SocialError::InvalidMediaFile => "invalid_media_file",
            SocialError::MediaUrlUnavailable => "media_url_unavailable",
            SocialError::MediaContainerFailed => "media_container_failed",
            SocialError::MediaProcessingTimeout => "media_processing_timeout",
            SocialError::PublishFailed => "publish_failed",
            SocialError::TiktokNotConfigured => "tiktok_not_configured",
            SocialError::XNotConfigured => "x_not_configured",
        };
        f.write_str(msg)
    }
}

impl std::error::Error for SocialError {}

impl From<std::io::Error> for SocialError {
    fn from(_: std::io::Error) -> Self {
        SocialError::ConnectionStoreError
    }
}

