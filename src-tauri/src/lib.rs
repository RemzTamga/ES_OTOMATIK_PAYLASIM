mod social;
mod site;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
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
          ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

