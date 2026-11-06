use ayphr_protocol::{
    CLOUD_STATE_ERROR, CLOUD_STATE_LINKED, CLOUD_STATE_PENDING, CLOUD_STATE_REQUESTING,
    CLOUD_STATE_UNLINKED, RESPONSE_CLOUD_LINK,
};
use core::fmt::Write as _;
use esp_idf_svc::http::client::{Configuration as HttpConfiguration, EspHttpConnection};
use esp_idf_svc::http::Method;
use log::{info, warn};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::clock;
use crate::config::DeviceSetup;
use crate::telemetry::{self, TelemetryPoint};

const API_BASE_URL: &str = "https://api.ayphr.com";
const DEFAULT_PAIRING_CODE_TTL_MS: u64 = 5 * 60 * 1000;
const HTTP_TIMEOUT_SECS: u64 = 20;
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Clone)]
enum LinkStatus {
    Unlinked,
    Requesting,
    Pending { code: String, expires_at_ms: u64 },
    Linked,
    Error(String),
}

static STATUS: Mutex<Option<LinkStatus>> = Mutex::new(None);
static POLLING: AtomicBool = AtomicBool::new(false);

fn lock_status() -> std::sync::MutexGuard<'static, Option<LinkStatus>> {
    match STATUS.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("Cloud status recovered from poisoning");
            poisoned.into_inner()
        }
    }
}

fn set_status(status: LinkStatus) {
    *lock_status() = Some(status);
}

fn current_status(setup: &DeviceSetup) -> LinkStatus {
    let linked = setup.lock_state().data.cloud_linked();
    let mut guard = lock_status();

    match guard.as_ref() {
        Some(LinkStatus::Linked) if linked => LinkStatus::Linked,
        Some(LinkStatus::Requesting) => LinkStatus::Requesting,
        Some(LinkStatus::Pending {
            code,
            expires_at_ms,
        }) => LinkStatus::Pending {
            code: code.clone(),
            expires_at_ms: *expires_at_ms,
        },
        Some(LinkStatus::Error(message)) => LinkStatus::Error(message.clone()),
        _ => {
            if linked {
                *guard = Some(LinkStatus::Linked);
                LinkStatus::Linked
            } else {
                *guard = Some(LinkStatus::Unlinked);
                LinkStatus::Unlinked
            }
        }
    }
}

pub fn init(setup: &DeviceSetup) {
    if setup.lock_state().data.cloud_linked() {
        set_status(LinkStatus::Linked);
    } else {
        set_status(LinkStatus::Unlinked);
    }
}

/// Starts a pairing request in the background. Returns immediately; callers
/// poll the status command for the generated code and link progress.
pub fn start_link(setup: Arc<DeviceSetup>) {
    let status = current_status(&setup);
    if !matches!(status, LinkStatus::Unlinked | LinkStatus::Error(_)) {
        return;
    }

    set_status(LinkStatus::Requesting);
    std::thread::spawn(move || request_pairing(setup));
}

/// Kicks off a non-blocking poll of the pairing endpoint when a code is
/// pending. Called by the status command; it never blocks the command thread.
pub fn refresh_link(setup: Arc<DeviceSetup>) {
    let status = current_status(&setup);
    let LinkStatus::Pending {
        code,
        expires_at_ms,
    } = status
    else {
        return;
    };

    let now = clock::now_unix_ms().unwrap_or(0);
    if now == 0 {
        return;
    }

    if now >= expires_at_ms {
        warn!("Cloud pairing code expired");
        set_status(LinkStatus::Unlinked);
        return;
    }

    if POLLING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        let path = format!("/api/devices/pair/{code}");
        match request(Method::Get, &path, None) {
            Ok((200, body)) => match json_string_field(&body, "token") {
                Some(token) if !token.is_empty() => match store_token(&setup, &token) {
                    Ok(()) => {
                        info!("Device linked to Ayphr Cloud");
                        set_status(LinkStatus::Linked);
                    }
                    Err(error) => {
                        warn!("Failed to store cloud token: {}", error);
                        set_status(LinkStatus::Error(
                            "Could not save the cloud credentials".to_string(),
                        ));
                    }
                },
                _ => {
                    warn!("Pairing response did not include a token");
                    set_status(LinkStatus::Error(
                        "Cloud returned an invalid link response".to_string(),
                    ));
                }
            },
            // 404 means the code has not been claimed on the website yet.
            Ok((404, _)) => {}
            Ok((status, body)) => {
                warn!(
                    "Pairing status request failed (HTTP {}): {}",
                    status,
                    truncate(&body)
                );
            }
            Err(error) => warn!("Pairing status request failed: {}", error),
        }
        POLLING.store(false, Ordering::SeqCst);
    });
}

