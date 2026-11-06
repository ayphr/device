mod ble;
mod clock;
mod cloud;
mod command_processor;
mod config;
mod identity;
mod live_data;
mod ota;
mod serial;
mod telemetry;
mod wifi;

use crate::config::DeviceSetup;
use ayphr_protocol::{SensorPacket, PACKET_TYPE_SENSOR};
use ayphr_protocol::{MAX_SAMPLING_INTERVAL_SECS, MIN_SAMPLING_INTERVAL_SECS};
use bme280_multibus::{Bme280, Config, CtrlMeas, Mode, Oversampling, Settings};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::hal::gpio::AnyIOPin;
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_svc::hal::uart::{config::Config as UartConfig, UartDriver};
use esp_idf_svc::hal::units::Hertz;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::BlockingWifi;
use log::{error, info, warn};
use std::time::{Duration, Instant};

const SERIAL_BAUD_RATE: u32 = 115_200;
const I2C_FREQ_HZ: u32 = 400_000;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!("Booting firmware...");

    command_processor::init_boot_time();

    let peripherals = esp_idf_svc::hal::peripherals::Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs_partition = EspDefaultNvsPartition::take()?;

    let setup = DeviceSetup::new(nvs_partition.clone())?;

    let mut wifi = BlockingWifi::wrap(
        esp_idf_svc::wifi::EspWifi::new(
            peripherals.modem,
            sys_loop.clone(),
            Some(nvs_partition.clone()),
        )?,
        sys_loop,
    )?;

    ble::init(setup.clone());

    let (ssid, password) = setup.load_wifi_credentials();
    wifi::connect(&mut wifi, &ssid, &password);

    let serial_setup = setup.clone();
    let uart1 = peripherals.uart1;
    let serial_tx = peripherals.pins.gpio5;
    let serial_rx = peripherals.pins.gpio4;
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

        info!(
            "Serial transport ready on UART1 at {} baud",
            SERIAL_BAUD_RATE
        );
        serial::run_loop(&uart, serial_setup);
    });

    let i2c_config = I2cConfig::new().baudrate(Hertz(I2C_FREQ_HZ));
    let i2c_driver = I2cDriver::new(
        peripherals.i2c0,
        peripherals.pins.gpio0,
        peripherals.pins.gpio1,
        &i2c_config,
    )?;

    let mut bme280 = match Bme280::from_i2c0(i2c_driver, bme280_multibus::Address::SdoGnd) {
        Ok(mut sensor) => match sensor.settings(&Settings {
            config: Config::RESET,
            ctrl_meas: CtrlMeas::RESET
                .set_osrs_t(Oversampling::X1)
                .set_osrs_p(Oversampling::X1)
                .set_mode(Mode::Sleep),
            ctrl_hum: Oversampling::X1,
        }) {
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

    // Start clock synchronization and telemetry upload loop.
    let _sntp = clock::start();
    cloud::init(&setup);
    telemetry::start(setup.clone());

    // Mark valid after subsystems initialized
    ota::mark_valid();

    let serial = identity::serial();
    info!("Derived device serial from MAC: {}", serial);

    let mut last_sample: Option<Instant> = None;
    let mut mock_state: Option<(f32, f32, f32)> = None;

    loop {
        if command_processor::take_wifi_reconnect_request() {
            let (ssid, password) = setup.load_wifi_credentials();
            wifi::reconnect(&mut wifi, &ssid, &password);
        }

        let sampling_interval_secs = {
            let state = setup.lock_state();
            state.data.sampling_interval_secs
        };
        let sampling_interval_secs =
            sampling_interval_secs.clamp(MIN_SAMPLING_INTERVAL_SECS, MAX_SAMPLING_INTERVAL_SECS);

        let now = Instant::now();
        let should_sample = last_sample.map_or(true, |last| {
            last.elapsed().as_secs() >= sampling_interval_secs as u64
        });

        if should_sample {
            let reading = if let Some(sensor) = bme280.as_mut() {
                let forced = Settings {
                    config: Config::RESET,
                    ctrl_meas: CtrlMeas::RESET
                        .set_osrs_t(Oversampling::X1)
                        .set_osrs_p(Oversampling::X1)
                        .set_mode(Mode::Forced),
                    ctrl_hum: Oversampling::X1,
                };
                let sample = match sensor.settings(&forced) {
                    Ok(()) => {
                        std::thread::sleep(Duration::from_millis(10));
                        sensor.sample().ok()
                    }
                    Err(_) => None,
                };

                match sample {
                    Some(measurements) => (
                        measurements.temperature,
                        measurements.humidity,
                        measurements.pressure / 100.0,
                    ),
                    None => {
                        warn!("Failed to read BME280 sensor sample; using mock data");
                        mock_reading(&mut mock_state)
                    }
                }
            } else {
                mock_reading(&mut mock_state)
            };

            let (temperature, humidity, pressure_hpa) = reading;

            info!(
                "Sensor -> Temp: {:.2}°C, Humidity: {:.2}%, Pressure: {:.2} hPa",
                temperature, humidity, pressure_hpa
            );

            live_data::update(SensorPacket {
                packet_type: PACKET_TYPE_SENSOR,
                temperature,
                humidity,
                air_pressure: pressure_hpa.round() as u16,
                serial,
            });

            if clock::is_synced() {
                if let Some(timestamp_ms) = clock::now_unix_ms() {
                    telemetry::record(telemetry::TelemetryPoint {
                        timestamp_ms,
                        temperature,
                        humidity,
                        air_pressure: pressure_hpa,
                    });
                }
            }

            last_sample = Some(now);
        }

        // Sleep in short chunks so WiFi reconnect requests and interval
        // changes are handled promptly without busy-waiting.
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn mock_reading(state: &mut Option<(f32, f32, f32)>) -> (f32, f32, f32) {
    use esp_idf_svc::sys::esp_random;

    let (temperature, humidity, pressure) = match state {
        Some(current) => {
            let seed = unsafe { esp_random() };
            let d_temp = ((seed & 0x1F) as f32 - 15.5) * 0.05;
            let d_hum = (((seed >> 5) & 0x1F) as f32 - 15.5) * 0.1;
            let d_pressure = (((seed >> 10) & 0x1F) as f32 - 15.5) * 0.05;

            current.0 = (current.0 + d_temp).clamp(18.0, 28.0);
            current.1 = (current.1 + d_hum).clamp(35.0, 65.0);
            current.2 = (current.2 + d_pressure).clamp(995.0, 1025.0);
            *current
        }
        None => {
            let seed = unsafe { esp_random() };
            let initial = (
                20.0 + (seed & 0xFF) as f32 / 25.5,
                45.0 + ((seed >> 8) & 0xFF) as f32 / 12.75,
                1005.0 + ((seed >> 16) & 0xFF) as f32 / 10.2,
            );
            *state = Some(initial);
            initial
        }
    };

    (temperature, humidity, pressure)
}
