use esp32_nimble::utilities::mutex::Mutex;
use esp32_nimble::{BLEAdvertisementData, BLECharacteristic, BLEDevice, NimbleProperties};
use log::{info, warn};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use ayphr_protocol::{
    BLE_FRAME_HEADER_LEN, BLE_NOTIFY_CHUNK_SIZE, FIRMWARE_MANUFACTURER_ID,
    FIRMWARE_RX_CHARACTERISTIC_UUID, FIRMWARE_SERVICE_UUID, FIRMWARE_TX_CHARACTERISTIC_UUID,
    MAX_BLE_FRAME_PAYLOAD,
};

use crate::command_processor;
use crate::config::DeviceSetup;

static BLE_RX_BUFFER: StdMutex<VecDeque<u8>> = StdMutex::new(VecDeque::new());

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
        if let Ok(mut buffer) = BLE_RX_BUFFER.lock() {
            buffer.clear();
        }
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
        // Process each write as a complete command for compatibility
        let response = command_processor::process_ble_request(&setup_handler, payload);
        if !response.is_empty() {
            info!("Sending BLE response bytes={}", response.len());
            notify_response(&tx_writer, &response);
        }
    });

    let dev_name = setup.device_name_for_advertising();
    if let Err(error) = BLEDevice::set_device_name(&dev_name) {
        warn!("Failed to set BLE device name: {:?}", error);
    }

    let mut adv_data = advertising_data(&setup, &dev_name);

    if let Err(error) = ble_advertising.lock().set_data(&mut adv_data) {
        warn!("Failed to configure BLE advertisement data: {:?}", error);
    }
    if let Err(error) = ble_advertising.lock().start() {
        warn!("Failed to start BLE advertising: {:?}", error);
    }

    info!(
        "BLE advertising started: name='{}', UUID='{}', MFG_ID=0x{:04X}",
        dev_name, service_uuid, FIRMWARE_MANUFACTURER_ID
    );
}

fn notify_response(tx_writer: &Arc<Mutex<BLECharacteristic>>, response: &[u8]) {
    if response.len() > u16::MAX as usize {
        warn!(
            "Response of {} bytes exceeds the BLE frame limit",
            response.len()
        );
        return;
    }

    let mut frame = Vec::with_capacity(BLE_FRAME_HEADER_LEN + response.len());
    frame.extend_from_slice(&(response.len() as u16).to_le_bytes());
    frame.extend_from_slice(response);

    let mut writer = tx_writer.lock();
    for chunk in frame.chunks(BLE_NOTIFY_CHUNK_SIZE) {
        writer.set_value(chunk).notify();
    }
}
