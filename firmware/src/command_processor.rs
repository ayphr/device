use ayphr_protocol::{
    COMMAND_APPLY_SETUP, COMMAND_AUTHENTICATE, COMMAND_CHANGE_PASSWORD, COMMAND_FACTORY_RESET,
    COMMAND_GET_FIRMWARE_INFO, COMMAND_GET_STATUS, COMMAND_OTA_BEGIN, COMMAND_OTA_DATA,
    COMMAND_OTA_END, COMMAND_RESTART, COMMAND_UPDATE_WIFI, RESPONSE_AUTH_FAILED, RESPONSE_AUTH_OK,
    RESPONSE_CHANGE_PASSWORD_OK, RESPONSE_ERROR, RESPONSE_FACTORY_RESET_OK, RESPONSE_FIRMWARE_INFO,
    RESPONSE_OTA_BEGIN_OK, RESPONSE_OTA_DATA_OK, RESPONSE_OTA_END_OK, RESPONSE_RESTART_OK,
    RESPONSE_SETUP_FAILED, RESPONSE_SETUP_OK, RESPONSE_STATUS, RESPONSE_UPDATE_WIFI_OK,
};
use log::info;

use crate::config::{DeviceSetup, DeviceSetupData};

pub fn process_ble_request(setup: &DeviceSetup, data: &[u8]) -> Vec<u8> {
    process_request(setup, data, false)
}

pub fn process_serial_request(setup: &DeviceSetup, data: &[u8]) -> Vec<u8> {
    process_request(setup, data, true)
}

fn process_request(setup: &DeviceSetup, data: &[u8], bypass_auth: bool) -> Vec<u8> {
    if data.is_empty() {
        return vec![RESPONSE_ERROR];
    }

    let command = data[0];
    let mut cursor = 1;

    match command {
        COMMAND_GET_STATUS => handle_get_status(setup, bypass_auth),
        COMMAND_AUTHENTICATE => handle_authenticate(setup, data, &mut cursor, bypass_auth),
        COMMAND_APPLY_SETUP => handle_apply_setup(setup, data, &mut cursor, bypass_auth),
        COMMAND_RESTART => handle_restart(setup, bypass_auth),
        COMMAND_FACTORY_RESET => handle_factory_reset(setup, bypass_auth),
        COMMAND_CHANGE_PASSWORD => handle_change_password(setup, data, &mut cursor, bypass_auth),
        COMMAND_UPDATE_WIFI => handle_update_wifi(setup, data, &mut cursor, bypass_auth),
        COMMAND_GET_FIRMWARE_INFO => handle_get_firmware_info(),
        COMMAND_OTA_BEGIN => handle_ota_begin(data, &mut cursor),
        COMMAND_OTA_DATA => handle_ota_data(data, &mut cursor),
        COMMAND_OTA_END => handle_ota_end(),
        _ => vec![RESPONSE_ERROR],
    }
}

