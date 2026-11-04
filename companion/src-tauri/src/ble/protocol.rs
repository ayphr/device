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

async fn write_payload(connection: &ActiveBleConnection, payload: &[u8]) -> Result<(), String> {
    let chunk_size = write_chunk_size(connection);
    if payload.is_empty() {
        return Ok(());
    }

    // Frame the request with a 2-byte little-endian length prefix, matching the
    // firmware's reassembly logic, so commands larger than a single BLE write
    // can be fragmented and reassembled correctly.
    let mut framed = Vec::with_capacity(BLE_FRAME_HEADER_LEN + payload.len());
    framed.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    framed.extend_from_slice(payload);

    if framed.len() <= chunk_size {
        connection
            .peripheral
            .write(
                &connection.rx_characteristic,
                &framed,
                btleplug::api::WriteType::WithoutResponse,
            )
            .await
            .map_err(|error| {
                warn!("[ble] write failed: {}", error);
                error.to_string()
            })?;
        return Ok(());
    }
    for chunk in framed.chunks(chunk_size) {
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
        // Accumulate notification bytes across the whole batch so that frames
        // which arrive coalesced in a single notification (or split across
        // several) are reassembled without losing trailing bytes.
        let mut buffer: Vec<u8> = Vec::new();
        let mut responses = Vec::with_capacity(payloads.len());
        for _ in 0..payloads.len() {
            responses.push(
                read_frame(&mut notifications, connection.tx_characteristic.uuid, &mut buffer)
                    .await?,
            );
        }
        Ok::<Vec<Vec<u8>>, String>(responses)
    })
    .await
    .map_err(|_| {
        warn!("[ble] timed out waiting for device response");
        "Timed out waiting for device response".to_string()
    })?
}

async fn read_frame<S>(
    notifications: &mut S,
    tx_uuid: Uuid,
    buffer: &mut Vec<u8>,
) -> Result<Vec<u8>, String>
where
    S: Stream<Item = ValueNotification> + Unpin,
{
    loop {
        // Reassemble a complete frame from the shared buffer, preserving any
        // trailing bytes for subsequent frames.
        if buffer.len() >= BLE_FRAME_HEADER_LEN {
            let payload_len =
                u16::from_le_bytes([buffer[0], buffer[1]]) as usize;
            if payload_len > MAX_BLE_FRAME_PAYLOAD {
                return Err(format!(
                    "BLE response length {payload_len} exceeds max limit of {MAX_BLE_FRAME_PAYLOAD} bytes"
                ));
            }
            let total = BLE_FRAME_HEADER_LEN + payload_len;
            if buffer.len() >= total {
                let payload = buffer[BLE_FRAME_HEADER_LEN..total].to_vec();
                buffer.drain(..total);
                return Ok(payload);
            }
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
        buffer.extend_from_slice(&notification.value);
    }
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
