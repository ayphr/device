use esp_idf_svc::wifi::{BlockingWifi, EspWifi};
use log::{error, info, warn};

pub fn connect(wifi: &mut BlockingWifi<EspWifi<'_>>, ssid: &str, password: &str) {
    if ssid.is_empty() {
        warn!("No WiFi credentials configured; skipping WiFi connection");
        return;
    }

    info!("Connecting to WiFi network '{}'", ssid);

    if let Err(error) = wifi.start() {
        error!("Failed to start WiFi: {:?}", error);
        return;
    }

    let ssid_heapless: heapless::String<32> = match ssid.try_into() {
        Ok(s) => s,
        Err(_) => {
            error!("WiFi SSID too long (max 32 bytes)");
            return;
        }
    };

    let password_heapless: heapless::String<64> = match password.try_into() {
        Ok(p) => p,
        Err(_) => {
            error!("WiFi password too long (max 64 bytes)");
            return;
        }
    };

    let mut client_config = esp_idf_svc::wifi::ClientConfiguration::default();
    client_config.ssid = ssid_heapless;
    client_config.password = password_heapless;

    if let Err(error) = wifi.set_configuration(&esp_idf_svc::wifi::Configuration::Client(
        client_config,
    )) {
        error!("Failed to set WiFi configuration: {:?}", error);
        return;
    }

    if let Err(error) = wifi.connect() {
        error!("Failed to connect to WiFi: {:?}", error);
        return;
    }

    if let Err(error) = wifi.wait_netif_up() {
        error!("Failed to wait for network interface: {:?}", error);
        return;
    }

    info!("WiFi connected successfully");
}