fn handle_get_status(setup: &DeviceSetup, bypass_auth: bool) -> Vec<u8> {
    let state = setup.state.lock().unwrap();
    let name_bytes = state.data.device_name.as_bytes();
    let name_len = name_bytes.len().min(255) as u8;

    let authenticated =
        !state.data.configured || bypass_auth || !state.data.auth_required || state.authenticated;

    let mut resp = vec![
        RESPONSE_STATUS,
        if state.data.configured { 1 } else { 0 },
        if authenticated { 1 } else { 0 },
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

fn handle_authenticate(
    setup: &DeviceSetup,
    data: &[u8],
    cursor: &mut usize,
    bypass_auth: bool,
) -> Vec<u8> {
    if bypass_auth {
        return vec![RESPONSE_AUTH_OK];
    }

    let pass = match read_field(data, cursor) {
        Some(p) => p,
        None => return vec![RESPONSE_ERROR],
    };

    let mut state = setup.state.lock().unwrap();

    if !state.data.configured || !state.data.auth_required {
        state.authenticated = true;
        return vec![RESPONSE_AUTH_OK];
    }

    let verified = !state.data.device_password.is_empty() && pass == state.data.device_password;
    state.authenticated = verified;
    vec![if verified {
        RESPONSE_AUTH_OK
    } else {
        RESPONSE_AUTH_FAILED
    }]
}

fn handle_apply_setup(
    setup: &DeviceSetup,
    data: &[u8],
    cursor: &mut usize,
    bypass_auth: bool,
) -> Vec<u8> {
    {
        let state = setup.state.lock().unwrap();
        if !bypass_auth && state.data.configured && !state.authenticated {
            return vec![RESPONSE_AUTH_FAILED];
        }
    }

    let dev_name = read_field(data, cursor).unwrap_or_default();
    let wifi_ssid = match read_field(data, cursor) {
        Some(s) => s,
        None => return vec![RESPONSE_SETUP_FAILED],
    };
    let wifi_pass = read_field(data, cursor).unwrap_or_default();
    let dev_pass = read_field(data, cursor).unwrap_or_default();
    let auth_required = data
        .get(*cursor)
        .copied()
        .map(|value| value != 0)
        .unwrap_or(true);
    let skip_wifi = data
        .get(*cursor + 1)
        .copied()
        .map(|value| value != 0)
        .unwrap_or(false);

    if auth_required && dev_pass.is_empty() {
        return vec![RESPONSE_SETUP_FAILED];
    }

    if !skip_wifi && wifi_ssid.is_empty() {
        return vec![RESPONSE_SETUP_FAILED];
    }

    let mut new_data = {
        let state = setup.state.lock().unwrap();
        state.data.clone()
    };

    new_data.wifi_ssid = wifi_ssid;
    new_data.wifi_password = wifi_pass;
    new_data.device_password = dev_pass;
    new_data.auth_required = auth_required;
    new_data.wifi_required = !skip_wifi;
    if !dev_name.is_empty() {
        new_data.device_name = dev_name;
    }
    new_data.configured = true;

    let result = DeviceSetup::save_to_nvs(&setup.nvs, &new_data);

    if result.is_ok() {
        let mut state = setup.state.lock().unwrap();
        state.data = new_data;
        state.authenticated = !bypass_auth;
        vec![RESPONSE_SETUP_OK]
    } else {
        vec![RESPONSE_SETUP_FAILED]
    }
}

fn handle_restart(setup: &DeviceSetup, bypass_auth: bool) -> Vec<u8> {
    {
        let state = setup.state.lock().unwrap();
        if !bypass_auth && state.data.configured && !state.authenticated {
            return vec![RESPONSE_AUTH_FAILED];
        }
    }

    info!("Executing device restart");
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(500));
        esp_idf_svc::hal::reset::restart();
    });
    vec![RESPONSE_RESTART_OK]
}

fn handle_factory_reset(setup: &DeviceSetup, bypass_auth: bool) -> Vec<u8> {
    {
        let state = setup.state.lock().unwrap();
        if !bypass_auth && state.data.configured && !state.authenticated {
            return vec![RESPONSE_AUTH_FAILED];
        }
    }

    let default_data = DeviceSetupData::default();
    let result = DeviceSetup::save_to_nvs(&setup.nvs, &default_data);

    if result.is_ok() {
        let mut state = setup.state.lock().unwrap();
        state.data = default_data;
        state.authenticated = false;
        info!("Factory reset complete");
        vec![RESPONSE_FACTORY_RESET_OK]
    } else {
        vec![RESPONSE_ERROR]
    }
}

fn handle_change_password(
    setup: &DeviceSetup,
    data: &[u8],
    cursor: &mut usize,
    bypass_auth: bool,
) -> Vec<u8> {
    {
        let state = setup.state.lock().unwrap();
        if !bypass_auth && state.data.configured && !state.authenticated {
            return vec![RESPONSE_AUTH_FAILED];
        }
    }

    let current_pass = match read_field(data, cursor) {
        Some(p) => p,
        None => return vec![RESPONSE_ERROR],
    };
    let new_pass = match read_field(data, cursor) {
        Some(p) if !p.is_empty() => p,
        _ => return vec![RESPONSE_ERROR],
    };

    {
        let state = setup.state.lock().unwrap();
        if !bypass_auth
            && state.data.configured
            && state.data.auth_required
            && current_pass != state.data.device_password
        {
            return vec![RESPONSE_AUTH_FAILED];
        }
    }

    let mut new_data = {
        let state = setup.state.lock().unwrap();
        state.data.clone()
    };

    new_data.device_password = new_pass.clone();
    new_data.auth_required = true;

    let result = DeviceSetup::save_to_nvs(&setup.nvs, &new_data);

    if result.is_ok() {
        let mut state = setup.state.lock().unwrap();
        state.data.device_password = new_pass;
        state.data.auth_required = true;
        vec![RESPONSE_CHANGE_PASSWORD_OK]
    } else {
        vec![RESPONSE_ERROR]
    }
}

