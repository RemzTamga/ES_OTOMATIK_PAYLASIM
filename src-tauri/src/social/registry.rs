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

#[cfg(test)]
mod tests {
    use super::*;

    /// GitHub Actions workflow dosyasının içeriği (yalnızca test için paketlenir).
    /// Gizli bilgi içermez; yalnız secret reference adları taşır.
    const WORKFLOW: &str = include_str!("../../../.github/workflows/tauri-windows.yml");

    /// Overview: build ortamından Rust'a aktarılan client id env adları, ilgili
    /// platform kaynağındaki `option_env!` adlarıyla birebir aynı olmalıdır.
    /// (Kontrol maddesi: "Workflow'daki environment variable adı ile Rust
    /// tarafındaki isim birebir aynı mı?")
    #[test]
    fn workflow_client_id_env_matches_rust_option_env() {
        let youtube_src = include_str!("platforms/youtube.rs");
        let meta_src = include_str!("platforms/meta.rs");

        // YouTube: worklow'da ES_OPS_YOUTUBE_CLIENT_ID, Rust'ta option_env!.
        assert!(
            WORKFLOW.contains("ES_OPS_YOUTUBE_CLIENT_ID"),
            "workflow ES_OPS_YOUTUBE_CLIENT_ID icermeli"
        );
        assert!(
            youtube_src.contains("option_env!(\"ES_OPS_YOUTUBE_CLIENT_ID\""),
            "youtube.rs ES_OPS_YOUTUBE_CLIENT_ID option_env ile okumali"
        );

        // Meta: worklow'da ES_OPS_META_APP_ID, Rust'ta option_env!.
        assert!(
            WORKFLOW.contains("ES_OPS_META_APP_ID"),
            "workflow ES_OPS_META_APP_ID icermeli"
        );
        assert!(
            meta_src.contains("option_env!(\"ES_OPS_META_APP_ID\""),
            "meta.rs ES_OPS_META_APP_ID option_env ile okumali"
        );
    }

    /// Uygulamaya client secret asla zorunlu `env!` makrosuyla gömülmemelidir;
    /// yalnız isteğe bağlı `option_env!` (derleme ortamında varsa) veya güvenli
    /// deposu kullanılır. Secret başka platformlarda da istemciye gömülmez.
    /// (Kontrol maddesi: "Secret veya token masaüstü uygulamasına açık biçimde
    /// gömülüyor mu?")
    #[test]
    fn no_platform_uses_required_env_secret() {
        let platforms = [
            ("youtube.rs", include_str!("platforms/youtube.rs")),
            ("linkedin.rs", include_str!("platforms/linkedin.rs")),
            ("tiktok.rs", include_str!("platforms/tiktok.rs")),
            ("x.rs", include_str!("platforms/x.rs")),
            ("meta.rs", include_str!("platforms/meta.rs")),
        ];

        for (name, src) in platforms {
            // Yalnız üretim kodu denetlenir; platformların kendi test modüllerinde
            // `#[cfg(test)]` açıklamaları/satırları env! içerebilir.
            let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
            // "env!(" zorunlu gömme yok. Önce option_env! kullanımları
            // çıkarılır; kalan metinde bağımsız env! kalmamalı.
            let without_option = prod.replace("option_env!(\"", "option_env!('");
            if without_option.contains("env!(\"") {
                panic!("{}: zorunlu env! gommesi bulundu (yalniz option_env! olmali)", name);
            }
            // Gömülü açık secret değeri kalmasın.
            assert!(
                !prod.contains("ES_OPS_META_APP_SECRET="),
                "{}: acik secret degeri yazilmamali", name
            );
        }
    }
}
