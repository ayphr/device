use crate::ble::state::ActiveBleConnection;
use crate::protocol::{parse_firmware_info_response, parse_status_response};
use crate::types::{ParsedFirmwareInfo, ParsedStatus};
use ayphr_protocol::{COMMAND_GET_FIRMWARE_INFO, COMMAND_GET_STATUS};

pub enum Transport {
    Ble(ActiveBleConnection),
    Serial(String),
}

impl Transport {
    pub async fn send_command(&self, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        match self {
            Transport::Ble(conn) => crate::ble::protocol::send_command(conn, payload).await,
            Transport::Serial(id) => crate::serial::protocol::send_command(id, payload),
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
}
