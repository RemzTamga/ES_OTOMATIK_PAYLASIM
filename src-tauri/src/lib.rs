mod license;
mod logging;
mod social;
mod site;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Rust panic'lerini uygulamanin veri klasorundeki log dosyasina yazar ve
    // kullaniciya gorunur bir uyari gosterir (sessiz kapanmayi onler).
    logging::install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Uygulama basladığında veri klasörü yapısını hazırlayıp bilgi satırı yazar.
            let dir = app.path().app_data_dir().ok();
            if let Some(base) = dir {
                crate::logging::set_data_dir(base.clone());
                let log = base.join("logs");
                let _ = std::fs::create_dir_all(&log);
                let _ = crate::logging::log_append_to_path(&log.join("es-ops.log"), "INFO", "ES OPS baslatildi (surum 1.0.0)");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::logging::log_append,
            crate::logging::log_open_folder,
            crate::logging::log_export_to,
            social::commands::social_platform_catalog,
            social::commands::social_account_connections,
            social::commands::social_account_status,
            social::commands::social_disconnect_account,
            social::commands::pick_video_file,
            social::commands::pick_media_files,
            social::commands::youtube_connect,
            social::commands::youtube_upload_video,
            social::commands::facebook_connect,
            social::commands::instagram_connect,
            social::commands::meta_config_status,
            social::commands::meta_set_config,
            social::commands::meta_clear_config,
            social::commands::facebook_publish,
            social::commands::instagram_publish,
            social::commands::tiktok_connect,
            social::commands::tiktok_config_status,
            social::commands::tiktok_set_config,
            social::commands::tiktok_clear_config,
            social::commands::tiktok_publish,
            social::commands::x_connect,
            social::commands::x_config_status,
            social::commands::x_set_config,
            social::commands::x_clear_config,
            social::commands::x_publish,
            social::commands::linkedin_connect,
            social::commands::linkedin_config_status,
            social::commands::linkedin_set_config,
            social::commands::linkedin_clear_config,
            social::commands::linkedin_publish,
            social::commands::pinterest_connect,
            social::commands::pinterest_config_status,
            social::commands::pinterest_set_config,
            social::commands::pinterest_clear_config,
            social::commands::pinterest_publish,
            site::website_config_get,
            site::website_config_save,
            site::website_config_clear,
            site::website_test,
            site::website_publish,
            site::website_sections,
            license::license_install,
            license::license_status,
            license::license_machine_id,
            license::license_clear,
          ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