fn request_pairing(setup: Arc<DeviceSetup>) {
    // The device may still be bringing Wi-Fi up after setup.
    for _ in 0..60 {
        if crate::wifi::is_connected() {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    if !crate::wifi::is_connected() {
        set_status(LinkStatus::Error(
            "Wi-Fi is not connected. Connect the device to a network and try again.".to_string(),
        ));
        return;
    }

    let (serial, name) = {
        let state = setup.lock_state();
        (crate::identity::serial(), state.data.device_name.clone())
    };
    let name = if name.is_empty() {
        format!("Device {}", serial)
    } else {
        name
    };

    let body = format!(
        "{{\"serial\":{},\"name\":\"{}\"}}",
        serial,
        json_escape(&name)
    );

    match request(Method::Post, "/api/devices/pair", Some(&body)) {
        Ok((201, response)) => {
            let Some(code) = json_string_field(&response, "code") else {
                set_status(LinkStatus::Error(
                    "Cloud did not return a pairing code".to_string(),
                ));
                return;
            };
            let expires_at_ms = json_string_field(&response, "expiresAt")
                .and_then(|value| parse_iso8601_ms(&value))
                .unwrap_or_else(|| clock::now_unix_ms().unwrap_or(0) + DEFAULT_PAIRING_CODE_TTL_MS);

            info!("Cloud pairing code generated: {}", code);
            set_status(LinkStatus::Pending {
                code,
                expires_at_ms,
            });
        }
        Ok((status, body)) => {
            warn!(
                "Pairing request failed (HTTP {}): {}",
                status,
                truncate(&body)
            );
            set_status(LinkStatus::Error(format!(
                "Cloud pairing failed (HTTP {})",
                status
            )));
        }
        Err(error) => {
            warn!("Pairing request failed: {}", error);
            set_status(LinkStatus::Error(
                "Could not reach Ayphr Cloud. Check the connection and try again.".to_string(),
            ));
        }
    }
}

/// Clears the locally stored token and buffered telemetry. The device also
/// needs to be removed from the account on the Ayphr website.
pub fn unlink(setup: &DeviceSetup) -> Result<(), String> {
    let mut new_data = setup.lock_state().data.clone();
    new_data.cloud_token.clear();

    DeviceSetup::save_to_nvs(&setup.nvs, &new_data).map_err(|error| error.to_string())?;
    setup.lock_state().data.cloud_token.clear();
    telemetry::clear();
    set_status(LinkStatus::Unlinked);
    info!("Device unlinked from Ayphr Cloud");
    Ok(())
}

pub fn reset() {
    telemetry::clear();
    set_status(LinkStatus::Unlinked);
}

fn store_token(setup: &DeviceSetup, token: &str) -> Result<(), String> {
    let mut new_data = setup.lock_state().data.clone();
    new_data.cloud_token = token.to_string();

    DeviceSetup::save_to_nvs(&setup.nvs, &new_data).map_err(|error| error.to_string())?;
    setup.lock_state().data.cloud_token = token.to_string();
    Ok(())
}

/// Builds the response payload for the cloud link status command.
pub fn status_response(setup: &DeviceSetup) -> Vec<u8> {
    let status = current_status(setup);

    let (state, code, expires_in_secs, message) = match status {
        LinkStatus::Unlinked => (CLOUD_STATE_UNLINKED, String::new(), 0, String::new()),
        LinkStatus::Requesting => (CLOUD_STATE_REQUESTING, String::new(), 0, String::new()),
        LinkStatus::Pending {
            code,
            expires_at_ms,
        } => {
            let now = clock::now_unix_ms().unwrap_or(0);
            let remaining = expires_at_ms.saturating_sub(now) / 1000;
            (
                CLOUD_STATE_PENDING,
                code,
                remaining.min(u32::MAX as u64) as u32,
                String::new(),
            )
        }
        LinkStatus::Linked => (CLOUD_STATE_LINKED, String::new(), 0, String::new()),
        LinkStatus::Error(message) => (CLOUD_STATE_ERROR, String::new(), 0, message),
    };

    let mut response = vec![RESPONSE_CLOUD_LINK, state];
    push_string(&mut response, &code);
    response.extend_from_slice(&expires_in_secs.to_le_bytes());
    push_string(&mut response, &message);
    response
}

fn push_string(buffer: &mut Vec<u8>, value: &str) {
    let bytes = value.as_bytes();
    let len = bytes.len().min(u8::MAX as usize);
    buffer.push(len as u8);
    buffer.extend_from_slice(&bytes[..len]);
}

pub fn upload_points(token: &str, points: &[TelemetryPoint]) -> Result<(), String> {
    let mut body = String::with_capacity(64 + points.len() * 96);
    body.push_str("{\"token\":\"");
    body.push_str(&json_escape(token));
    body.push_str("\",\"data\":[");

    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        let _ = write!(
            body,
            "{{\"timestamp\":{},\"temperature\":{:.2},\"humidity\":{:.2},\"airPressure\":{:.2}}}",
            point.timestamp_ms, point.temperature, point.humidity, point.air_pressure
        );
    }

    body.push_str("]}");

    let (status, response) = request(Method::Post, "/api/devices/data", Some(&body))?;
    if status == 200 {
        Ok(())
    } else {
        Err(format!(
            "cloud upload failed (HTTP {}): {}",
            status,
            truncate(&response)
        ))
    }
}

fn request(method: Method, path: &str, body: Option<&str>) -> Result<(u16, String), String> {
    let config = HttpConfiguration {
        buffer_size: Some(4096),
        buffer_size_tx: Some(4096),
        timeout: Some(Duration::from_secs(HTTP_TIMEOUT_SECS)),
        crt_bundle_attach: Some(esp_idf_svc::sys::esp_crt_bundle_attach),
        ..Default::default()
    };

    let mut connection =
        EspHttpConnection::new(&config).map_err(|error| format!("HTTP init failed: {error}"))?;

    let url = format!("{}{}", API_BASE_URL, path);
    let content_length = body.map(|value| value.len()).unwrap_or(0).to_string();

    let mut headers: Vec<(&str, &str)> = vec![("accept", "application/json")];
    if body.is_some() {
        headers.push(("content-type", "application/json"));
        headers.push(("content-length", content_length.as_str()));
    }

    connection
        .initiate_request(method, &url, &headers)
        .map_err(|error| format!("HTTP request failed: {error}"))?;

    if let Some(body) = body {
        connection
            .write_all(body.as_bytes())
            .map_err(|error| format!("HTTP write failed: {error}"))?;
    }

    connection
        .initiate_response()
        .map_err(|error| format!("HTTP response failed: {error}"))?;

    let status = connection.status();
    let mut response = Vec::new();
    let mut chunk = [0u8; 512];

    loop {
        match connection.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                response.extend_from_slice(&chunk[..read]);
                if response.len() >= MAX_RESPONSE_BYTES {
                    break;
                }
            }
            Err(error) => return Err(format!("HTTP read failed: {error}")),
        }
    }

    Ok((status, String::from_utf8_lossy(&response).into_owned()))
}

