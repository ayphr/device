mod ble;
mod serial;

use ble::{
    authenticate_ble_device, change_ble_device_password, connect_ble_device, disconnect_ble_device,
    factory_reset_ble_device, get_ble_devices, restart_ble_device, scan_ble_devices,
    submit_ble_setup, update_ble_device_wifi, BleDeviceStore,
};
use serial::{
    change_serial_device_password, connect_serial_device, factory_reset_serial_device,
    get_serial_devices, restart_serial_device, scan_serial_devices, submit_serial_setup,
    update_serial_device_wifi, SerialDeviceStore,
};
use tauri::menu::{MenuBuilder, MenuItem};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tracing::{error, info};
use tracing_subscriber;

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    info!("starting companion_lib");

    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::default(), None))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app_handle = app.handle().clone();
            let device_store = BleDeviceStore::default();
            let serial_store = SerialDeviceStore::default();
            let tray_menu = MenuBuilder::new(app)
                .item(&MenuItem::with_id(app, "show", "Show Ayphr Companion", true, None::<&str>)?)
                .item(&MenuItem::with_id(app, "hide", "Hide Ayphr Companion", true, None::<&str>)?)
                .separator()
                .item(&MenuItem::with_id(app, "quit", "Quit Ayphr Companion", true, None::<&str>)?)
                .build()?;

            app.manage(device_store.clone());
            app.manage(serial_store.clone());

            if let Some(icon) = app.default_window_icon().cloned() {
                let _tray = tauri::tray::TrayIconBuilder::with_id("main")
                    .menu(&tray_menu)
                    .icon(icon)
                    .on_menu_event(|app, event| match event.id().as_ref() {
                        "show" => show_main_window(app),
                        "hide" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.hide();
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, _event| {
                        show_main_window(tray.app_handle());
                    })
                    .build(app)?;
            }

            tauri::async_runtime::spawn(async move {
                if let Err(error) = scan_ble_devices(app_handle, device_store).await {
                    error!("failed to scan BLE devices: {error}");
                }
            });

            let serial_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = scan_serial_devices(serial_app_handle, serial_store).await {
                    error!("failed to scan serial devices: {error}");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_ble_devices,
            get_serial_devices,
            connect_ble_device,
            authenticate_ble_device,
            submit_ble_setup,
            disconnect_ble_device,
            restart_ble_device,
            factory_reset_ble_device,
            change_ble_device_password,
            update_ble_device_wifi,
            connect_serial_device,
            submit_serial_setup,
            restart_serial_device,
            factory_reset_serial_device,
            change_serial_device_password,
            update_serial_device_wifi
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
