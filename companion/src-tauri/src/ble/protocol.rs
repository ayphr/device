use futures_util::StreamExt;
use btleplug::api::Peripheral as _;
use uuid::Uuid;
use tracing::{debug, warn};

use super::state::ActiveBleConnection;
use crate::constants::COMMAND_TIMEOUT;
use crate::protocol::format_bytes;

pub async fn send_command(
    connection: &ActiveBleConnection,
    payload: Vec<u8>,
) -> Result<Vec<u8>, String> {
    debug!(
        "[ble] sending command bytes={} tx_uuid={}",
        format_bytes(&payload),
        connection.tx_characteristic.uuid
    );
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
                debug!(
                    "[ble] received notification bytes={}",
                    format_bytes(&notification.value)
                );
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
