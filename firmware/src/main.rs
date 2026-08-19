use ayphr_protocol::{
    COMMAND_APPLY_SETUP, COMMAND_AUTHENTICATE, COMMAND_CHANGE_PASSWORD, COMMAND_FACTORY_RESET,
    COMMAND_GET_STATUS, COMMAND_RESTART, COMMAND_UPDATE_WIFI, RESPONSE_AUTH_FAILED,
    RESPONSE_AUTH_OK, RESPONSE_CHANGE_PASSWORD_OK, RESPONSE_ERROR, RESPONSE_FACTORY_RESET_OK,
    RESPONSE_RESTART_OK, RESPONSE_SETUP_FAILED, RESPONSE_SETUP_OK, RESPONSE_STATUS,
    RESPONSE_UPDATE_WIFI_OK,
};
use esp_idf_svc::nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault};
use log::{info, warn};
use std::sync::{Arc, Mutex};

pub const DEFAULT_DEVICE_NAME: &str = "Unconfigured Geo";
const MAX_FIELD_LEN: usize = 64;

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
            device_name: String::default(),
            wifi_ssid: String::default(),
            wifi_password: String::default(),
            device_password: String::default(),
            auth_required: true,
            wifi_required: true,
            configured: false,
        }
    }
}

pub struct DeviceSetup {
    nvs: EspNvs<NvsDefault>,
    state: Mutex<State>,
}

struct State {
    data: DeviceSetupData,
    authenticated: bool,
}

