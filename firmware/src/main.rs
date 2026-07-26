mod device_setup;

use crate::device_setup::DeviceSetup;
use bme280_multibus::Bme280;
use esp32_nimble::{BLEDevice, NimbleProperties, BLEAdvertisementData};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use log::{error, info, warn};
use std::sync::Arc;
use std::time::Duration;

use ayphr_protocol::{FIRMWARE_MANUFACTURER_ID, FIRMWARE_RX_CHARACTERISTIC_UUID, FIRMWARE_SERVICE_UUID, FIRMWARE_TX_CHARACTERISTIC_UUID};

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Booting firmware...");

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    let setup = DeviceSetup::new(nvs_partition.clone())?;

    let _wifi = BlockingWifi::wrap(
        EspWifi::new(
            peripherals.modem,
            sys_loop.clone(),
            Some(nvs_partition.clone()),
        )?,
        sys_loop,
    )?;

    let ble_device = BLEDevice::take();
    let ble_advertising = ble_device.get_advertising();

    let server = ble_device.get_server();
    let setup_clone = Arc::clone(&setup);

    server.on_connect(move |_server, _desc| {
        info!("BLE Central Connected");
    });

    server.on_disconnect(move |_desc, _reason| {
        info!("BLE Central Disconnected");
        setup_clone.reset_authentication();
    });

    let service_uuid = esp32_nimble::uuid128!(FIRMWARE_SERVICE_UUID);
    let rx_uuid = esp32_nimble::uuid128!(FIRMWARE_RX_CHARACTERISTIC_UUID);
    let tx_uuid = esp32_nimble::uuid128!(FIRMWARE_TX_CHARACTERISTIC_UUID);

    let service = server.create_service(service_uuid);
    let rx_char = service
        .lock()
        .create_characteristic(rx_uuid, NimbleProperties::WRITE);
    let tx_char = service.lock().create_characteristic(
        tx_uuid,
        NimbleProperties::READ | NimbleProperties::NOTIFY,
    );

    let setup_request_handler = Arc::clone(&setup);
    let tx_char_writer = Arc::clone(&tx_char);

    rx_char.lock().on_write(move |args| {
        let payload = args.recv_data();
        info!("Received BLE setup request bytes={}", payload.len());

        let response = setup_request_handler.process_request(payload);
        if !response.is_empty() {
            info!("Sending BLE setup response bytes={}", response.len());
            tx_char_writer.lock().set_value(&response).notify();
        }
    });

    let dev_name = setup.device_name_for_advertising();
    esp32_nimble::BLEDevice::set_device_name(&dev_name).unwrap();

    let mut adv_data = BLEAdvertisementData::new();
    adv_data.name(&dev_name);
    adv_data.add_service_uuid(service_uuid);

    let setup_complete_flag = if setup.is_configured() { 1u8 } else { 0u8 };
    let manufacturer_data_payload = vec![setup_complete_flag];

    let mut mfg_data = Vec::new();
    mfg_data.extend_from_slice(&FIRMWARE_MANUFACTURER_ID.to_le_bytes());
    mfg_data.extend_from_slice(&manufacturer_data_payload);

    adv_data.manufacturer_data(&mfg_data);

    ble_advertising.lock().set_data(&mut adv_data)?;
    ble_advertising.lock().start()?;

    info!(
        "BLE advertising started with name='{}', Service UUID='{}', Manufacturer ID=0x{:04X}",
        dev_name, service_uuid, FIRMWARE_MANUFACTURER_ID
    );

    let i2c_config = I2cConfig::new().baudrate(Hertz(400_000).into());
    let i2c_driver = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio21, // SDA
        peripherals.pins.gpio22, // SCL
        &i2c_config,
    )?;

    let mut bme280 = match Bme280::from_i2c0(i2c_driver, bme280_multibus::Address::SdoGnd) {
        Ok(mut sensor) => {
            if let Err(e) = sensor.settings(&bme280_multibus::Settings::default()) {
                error!("Failed to configure BME280 sensor: {:?}", e);
                None
            } else {
                info!("BME280 sensor successfully initialized over I2C");
                Some(sensor)
            }
        }
        Err(e) => {
            warn!("BME280 sensor not found on I2C bus: {:?}", e);
            None
        }
    };

    loop {
        if let Some(ref mut sensor) = bme280 {
            match sensor.sample() {
                Ok(measurements) => {
                    info!(
                        "Sensor Readings -> Temp: {:.2}°C, Humidity: {:.2}%, Pressure: {:.2} hPa",
                        measurements.temperature,
                        measurements.humidity,
                        measurements.pressure / 100.0
                    );
                }
                Err(_) => {
                    warn!("Failed to read measurement sample from BME280 sensor");
                }
            }
        }

        std::thread::sleep(Duration::from_secs(2));
    }
}