fn truncate(value: &str) -> String {
    value.chars().take(200).collect()
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => escaped.push(' '),
            character => escaped.push(character),
        }
    }
    escaped
}

fn json_string_field(body: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = body.find(&pattern)? + pattern.len();
    let rest = &body[start..];
    let colon = rest.find(':')?;
    let rest = rest[colon + 1..].trim_start();
    let rest = rest.strip_prefix('"')?;

    let mut value = String::new();
    let mut chars = rest.chars();
    while let Some(character) = chars.next() {
        match character {
            '"' => return Some(value),
            '\\' => match chars.next()? {
                'n' => value.push('\n'),
                'r' => value.push('\r'),
                't' => value.push('\t'),
                'u' => {
                    for _ in 0..4 {
                        chars.next();
                    }
                    value.push('?');
                }
                other => value.push(other),
            },
            character => value.push(character),
        }
    }

    None
}

/// Parses an ISO 8601 timestamp (for example `2026-11-06T12:34:56.789Z`)
/// into Unix milliseconds.
fn parse_iso8601_ms(value: &str) -> Option<u64> {
    let value = value.trim();
    let year: i64 = value.get(0..4)?.parse().ok()?;
    let month: i64 = value.get(5..7)?.parse().ok()?;
    let day: i64 = value.get(8..10)?.parse().ok()?;
    let hour: i64 = value.get(11..13)?.parse().ok()?;
    let minute: i64 = value.get(14..16)?.parse().ok()?;
    let second: i64 = value.get(17..19)?.parse().ok()?;
    let millis: i64 = if value.as_bytes().get(19) == Some(&b'.') {
        value
            .get(20..23)
            .and_then(|part| part.parse().ok())
            .unwrap_or(0)
    } else {
        0
    };

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    // Howard Hinnant's days-from-civil algorithm.
    let adjusted_year = year - if month <= 2 { 1 } else { 0 };
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let month_prime = (month + 9) % 12;
    let day_of_year = (153 * month_prime + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days_since_epoch = era * 146_097 + day_of_era - 719_468;

    let seconds = days_since_epoch * 86_400 + hour * 3_600 + minute * 60 + second;
    if seconds < 0 {
        return None;
    }

    Some(seconds as u64 * 1000 + millis as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_timestamp() {
        assert_eq!(
            parse_iso8601_ms("2026-11-06T00:00:00.000Z"),
            Some(1_793_923_200_000)
        );
        assert_eq!(parse_iso8601_ms("1970-01-01T00:00:01.500Z"), Some(1_500));
    }

    #[test]
    fn extracts_json_strings() {
        let body = r#"{"code":"ABC123","expiresAt":"2026-11-06T12:34:56.789Z"}"#;
        assert_eq!(json_string_field(body, "code").as_deref(), Some("ABC123"));
        assert_eq!(
            json_string_field(body, "expiresAt").as_deref(),
            Some("2026-11-06T12:34:56.789Z")
        );
    }

    #[test]
    fn escapes_json_strings() {
        assert_eq!(json_escape("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    }
}
