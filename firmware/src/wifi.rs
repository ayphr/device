use esp_idf_svc::wifi::{BlockingWifi, Configuration, EspWifi};
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

    apply_config_and_connect(wifi, ssid, password);
}

/// Applies newly stored credentials and reconnects the WiFi stack.
/// Can be called after the radio is already started and/or connected,
/// e.g. when the user updates the WiFi settings at runtime.
pub fn reconnect(wifi: &mut BlockingWifi<EspWifi<'_>>, ssid: &str, password: &str) {
    if ssid.is_empty() {
        warn!("No WiFi credentials configured; skipping WiFi reconnection");
        return;
    }

    info!("Reconnecting to WiFi network '{}'", ssid);

    if wifi.is_started().unwrap_or(false) && wifi.is_connected().unwrap_or(false) {
        info!("Disconnecting from current WiFi network before applying new credentials");
        if let Err(error) = wifi.disconnect() {
            error!("Failed to disconnect from WiFi: {:?}", error);
        }
    }

    if !wifi.is_started().unwrap_or(false) {
        if let Err(error) = wifi.start() {
            error!("Failed to start WiFi: {:?}", error);
            return;
        }
    }

    apply_config_and_connect(wifi, ssid, password);
}

fn apply_config_and_connect(
    wifi: &mut BlockingWifi<EspWifi<'_>>,
    ssid: &str,
    password: &str,
) {
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

    let client_config = esp_idf_svc::wifi::ClientConfiguration {
        ssid: ssid_heapless,
        password: password_heapless,
        ..Default::default()
    };

    if let Err(error) = wifi.set_configuration(&Configuration::Client(client_config)) {
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