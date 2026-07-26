use futures_util::StreamExt;
use btleplug::api::Peripheral as _;
use uuid::Uuid;
use tracing::{debug, warn};

use ayphr_protocol::{
    COMMAND_GET_STATUS, RESPONSE_STATUS,
};
use super::constants::COMMAND_TIMEOUT;
use super::state::{ActiveBleConnection, ParsedStatus};

pub async fn query_status(connection: &ActiveBleConnection) -> Result<ParsedStatus, String> {
    debug!("[ble] querying device status");
    let response = send_command(connection, vec![COMMAND_GET_STATUS])
        .await
        .map_err(|error| {
            warn!("[ble] status query failed: {}", error);
            error
        })?;
    debug!("[ble] status response bytes={}", format_bytes(&response));
    parse_status_response(&response)
}

pub fn parse_status_response(payload: &[u8]) -> Result<ParsedStatus, String> {
    if payload.len() < 5 || payload[0] != RESPONSE_STATUS {
        warn!("[ble] invalid status payload={}", format_bytes(payload));
        return Err("Invalid status response payload".to_string());
    }

    let setup_complete = payload[1] == 1;
    let authenticated = payload[2] == 1;
    let name_length = payload[4] as usize;

    if payload.len() < 5 + name_length {
        warn!("[ble] status response missing device name bytes");
        return Err("Status response is missing device name bytes".to_string());
    }

    let device_name = String::from_utf8(payload[5..5 + name_length].to_vec())
        .map_err(|error| {
            warn!("[ble] device name decode failed: {}", error);
            "Device name is not valid UTF-8".to_string()
        })?;

    debug!(
        "[ble] parsed status setup_complete={} authenticated={} device_name={}",
        setup_complete, authenticated, device_name
    );

    Ok(ParsedStatus {
        setup_complete,
        authenticated,
        device_name,
    })
}

pub async fn send_command(
    connection: &ActiveBleConnection,
    payload: Vec<u8>,
) -> Result<Vec<u8>, String> {
    debug!("[ble] sending command bytes={} tx_uuid={}", format_bytes(&payload), connection.tx_characteristic.uuid);
    let mut notifications = connection
        .peripheral
        .notifications()
        .await
        .map_err(|error| {
            warn!("[ble] failed to start notification stream: {}", error);
            error.to_string()
        })?;

    connection
        .peripheral
        .subscribe(&connection.tx_characteristic)
        .await
        .map_err(|error| {
            warn!("[ble] subscribe failed: {}", error);
            error.to_string()
        })?;

    connection
        .peripheral
        .write(
            &connection.rx_characteristic,
            &payload,
            btleplug::api::WriteType::WithoutResponse,
        )
        .await
        .map_err(|error| {
            warn!("[ble] write failed: {}", error);
            error.to_string()
        })?;

    tokio::time::timeout(COMMAND_TIMEOUT, async {
        while let Some(notification) = notifications.next().await {
            if notification.uuid == connection.tx_characteristic.uuid {
                debug!("[ble] received notification bytes={}", format_bytes(&notification.value));
                return Ok::<Vec<u8>, String>(notification.value);
            }
        }

        warn!("[ble] notification stream ended before response was received");
        Err("Notification stream ended before response was received".to_string())
    })
    .await
    .map_err(|_| {
        warn!("[ble] timed out waiting for device response");
        "Timed out waiting for device response".to_string()
    })?
}

pub fn parse_uuid(value: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|error| format!("Invalid UUID '{value}': {error}"))
}

fn format_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
