mod ble;

use ble::{
    authenticate_ble_device, change_ble_device_password, connect_ble_device, disconnect_ble_device,
    factory_reset_ble_device, get_ble_devices, restart_ble_device, scan_ble_devices,
    submit_ble_setup, update_ble_device_wifi, BleDeviceStore,
};
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
use tracing::{error, info};
use tracing_subscriber;

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

            app.manage(device_store.clone());

            tauri::async_runtime::spawn(async move {
                if let Err(error) = scan_ble_devices(app_handle, device_store).await {
                    error!("failed to scan BLE devices: {error}");
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_ble_devices,
            connect_ble_device,
            authenticate_ble_device,
            submit_ble_setup,
            disconnect_ble_device,
            restart_ble_device,
            factory_reset_ble_device,
            change_ble_device_password,
            update_ble_device_wifi
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