impl DeviceSetup {
    pub fn new(nvs_partition: EspDefaultNvsPartition) -> Result<Arc<Self>, anyhow::Error> {
        let nvs = EspNvs::new(nvs_partition, "device_setup", true)?;

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

    pub fn wifi_credentials(&self) -> (String, String) {
        let state = self.state.lock().unwrap();
        (state.data.wifi_ssid.clone(), state.data.wifi_password.clone())
    }

    fn load_from_nvs(&mut self) {
        let mut state = self.state.lock().unwrap();
        
        state.data.configured = self.nvs.get_u8("configured").unwrap_or(Some(0)) == Some(1);

        let mut buf_name = [0u8; 128];
        state.data.device_name = self
            .nvs
            .get_str("name", &mut buf_name)
            .ok()
            .flatten()
            .unwrap_or(DEFAULT_DEVICE_NAME)
            .to_string();

        let mut buf_ssid = [0u8; 128];
        state.data.wifi_ssid = self
            .nvs
            .get_str("wifi_ssid", &mut buf_ssid)
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_string();

        let mut buf_wpass = [0u8; 128];
        state.data.wifi_password = self
            .nvs
            .get_str("wifi_pass", &mut buf_wpass)
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_string();

        let mut buf_dpass = [0u8; 128];
        state.data.device_password = self
            .nvs
            .get_str("dev_pass", &mut buf_dpass)
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_string();

        state.data.auth_required = self.nvs.get_u8("auth_req").unwrap_or(Some(1)) == Some(1);
        state.data.wifi_required = self.nvs.get_u8("wifi_req").unwrap_or(Some(1)) == Some(1);

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

    pub fn save_to_nvs_internal(
        nvs: &EspNvs<NvsDefault>,
        data: &DeviceSetupData,
    ) -> Result<(), anyhow::Error> {
        nvs.set_u8("configured", if data.configured { 1 } else { 0 })?;
        nvs.set_str("name", &data.device_name)?;
        nvs.set_str("wifi_ssid", &data.wifi_ssid)?;
        nvs.set_str("wifi_pass", &data.wifi_password)?;
        nvs.set_str("dev_pass", &data.device_password)?;
        nvs.set_u8("auth_req", if data.auth_required { 1 } else { 0 })?;
        nvs.set_u8("wifi_req", if data.wifi_required { 1 } else { 0 })?;
        info!("Setup persisted to NVS");
        Ok(())
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

    pub fn process_request(&self, data: &[u8]) -> Vec<u8> {
        self.process_request_for_transport(data, false)
    }

    pub fn process_serial_request(&self, data: &[u8]) -> Vec<u8> {
        self.process_request_for_transport(data, true)
    }

    fn process_request_for_transport(&self, data: &[u8], bypass_auth: bool) -> Vec<u8> {
        if data.is_empty() {
            return vec![RESPONSE_ERROR];
        }

        let mut state = self.state.lock().unwrap();
        let command = data[0];
        let mut cursor = 1;

        match command {
            COMMAND_GET_STATUS => {
                let name_bytes = state.data.device_name.as_bytes();
                let name_len = name_bytes.len().min(255) as u8;
                let mut resp = vec![
                    RESPONSE_STATUS,
                    if state.data.configured { 1 } else { 0 },
                    if !state.data.configured
                        || bypass_auth
                        || !state.data.auth_required
                        || state.authenticated
                    {
                        1
                    } else {
                        0
                    },
                    if state.data.auth_required { 1 } else { 0 },
                    if state.data.wifi_required { 1 } else { 0 },
                    if !state.data.wifi_ssid.is_empty() {
                        1
                    } else {
                        0
                    },
                    name_len,
                ];
                resp.extend_from_slice(&name_bytes[..name_len as usize]);
                resp
            }
            COMMAND_AUTHENTICATE => {
                if bypass_auth {
                    state.authenticated = false;
                    return vec![RESPONSE_AUTH_OK];
                }

                let pass = match Self::read_field(data, &mut cursor) {
                    Some(p) => p,
                    None => return vec![RESPONSE_ERROR],
                };

                if !state.data.configured || !state.data.auth_required {
                    state.authenticated = true;
                    return vec![RESPONSE_AUTH_OK];
                }

                let verified =
                    !state.data.device_password.is_empty() && pass == state.data.device_password;
                state.authenticated = verified;
                vec![if verified {
                    RESPONSE_AUTH_OK
                } else {
                    RESPONSE_AUTH_FAILED
                }]
            }
            COMMAND_APPLY_SETUP => {
                if !bypass_auth && state.data.configured && !state.authenticated {
                    return vec![RESPONSE_AUTH_FAILED];
                }

                let dev_name = Self::read_field(data, &mut cursor).unwrap_or_default();
                let wifi_ssid = match Self::read_field(data, &mut cursor) {
                    Some(s) => s,
                    None => return vec![RESPONSE_SETUP_FAILED],
                };
                let wifi_pass = Self::read_field(data, &mut cursor).unwrap_or_default();
                let dev_pass = Self::read_field(data, &mut cursor).unwrap_or_default();
                let auth_required = data
                    .get(cursor)
                    .copied()
                    .map(|value| value != 0)
                    .unwrap_or(true);
                let skip_wifi = data
                    .get(cursor + 1)
                    .copied()
                    .map(|value| value != 0)
                    .unwrap_or(false);

                if auth_required && dev_pass.is_empty() {
                    return vec![RESPONSE_SETUP_FAILED];
                }

                if !skip_wifi && wifi_ssid.is_empty() {
                    return vec![RESPONSE_SETUP_FAILED];
                }

                state.data.wifi_ssid = wifi_ssid;
                state.data.wifi_password = wifi_pass;
                state.data.device_password = dev_pass;
                state.data.auth_required = auth_required;
                state.data.wifi_required = !skip_wifi;
                if !dev_name.is_empty() {
                    state.data.device_name = dev_name;
                }
                state.data.configured = true;
                state.authenticated = !bypass_auth;

                if Self::save_to_nvs_internal(&self.nvs, &state.data).is_ok() {
                    vec![RESPONSE_SETUP_OK]
                } else {
                    vec![RESPONSE_SETUP_FAILED]
                }
            }
            COMMAND_RESTART => {
                if !bypass_auth && state.data.configured && !state.authenticated {
                    return vec![RESPONSE_AUTH_FAILED];
                }
                info!("Executing restart via command");
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    esp_idf_svc::hal::reset::restart();
                });
                vec![RESPONSE_RESTART_OK]
            }
            COMMAND_FACTORY_RESET => {
                if !bypass_auth && state.data.configured && !state.authenticated {
                    return vec![RESPONSE_AUTH_FAILED];
                }

                state.data = DeviceSetupData::default();
                state.authenticated = false;

                if Self::save_to_nvs_internal(&self.nvs, &state.data).is_ok() {
                    info!("Factory reset complete");
                    vec![RESPONSE_FACTORY_RESET_OK]
                } else {
                    vec![RESPONSE_ERROR]
                }
            }
            COMMAND_CHANGE_PASSWORD => {
                if !bypass_auth && state.data.configured && !state.authenticated {
                    return vec![RESPONSE_AUTH_FAILED];
                }

                let current_pass = match Self::read_field(data, &mut cursor) {
                    Some(p) => p,
                    None => return vec![RESPONSE_ERROR],
                };
                let new_pass = match Self::read_field(data, &mut cursor) {
                    Some(p) if !p.is_empty() => p,
                    _ => return vec![RESPONSE_ERROR],
                };

                if !bypass_auth
                    && state.data.configured
                    && state.data.auth_required
                    && current_pass != state.data.device_password
                {
                    return vec![RESPONSE_AUTH_FAILED];
                }

                state.data.device_password = new_pass;
                state.data.auth_required = true;
                if Self::save_to_nvs_internal(&self.nvs, &state.data).is_ok() {
                    vec![RESPONSE_CHANGE_PASSWORD_OK]
                } else {
                    vec![RESPONSE_ERROR]
                }
            }
            COMMAND_UPDATE_WIFI => {
                if !bypass_auth && state.data.configured && !state.authenticated {
                    return vec![RESPONSE_AUTH_FAILED];
                }

                let wifi_ssid = match Self::read_field(data, &mut cursor) {
                    Some(s) if !s.is_empty() => s,
                    _ => return vec![RESPONSE_ERROR],
                };
                let wifi_pass = Self::read_field(data, &mut cursor).unwrap_or_default();

                state.data.wifi_ssid = wifi_ssid;
                state.data.wifi_password = wifi_pass;
                state.data.wifi_required = true;

                if Self::save_to_nvs_internal(&self.nvs, &state.data).is_ok() {
                    vec![RESPONSE_UPDATE_WIFI_OK]
                } else {
                    vec![RESPONSE_ERROR]
                }
            }
            _ => vec![RESPONSE_ERROR],
        }
    }

    fn read_field(data: &[u8], cursor: &mut usize) -> Option<String> {
        if *cursor >= data.len() {
            return None;
        }
        let len = data[*cursor] as usize;
        *cursor += 1;
        if len > MAX_FIELD_LEN || *cursor + len > data.len() {
            return None;
        }
        let val = std::str::from_utf8(&data[*cursor..*cursor + len])
            .ok()?
            .to_string();
        *cursor += len;
        Some(val)
    }
}
