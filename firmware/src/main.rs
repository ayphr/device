mod ble;
mod command_processor;
mod config;
mod serial;
mod wifi;

use crate::config::DeviceSetup;
use bme280_multibus::Bme280;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::AnyIOPin;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::uart::{config::Config as UartConfig, UartDriver};
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::BlockingWifi;
use log::{error, info, warn};
use std::time::Duration;

const SERIAL_BAUD_RATE: u32 = 115_200;
const I2C_FREQ_HZ: u32 = 400_000;
const SENSOR_SAMPLE_INTERVAL_SECS: u64 = 2;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Booting firmware...");

    let peripherals = esp_idf_svc::hal::peripherals::Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    let setup = DeviceSetup::new(nvs_partition.clone())?;

    let mut _wifi = BlockingWifi::wrap(
        esp_idf_svc::wifi::EspWifi::new(
            peripherals.modem,
            sys_loop.clone(),
            Some(nvs_partition.clone()),
        )?,
        sys_loop,
    )?;

    ble::init(setup.clone());

    let (ssid, password) = setup.load_wifi_credentials();
    wifi::connect(&mut _wifi, &ssid, &password);

    let uart1 = peripherals.uart1;
    let serial_tx = peripherals.pins.gpio17;
    let serial_rx = peripherals.pins.gpio16;
    let serial_setup = setup.clone();
    std::thread::spawn(move || {
        let config = UartConfig::default().baudrate(Hertz(SERIAL_BAUD_RATE));
        let uart = match UartDriver::new(
            uart1,
            serial_tx,
            serial_rx,
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

        info!("Serial transport ready on UART1 at {} baud", SERIAL_BAUD_RATE);
        serial::run_loop(&uart, &serial_setup);
    });

    let i2c_config = I2cConfig::new().baudrate(Hertz(I2C_FREQ_HZ).into());
    let i2c_driver = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio21,
        peripherals.pins.gpio22,
        &i2c_config,
    )?;

    let mut bme280 = match Bme280::from_i2c0(i2c_driver, bme280_multibus::Address::SdoGnd) {
        Ok(mut sensor) => match sensor.settings(&bme280_multibus::Settings::default()) {
            Ok(()) => {
                info!("BME280 sensor initialized over I2C");
                Some(sensor)
            }
            Err(e) => {
                error!("Failed to configure BME280 sensor: {:?}", e);
                None
            }
        },
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
                        "Sensor -> Temp: {:.2}°C, Humidity: {:.2}%, Pressure: {:.2} hPa",
                        measurements.temperature,
                        measurements.humidity,
                        measurements.pressure / 100.0
                    );
                }
                Err(_) => {
                    warn!("Failed to read BME280 sensor sample");
                }
            }
        }

        std::thread::sleep(Duration::from_secs(SENSOR_SAMPLE_INTERVAL_SECS));
    }
}
