use crate::device_setup::DeviceSetup;
use bme280_multibus::Bme280;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::AnyIOPin;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::peripherals::Peripherals;
use esp_idf_svc::hal::uart::{config::Config as UartConfig, UartDriver};
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi};
use esp32_nimble::{BLEAdvertisementData, BLEDevice, NimbleProperties};
use log::{error, info, warn};
use std::sync::mpsc::{channel, Sender};
use std::sync::Arc;
use std::time::Duration;

use ayphr_protocol::{
    FIRMWARE_MANUFACTURER_ID, FIRMWARE_RX_CHARACTERISTIC_UUID, FIRMWARE_SERVICE_UUID,
    FIRMWARE_TX_CHARACTERISTIC_UUID,
};

const SERIAL_BAUD_RATE: u32 = 115_200;

enum SystemEvent {
    BleRequest(Vec<u8>),
    ConfigurationUpdated,
}

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Booting firmware...");

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    let setup = DeviceSetup::new(nvs_partition.clone())?;
    let (event_tx, event_rx) = channel::<SystemEvent>();

    let mut wifi = BlockingWifi::wrap(
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

    let event_tx_ble = event_tx.clone();
    rx_char.lock().on_write(move |args| {
        let payload = args.recv_data();
        let _ = event_tx_ble.send(SystemEvent::BleRequest(payload.to_vec()));
    });

    let uart1 = peripherals.uart1;
    let serial_tx_pin = peripherals.pins.gpio17;
    let serial_rx_pin = peripherals.pins.gpio16;
    let serial_setup = Arc::clone(&setup);
    let event_tx_serial = event_tx.clone();

    std::thread::spawn(move || {
        let config = UartConfig::default().baudrate(Hertz(SERIAL_BAUD_RATE));
        let uart = match UartDriver::new(
            uart1,
            serial_tx_pin,
            serial_rx_pin,
            Option::<AnyIOPin>::None,
            Option::<AnyIOPin>::None,
            &config,
        ) {
            Ok(driver) => driver,
            Err(error) => {
                error!("Failed to initialize serial transport: {:?}", error);
                return;
            }
        };

        loop {
            let mut length_buf = [0u8; 2];
            if read_exact_timeout(&uart, &mut length_buf, 5000).is_err() {
                continue;
            }

            let payload_len = u16::from_le_bytes(length_buf) as usize;
            if payload_len == 0 || payload_len > 512 {
                continue;
            }

            let mut payload = vec![0u8; payload_len];
            if read_exact_timeout(&uart, &mut payload, 1000).is_err() {
                continue;
            }

            let response = serial_setup.process_serial_request(&payload);
            let _ = write_frame(&uart, &response);
            let _ = event_tx_serial.send(SystemEvent::ConfigurationUpdated);
        }
    });

    update_advertising(&setup, &service_uuid, &ble_advertising)?;

    let setup_worker = Arc::clone(&setup);
    let tx_char_writer = Arc::clone(&tx_char);
    let event_tx_worker = event_tx.clone();

    std::thread::spawn(move || {
        while let Ok(event) = event_rx.recv() {
            match event {
                SystemEvent::BleRequest(payload) => {
                    let response = setup_worker.process_request(&payload);
                    if !response.is_empty() {
                        tx_char_writer.lock().set_value(&response).notify();
                    }
                    let _ = event_tx_worker.send(SystemEvent::ConfigurationUpdated);
                }
                SystemEvent::ConfigurationUpdated => {
                    let _ = update_advertising(&setup_worker, &service_uuid, &ble_advertising);
                    let (ssid, pass) = setup_worker.wifi_credentials();
                    if !ssid.is_empty() {
                        let _ = connect_wifi(&mut wifi, &ssid, &pass);
                    }
                }
            }
        }
    });

    let event_tx_init = event_tx.clone();
    let _ = event_tx_init.send(SystemEvent::ConfigurationUpdated);

    let i2c0 = peripherals.i2c0;
    let sda = peripherals.pins.gpio21;
    let scl = peripherals.pins.gpio22;

    std::thread::spawn(move || {
        let i2c_config = I2cConfig::new().baudrate(Hertz(400_000).into());
        let mut driver_option = I2cDriver::new(i2c0, sda, scl, &i2c_config).ok();
        let mut backoff_secs = 2;

        loop {
            if let Some(ref driver) = driver_option {
                match Bme280::from_i2c0(driver, bme280_multibus::Address::SdoGnd) {
                    Ok(mut sensor) => {
                        if sensor.settings(&bme280_multibus::Settings::default()).is_ok() {
                            info!("BME280 initialized dynamic rediscovery");
                            backoff_secs = 2;
                            loop {
                                match sensor.sample() {
                                    Ok(m) => {
                                        info!(
                                            "Sensor -> Temp: {:.2}°C, Hum: {:.2}%, Pres: {:.2} hPa",
                                            m.temperature,
                                            m.humidity,
                                            m.pressure / 100.0
                                        );
                                    }
                                    Err(_) => {
                                        warn!("Sensor sample failed, forcing rediscovery");
                                        break;
                                    }
                                }
                                std::thread::sleep(Duration::from_secs(2));
                            }
                        }
                    }
                    Err(_) => {
                        warn!("Sensor not detected on bus");
                    }
                }
            } else {
                driver_option = I2cDriver::new(i2c0, sda, scl, &i2c_config).ok();
            }

            std::thread::sleep(Duration::from_secs(backoff_secs));
            backoff_secs = (backoff_secs * 2).min(30);
        }
    });

    loop {
        std::thread::sleep(Duration::from_secs(10));
    }
}