fn handle_update_wifi(
    setup: &DeviceSetup,
    data: &[u8],
    cursor: &mut usize,
    bypass_auth: bool,
) -> Vec<u8> {
    {
        let state = setup.state.lock().unwrap();
        if !bypass_auth && state.data.configured && !state.authenticated {
            return vec![RESPONSE_AUTH_FAILED];
        }
    }

    let wifi_ssid = match read_field(data, cursor) {
        Some(s) if !s.is_empty() => s,
        _ => return vec![RESPONSE_ERROR],
    };
    let wifi_pass = read_field(data, cursor).unwrap_or_default();

    let mut new_data = {
        let state = setup.state.lock().unwrap();
        state.data.clone()
    };

    new_data.wifi_ssid = wifi_ssid.clone();
    new_data.wifi_password = wifi_pass.clone();
    new_data.wifi_required = true;

    let result = DeviceSetup::save_to_nvs(&setup.nvs, &new_data);

    if result.is_ok() {
        let mut state = setup.state.lock().unwrap();
        state.data.wifi_ssid = wifi_ssid;
        state.data.wifi_password = wifi_pass;
        state.data.wifi_required = true;
        vec![RESPONSE_UPDATE_WIFI_OK]
    } else {
        vec![RESPONSE_ERROR]
    }
}

fn read_field(data: &[u8], cursor: &mut usize) -> Option<String> {
    if *cursor >= data.len() {
        return None;
    }
    let len = data[*cursor] as usize;
    *cursor += 1;
    if *cursor + len > data.len() {
        return None;
    }
    let val = std::str::from_utf8(&data[*cursor..*cursor + len])
        .ok()?
        .to_string();
    *cursor += len;
    Some(val)
}

fn handle_get_firmware_info() -> Vec<u8> {
    let version = ayphr_protocol::FIRMWARE_VERSION;
    let hardware_rev = "rev1";
    let uptime_secs = BOOT_TIME
        .get()
        .map(|boot_time| (std::time::Instant::now() - *boot_time).as_secs() as u32)
        .unwrap_or(0);

    let version_bytes = version.as_bytes();
    let hw_rev_bytes = hardware_rev.as_bytes();

    let mut resp = Vec::with_capacity(2 + version_bytes.len() + hw_rev_bytes.len() + 4);
    resp.push(RESPONSE_FIRMWARE_INFO);
    resp.push(version_bytes.len() as u8);
    resp.extend_from_slice(version_bytes);
    resp.push(hw_rev_bytes.len() as u8);
    resp.extend_from_slice(hw_rev_bytes);
    resp.extend_from_slice(&uptime_secs.to_le_bytes());
    resp
}

fn handle_ota_begin(data: &[u8], cursor: &mut usize) -> Vec<u8> {
    if *cursor + 4 > data.len() {
        return vec![RESPONSE_ERROR];
    }
    let total_size = u32::from_le_bytes([
        data[*cursor],
        data[*cursor + 1],
        data[*cursor + 2],
        data[*cursor + 3],
    ]) as usize;
    *cursor += 4;

    match crate::ota::begin(total_size) {
        Ok(()) => vec![RESPONSE_OTA_BEGIN_OK],
        Err(error) => {
            log::error!("OTA begin failed: {}", error);
            vec![RESPONSE_ERROR]
        }
    }
}

fn handle_ota_data(data: &[u8], cursor: &mut usize) -> Vec<u8> {
    if *cursor + 4 > data.len() {
        return vec![RESPONSE_ERROR];
    }
    *cursor += 4;
    let payload = &data[*cursor..];

    match crate::ota::write_data(payload) {
        Ok(()) => vec![RESPONSE_OTA_DATA_OK],
        Err(error) => {
            log::error!("OTA data write failed: {}", error);
            vec![RESPONSE_ERROR]
        }
    }
}

fn handle_ota_end() -> Vec<u8> {
    match crate::ota::end() {
        Ok(()) => vec![RESPONSE_OTA_END_OK],
        Err(error) => {
            log::error!("OTA end failed: {}", error);
            vec![RESPONSE_ERROR]
        }
    }
}

use std::time::Instant;

static BOOT_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub fn init_boot_time() {
    BOOT_TIME.set(Instant::now()).ok();
}
