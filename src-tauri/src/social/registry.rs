use super::models::{PlatformDefinition, SupportStatus};

/// Statik platform kataloğu. Bu, platformların teknik destek durumlarının
/// tek ve yetkili kaynağıdır. JavaScript tarafı ikon/sıralama gibi görsel
/// bilgileri tutar; teknik support_status buradan gelir.
pub fn platform_catalog() -> Vec<PlatformDefinition> {
    vec![
        PlatformDefinition {
            platform_id: "youtube",
            display_name: "YouTube",
            support_status: SupportStatus::Supported,
        },
        PlatformDefinition {
            platform_id: "x",
            display_name: "X",
            support_status: SupportStatus::Supported,
        },
        PlatformDefinition {
            platform_id: "facebook",
            display_name: "Facebook",
            support_status: SupportStatus::Restricted,
        },
        PlatformDefinition {
            platform_id: "instagram",
            display_name: "Instagram",
            support_status: SupportStatus::Restricted,
        },
        PlatformDefinition {
            platform_id: "linkedin",
            display_name: "LinkedIn",
            support_status: SupportStatus::Supported,
        },
        PlatformDefinition {
            platform_id: "tiktok",
            display_name: "TikTok",
            support_status: SupportStatus::Supported,
        },
        PlatformDefinition {
            platform_id: "pinterest",
            display_name: "Pinterest",
            support_status: SupportStatus::Supported,
        },
        PlatformDefinition {
            platform_id: "threads",
            display_name: "Threads",
            support_status: SupportStatus::Unsupported,
        },
    ]
}

/// Platform id'si katalogda tanımlı mı diye denetler.
/// Bilinmeyen bir platform id'si geçersiz kabul edilir.
pub fn platform_exists(platform_id: &str) -> bool {
    platform_catalog()
        .iter()
        .any(|p| p.platform_id == platform_id)
}
