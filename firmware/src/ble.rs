use esp32_nimble::{BLEAdvertisementData, BLEDevice, NimbleProperties};
use log::{info, warn};
use std::sync::Arc;

use ayphr_protocol::{
    FIRMWARE_MANUFACTURER_ID, FIRMWARE_RX_CHARACTERISTIC_UUID, FIRMWARE_SERVICE_UUID,
    FIRMWARE_TX_CHARACTERISTIC_UUID,
};

use crate::command_processor;
use crate::config::DeviceSetup;

/// Builds the advertisement payload advertising the given device state.
fn advertising_data(setup: &DeviceSetup, dev_name: &str) -> BLEAdvertisementData {
    let service_uuid = esp32_nimble::uuid128!(FIRMWARE_SERVICE_UUID);

    let mut adv_data = BLEAdvertisementData::new();
    adv_data.name(dev_name);
    adv_data.add_service_uuid(service_uuid);

    let setup_complete_flag = if setup.is_configured() { 1u8 } else { 0u8 };
    let mut mfg_data = Vec::new();
    mfg_data.extend_from_slice(&FIRMWARE_MANUFACTURER_ID.to_le_bytes());
    mfg_data.push(setup_complete_flag);
    adv_data.manufacturer_data(&mfg_data);

    adv_data
}

/// Re-applies the device name and advertisement payload so a rename or setup
/// change is visible to scanners without a reboot. Best effort: failures are
/// logged and the previous advertisement stays in place.
pub fn refresh_advertising(setup: &DeviceSetup) {
    let dev_name = setup.device_name_for_advertising();

    if let Err(error) = BLEDevice::set_device_name(&dev_name) {
        warn!("Failed to update BLE device name: {:?}", error);
        return;
    }

    let mut adv_data = advertising_data(setup, &dev_name);

    let advertising = BLEDevice::take().get_advertising();
    let mut advertising = advertising.lock();

    if advertising.is_advertising() {
        if let Err(error) = advertising.stop() {
            warn!("Failed to stop advertising before refresh: {:?}", error);
            return;
        }
    }

    if let Err(error) = advertising.set_data(&mut adv_data) {
        warn!("Failed to apply refreshed advertisement data: {:?}", error);
        return;
    }

    if let Err(error) = advertising.start() {
        warn!("Failed to restart advertising after refresh: {:?}", error);
    }
}

pub fn init(setup: Arc<DeviceSetup>) {
    let ble_device = BLEDevice::take();
    let ble_advertising = ble_device.get_advertising();
    let server = ble_device.get_server();

    let setup_on_disconnect = Arc::clone(&setup);
    server.on_connect(move |_server, _desc| {
        info!("BLE Central Connected");
    });
    server.on_disconnect(move |_desc, _reason| {
        info!("BLE Central Disconnected");
        setup_on_disconnect.reset_authentication();
    });

    let service_uuid = esp32_nimble::uuid128!(FIRMWARE_SERVICE_UUID);
    let rx_uuid = esp32_nimble::uuid128!(FIRMWARE_RX_CHARACTERISTIC_UUID);
    let tx_uuid = esp32_nimble::uuid128!(FIRMWARE_TX_CHARACTERISTIC_UUID);

    let service = server.create_service(service_uuid);
    let rx_char = service.lock().create_characteristic(
        rx_uuid,
        NimbleProperties::WRITE | NimbleProperties::WRITE_NO_RSP,
    );
    let tx_char = service
        .lock()
        .create_characteristic(tx_uuid, NimbleProperties::READ | NimbleProperties::NOTIFY);

    let setup_handler = Arc::clone(&setup);
    let tx_writer = Arc::clone(&tx_char);

    rx_char.lock().on_write(move |args| {
        let payload = args.recv_data();
        info!("Received BLE request bytes={}", payload.len());

        let response = command_processor::process_ble_request(&setup_handler, payload);
        if !response.is_empty() {
            info!("Sending BLE response bytes={}", response.len());
            tx_writer.lock().set_value(&response).notify();
        }
    });

    let dev_name = setup.device_name_for_advertising();
    BLEDevice::set_device_name(&dev_name).unwrap();

    let mut adv_data = advertising_data(&setup, &dev_name);

    ble_advertising.lock().set_data(&mut adv_data).unwrap();
    ble_advertising.lock().start().unwrap();

    info!(
        "BLE advertising started: name='{}', UUID='{}', MFG_ID=0x{:04X}",
        dev_name, service_uuid, FIRMWARE_MANUFACTURER_ID
    );
}
