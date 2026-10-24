use btleplug::api::{Peripheral as _, ValueNotification};
use futures_util::{Stream, StreamExt};
use tracing::{debug, warn};
use uuid::Uuid;

use super::state::ActiveBleConnection;
use crate::constants::COMMAND_TIMEOUT;
use crate::protocol::format_bytes;
use ayphr_protocol::{BLE_FRAME_HEADER_LEN, MAX_BLE_FRAME_PAYLOAD};

const STALE_DRAIN_GRACE: std::time::Duration = std::time::Duration::from_millis(5);
const DEFAULT_BLE_WRITE_CHUNK: usize = 180;

fn write_chunk_size(connection: &ActiveBleConnection) -> usize {
    let mtu = connection.peripheral.mtu();
    if mtu == 0 {
        return DEFAULT_BLE_WRITE_CHUNK;
    }
    // ATT write payload max is MTU-3; leave small margin
    ((mtu as usize).saturating_sub(4)).max(20)
}

async fn write_payload(
    connection: &ActiveBleConnection,
    payload: &[u8],
) -> Result<(), String> {
    let chunk_size = write_chunk_size(connection);
    if payload.is_empty() {
        return Ok(());
    }
    if payload.len() <= chunk_size {
        connection
            .peripheral
            .write(
                &connection.rx_characteristic,
                payload,
                btleplug::api::WriteType::WithoutResponse,
            )
            .await
            .map_err(|error| {
                warn!("[ble] write failed: {}", error);
                error.to_string()
            })?;
        return Ok(());
    }
    for chunk in payload.chunks(chunk_size) {
        connection
            .peripheral
            .write(
                &connection.rx_characteristic,
                chunk,
                btleplug::api::WriteType::WithoutResponse,
            )
            .await
            .map_err(|error| {
                warn!("[ble] write failed: {}", error);
                error.to_string()
            })?;
    }
    Ok(())
}

pub async fn send_command(
    connection: &ActiveBleConnection,
    payload: Vec<u8>,
) -> Result<Vec<u8>, String> {
    let mut responses = send_commands(connection, vec![payload]).await?;
    responses
        .pop()
        .ok_or_else(|| "Device produced no response".to_string())
}

pub async fn send_commands(
    connection: &ActiveBleConnection,
    payloads: Vec<Vec<u8>>,
) -> Result<Vec<Vec<u8>>, String> {
    let _guard = connection.command_lock.lock().await;

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

    drain_stale_notifications(&mut notifications).await;

    for payload in &payloads {
        debug!(
            "[ble] sending command bytes={} tx_uuid={}",
            format_bytes(payload),
            connection.tx_characteristic.uuid
        );
        write_payload(connection, payload).await?;
    }

    tokio::time::timeout(COMMAND_TIMEOUT, async {
        let mut responses = Vec::with_capacity(payloads.len());
        for _ in 0..payloads.len() {
            responses
                .push(read_frame(&mut notifications, connection.tx_characteristic.uuid).await?);
        }
        Ok::<Vec<Vec<u8>>, String>(responses)
    })
    .await
    .map_err(|_| {
        warn!("[ble] timed out waiting for device response");
        "Timed out waiting for device response".to_string()
    })?
}

async fn read_frame<S>(notifications: &mut S, tx_uuid: Uuid) -> Result<Vec<u8>, String>
where
    S: Stream<Item = ValueNotification> + Unpin,
{
    let mut collected: Vec<u8> = Vec::new();
    let mut frame_len: Option<usize> = None;

    loop {
        let complete = match frame_len {
            Some(total) => collected.len() >= total,
            None => collected.len() >= BLE_FRAME_HEADER_LEN,
        };
        if complete {
            break;
        }

        let notification = match notifications.next().await {
            Some(notification) => notification,
            None => {
                warn!("[ble] notification stream ended before response was received");
                return Err("Notification stream ended before response was received".to_string());
            }
        };
        if notification.uuid != tx_uuid {
            continue;
        }
        debug!(
            "[ble] received notification bytes={}",
            format_bytes(&notification.value)
        );
        collected.extend_from_slice(&notification.value);

        if frame_len.is_none() && collected.len() >= BLE_FRAME_HEADER_LEN {
            let payload_len = u16::from_le_bytes([collected[0], collected[1]]) as usize;
            if payload_len > MAX_BLE_FRAME_PAYLOAD {
                return Err(format!(
                    "BLE response length {payload_len} exceeds max limit of {MAX_BLE_FRAME_PAYLOAD} bytes"
                ));
            }
            frame_len = Some(BLE_FRAME_HEADER_LEN + payload_len);
        }
    }

    let total = frame_len.unwrap_or(BLE_FRAME_HEADER_LEN);
    Ok(collected[BLE_FRAME_HEADER_LEN..total].to_vec())
}

async fn drain_stale_notifications<S>(notifications: &mut S)
where
    S: Stream<Item = ValueNotification> + Unpin,
{
    while let Ok(Some(notification)) =
        tokio::time::timeout(STALE_DRAIN_GRACE, notifications.next()).await
    {
        debug!(
            "[ble] discarding stale notification bytes={}",
            format_bytes(&notification.value)
        );
    }
}

pub fn parse_uuid(value: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|error| format!("Invalid UUID '{value}': {error}"))
}
