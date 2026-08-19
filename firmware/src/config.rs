use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use log::{info, warn};
use std::sync::{Arc, Mutex};

pub const DEFAULT_DEVICE_NAME: &str = "Unconfigured Geo";

const NVS_NAMESPACE: &str = "device_setup";
const NVS_KEY_CONFIGURED: &str = "configured";
const NVS_KEY_NAME: &str = "name";
const NVS_KEY_WIFI_SSID: &str = "wifi_ssid";
const NVS_KEY_WIFI_PASS: &str = "wifi_pass";
const NVS_KEY_DEV_PASS: &str = "dev_pass";
const NVS_KEY_AUTH_REQUIRED: &str = "auth_required";
const NVS_KEY_WIFI_REQUIRED: &str = "wifi_required";

#[derive(Clone)]
pub struct DeviceSetupData {
    pub device_name: String,
    pub wifi_ssid: String,
    pub wifi_password: String,
    pub device_password: String,
    pub auth_required: bool,
    pub wifi_required: bool,
    pub configured: bool,
}

impl Default for DeviceSetupData {
    fn default() -> Self {
        Self {
            device_name: String::new(),
            wifi_ssid: String::new(),
            wifi_password: String::new(),
            device_password: String::new(),
            auth_required: true,
            wifi_required: true,
            configured: false,
        }
    }
}

pub struct DeviceSetup {
    pub nvs: EspNvs<NvsDefault>,
    pub state: Mutex<State>,
}

pub struct State {
    pub data: DeviceSetupData,
    pub authenticated: bool,
}

impl DeviceSetup {
    pub fn new(nvs_partition: EspDefaultNvsPartition) -> Result<Arc<Self>, anyhow::Error> {
        let nvs = EspNvs::new(nvs_partition, NVS_NAMESPACE, true)?;

        let mut setup = Self {
            nvs,
            state: Mutex::new(State {
                data: DeviceSetupData::default(),
                authenticated: false,
            }),
        };

        setup.load_from_nvs();
        Ok(Arc::new(setup))
    }

    pub fn is_configured(&self) -> bool {
        self.state.lock().unwrap().data.configured
    }

    pub fn reset_authentication(&self) {
        let mut state = self.state.lock().unwrap();
        state.authenticated = false;
        info!("BLE authentication reset");
    }

    pub fn device_name_for_advertising(&self) -> String {
        let state = self.state.lock().unwrap();
        if !state.data.device_name.is_empty() {
            state.data.device_name.clone()
        } else {
            DEFAULT_DEVICE_NAME.to_string()
        }
    }

    pub fn load_wifi_credentials(&self) -> (String, String) {
        let state = self.state.lock().unwrap();
        (
            state.data.wifi_ssid.clone(),
            state.data.wifi_password.clone(),
        )
    }

    fn load_from_nvs(&mut self) {
        let mut state = self.state.lock().unwrap();
        let mut buf = [0u8; 128];

        state.data.configured =
            self.nvs.get_u8(NVS_KEY_CONFIGURED).unwrap_or(Some(0)) == Some(1);

        state.data.device_name = self
            .nvs
            .get_str(NVS_KEY_NAME, &mut buf)
            .ok()
            .flatten()
            .unwrap_or(DEFAULT_DEVICE_NAME)
            .to_string();

        state.data.wifi_ssid = self
            .nvs
            .get_str(NVS_KEY_WIFI_SSID, &mut buf)
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_string();

        state.data.wifi_password = self
            .nvs
            .get_str(NVS_KEY_WIFI_PASS, &mut buf)
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_string();

        state.data.device_password = self
            .nvs
            .get_str(NVS_KEY_DEV_PASS, &mut buf)
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_string();

        state.data.auth_required =
            self.nvs.get_u8(NVS_KEY_AUTH_REQUIRED).unwrap_or(Some(1)) == Some(1);
        state.data.wifi_required =
            self.nvs.get_u8(NVS_KEY_WIFI_REQUIRED).unwrap_or(Some(1)) == Some(1);

        if state.data.configured
            && ((state.data.wifi_required && state.data.wifi_ssid.is_empty())
                || (state.data.auth_required && state.data.device_password.is_empty()))
        {
            warn!("Incomplete stored configuration; falling back to unconfigured");
            state.data.configured = false;
        }

        info!(
            "Loaded setup: configured={}, name='{}'",
            state.data.configured, state.data.device_name
        );
    }

    pub fn save_to_nvs(nvs: &EspNvs<NvsDefault>, data: &DeviceSetupData) -> Result<(), anyhow::Error> {
        nvs.set_u8(NVS_KEY_CONFIGURED, if data.configured { 1 } else { 0 })?;
        nvs.set_str(NVS_KEY_NAME, &data.device_name)?;
        nvs.set_str(NVS_KEY_WIFI_SSID, &data.wifi_ssid)?;
        nvs.set_str(NVS_KEY_WIFI_PASS, &data.wifi_password)?;
        nvs.set_str(NVS_KEY_DEV_PASS, &data.device_password)?;
        nvs.set_u8(NVS_KEY_AUTH_REQUIRED, if data.auth_required { 1 } else { 0 })?;
        nvs.set_u8(NVS_KEY_WIFI_REQUIRED, if data.wifi_required { 1 } else { 0 })?;
        info!("Setup persisted to NVS");
        Ok(())
    }
}
