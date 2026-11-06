use crate::ble::state::ActiveBleConnection;
use crate::constants::BLE_OTA_WINDOW;
use crate::protocol::{
    parse_firmware_info_response, parse_live_data_response, parse_status_response,
};
use crate::types::{CloudLinkResult, DeviceSettingsResult, ParsedFirmwareInfo, ParsedStatus};
use ayphr_protocol::{
    SensorPacket, COMMAND_CLOUD_LINK_START, COMMAND_CLOUD_LINK_STATUS, COMMAND_CLOUD_UNLINK,
    COMMAND_GET_DEVICE_SETTINGS, COMMAND_GET_FIRMWARE_INFO, COMMAND_GET_LIVE_DATA,
    COMMAND_GET_STATUS, COMMAND_UPDATE_DEVICE_SETTINGS,
};

pub enum Transport {
    Ble(ActiveBleConnection),
    Serial(String),
}

impl Transport {
    pub async fn send_command(&self, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        match self {
            Transport::Ble(conn) => crate::ble::protocol::send_command(conn, payload).await,
            Transport::Serial(id) => {
                crate::serial::protocol::send_command(id.clone(), payload).await
            }
        }
    }

    pub async fn send_commands_batch(
        &self,
        payloads: Vec<Vec<u8>>,
    ) -> Result<Vec<Vec<u8>>, String> {
        match self {
            Transport::Ble(conn) => crate::ble::protocol::send_commands(conn, payloads).await,
            Transport::Serial(_) => {
                let mut responses = Vec::with_capacity(payloads.len());
                for payload in payloads {
                    responses.push(self.send_command(payload).await?);
                }
                Ok(responses)
            }
        }
    }

    pub fn ota_window(&self) -> usize {
        match self {
            Transport::Ble(_) => BLE_OTA_WINDOW,
            Transport::Serial(_) => 1,
        }
    }

    pub async fn query_status(&self) -> Result<ParsedStatus, String> {
        let response = self.send_command(vec![COMMAND_GET_STATUS]).await?;
        parse_status_response(&response)
    }

    pub async fn query_firmware_info(&self) -> Result<ParsedFirmwareInfo, String> {
        let response = self.send_command(vec![COMMAND_GET_FIRMWARE_INFO]).await?;
        parse_firmware_info_response(&response)
    }

    pub async fn query_live_data(&self) -> Result<Option<SensorPacket>, String> {
        let response = self.send_command(vec![COMMAND_GET_LIVE_DATA]).await?;
        parse_live_data_response(&response)
    }

    pub async fn query_device_settings(&self) -> Result<DeviceSettingsResult, String> {
        let response = self.send_command(vec![COMMAND_GET_DEVICE_SETTINGS]).await?;
        crate::protocol::parse_device_settings_response(&response)
    }

    pub async fn update_device_settings(
        &self,
        sampling_interval_secs: u32,
        upload_interval_secs: u32,
    ) -> Result<DeviceSettingsResult, String> {
        let mut command = vec![COMMAND_UPDATE_DEVICE_SETTINGS];
        command.extend_from_slice(&sampling_interval_secs.to_le_bytes());
        command.extend_from_slice(&upload_interval_secs.to_le_bytes());
        let response = self.send_command(command).await?;
        if response.first().copied() != Some(ayphr_protocol::RESPONSE_DEVICE_SETTINGS_OK) {
            return Err("Device rejected settings update".to_string());
        }
        self.query_device_settings().await
    }

    pub async fn start_cloud_link(&self) -> Result<CloudLinkResult, String> {
        let response = self.send_command(vec![COMMAND_CLOUD_LINK_START]).await?;
        crate::protocol::parse_cloud_link_response(&response)
    }

    pub async fn query_cloud_link(&self) -> Result<CloudLinkResult, String> {
        let response = self.send_command(vec![COMMAND_CLOUD_LINK_STATUS]).await?;
        crate::protocol::parse_cloud_link_response(&response)
    }

    pub async fn unlink_cloud(&self) -> Result<(), String> {
        let response = self.send_command(vec![COMMAND_CLOUD_UNLINK]).await?;
        if response.first().copied() != Some(ayphr_protocol::RESPONSE_CLOUD_UNLINK_OK) {
            return Err("Device rejected cloud unlink request".to_string());
        }
        Ok(())
    }
}
