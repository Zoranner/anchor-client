use tauri::Manager;

use instance::SessionInstance;

pub mod instance;
pub mod state;

use state::NativeState;

pub fn run() {
    let _session_instance = match SessionInstance::acquire() {
        Ok(Some(instance)) => instance,
        Ok(None) => return,
        Err(error) => {
            eprintln!("{error}");
            return;
        }
    };
    let show_on_start = std::env::args_os().any(|argument| argument == "--show");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            app.manage(NativeState::default());
            crate::preferences::tray::install(app.handle())?;
            if show_on_start {
                crate::preferences::tray::show_main_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(crate::preferences::tray::hide_on_close)
        .invoke_handler(tauri::generate_handler![
            crate::commands::policy::get_policy_status,
            crate::commands::policy::inspect_policy_file,
            crate::commands::policy::import_pending_policy,
            crate::commands::policy::discard_pending_policy,
            crate::commands::policy::get_audit_page,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Anchor Guard desktop");
}
