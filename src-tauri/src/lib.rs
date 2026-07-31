mod social;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            social::commands::social_platform_catalog,
            social::commands::social_account_connections,
            social::commands::social_account_status,
            social::commands::social_disconnect_account,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