fn update_advertising(
    setup: &DeviceSetup,
    service_uuid: &esp32_nimble::UUID,
    adv_handle: &Arc<Mutex<esp32_nimble::BLEAdvertising>>,
) -> anyhow::Result<()> {
    let dev_name = setup.device_name_for_advertising();
    esp32_nimble::BLEDevice::set_device_name(&dev_name)?;

    let mut adv_data = BLEAdvertisementData::new();
    adv_data.name(&dev_name);
    adv_data.add_service_uuid(*service_uuid);

    let setup_complete_flag = if setup.is_configured() { 1u8 } else { 0u8 };
    let mut mfg_data = Vec::new();
    mfg_data.extend_from_slice(&FIRMWARE_MANUFACTURER_ID.to_le_bytes());
    mfg_data.push(setup_complete_flag);

    adv_data.manufacturer_data(&mfg_data);

    let mut adv = adv_handle.lock();
    adv.stop()?;
    adv.set_data(&mut adv_data)?;
    adv.start()?;
    Ok(())
}

fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>, ssid: &str, pass: &str) -> anyhow::Result<()> {
    info!("Attempting WiFi connection to SSID: {}", ssid);
    let mut auth = AuthMethod::WPA2Personal;
    if pass.is_empty() {
        auth = AuthMethod::None;
    }

    let config = Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().unwrap_or_default(),
        password: pass.try_into().unwrap_or_default(),
        auth_method: auth,
        ..Default::default()
    });

    wifi.set_configuration(&config)?;
    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;
    info!("WiFi connected successfully");
    Ok(())
}

fn read_exact_timeout(uart: &UartDriver<'_>, buf: &mut [u8], timeout_ms: u32) -> anyhow::Result<()> {
    let mut offset = 0;
    let ticks = esp_idf_svc::hal::delay::TickType::from_millis(timeout_ms).ticks();

    while offset < buf.len() {
        let read = uart.read(&mut buf[offset..], ticks)?;
        if read == 0 {
            anyhow::bail!("Read timeout");
        }
        offset += read;
    }
    Ok(())
}

fn write_frame(uart: &UartDriver<'_>, payload: &[u8]) -> anyhow::Result<()> {
    let len = u16::try_from(payload.len())?;
    let mut frame = Vec::with_capacity(2 + payload.len());
    frame.extend_from_slice(&len.to_le_bytes());
    frame.extend_from_slice(payload);

    let mut written = 0;
    while written < frame.len() {
        written += uart.write(&frame[written..])?;
    }

    Ok(())
}
