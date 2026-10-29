use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime, WindowEvent,
};

const SHOW_MENU_ID: &str = "show";
const IMPORT_MENU_ID: &str = "import";
const HIDE_MENU_ID: &str = "hide";
const TRAY_IMPORT_EVENT: &str = "tray:import-policy";

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let version = MenuItem::with_id(
        app,
        "version",
        format!("Anchor Guard v{}", app.package_info().version),
        false,
        None::<&str>,
    )?;
    let show = MenuItem::with_id(app, SHOW_MENU_ID, "显示窗口", true, None::<&str>)?;
    let import = MenuItem::with_id(app, IMPORT_MENU_ID, "导入策略…", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, HIDE_MENU_ID, "隐藏窗口", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&version, &separator, &show, &import, &hide])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("the packaged Anchor Guard icon should be available at runtime");

    TrayIconBuilder::with_id("anchor-guard")
        .icon(icon)
        .tooltip("Anchor Guard")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                show_main_window(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            SHOW_MENU_ID => show_main_window(app),
            IMPORT_MENU_ID => {
                show_main_window(app);
                let _ = app.emit(TRAY_IMPORT_EVENT, ());
            }
            HIDE_MENU_ID => hide_main_window(app),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

pub fn hide_on_close<R: Runtime>(window: &tauri::Window<R>, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        let _ = window.hide();
        api.prevent_close();
    }
}

pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn hide_main_window<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}
