use ayphr_protocol::{MAX_TELEMETRY_BUFFER_POINTS, MAX_TELEMETRY_POINTS_PER_UPLOAD};
use log::{info, warn};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::DeviceSetup;

#[derive(Clone, Copy)]
pub struct TelemetryPoint {
    pub timestamp_ms: u64,
    pub temperature: f32,
    pub humidity: f32,
    pub air_pressure: f32,
}

static BUFFER: Mutex<Option<VecDeque<TelemetryPoint>>> = Mutex::new(None);

fn lock_buffer<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            warn!("Telemetry buffer recovered from poisoning");
            poisoned.into_inner()
        }
    }
}

/// Adds a sample to the bounded upload buffer. Oldest points are dropped once
/// the buffer is full.
pub fn record(point: TelemetryPoint) {
    let mut guard = lock_buffer(&BUFFER);
    let buffer = guard.get_or_insert_with(VecDeque::new);
    if buffer.len() >= MAX_TELEMETRY_BUFFER_POINTS {
        buffer.pop_front();
    }
    buffer.push_back(point);
}

pub fn pending_len() -> usize {
    let guard = lock_buffer(&BUFFER);
    guard.as_ref().map_or(0, VecDeque::len)
}

pub fn clear() {
    if let Some(buffer) = lock_buffer(&BUFFER).as_mut() {
        buffer.clear();
    }
}

fn drain_batch(max_points: usize) -> Vec<TelemetryPoint> {
    let mut guard = lock_buffer(&BUFFER);
    let Some(buffer) = guard.as_mut() else {
        return Vec::new();
    };

    let take = buffer.len().min(max_points);
    buffer.drain(..take).collect()
}

fn requeue(points: Vec<TelemetryPoint>) {
    if points.is_empty() {
        return;
    }

    let mut guard = lock_buffer(&BUFFER);
    let buffer = guard.get_or_insert_with(VecDeque::new);
    for point in points.into_iter().rev() {
        buffer.push_front(point);
    }
    while buffer.len() > MAX_TELEMETRY_BUFFER_POINTS {
        buffer.pop_front();
    }
}

/// Starts the background upload loop. It only sends data while a cloud token
/// is configured and Wi-Fi is connected.
pub fn start(setup: Arc<DeviceSetup>) {
    std::thread::spawn(move || run(setup));
}

fn run(setup: Arc<DeviceSetup>) {
    let mut last_upload: Option<Instant> = None;
    let mut retry_at: Option<Instant> = None;

    loop {
        std::thread::sleep(Duration::from_secs(1));

        let (token, upload_interval_secs) = {
            let state = setup.lock_state();
            (
                state.data.cloud_token.clone(),
                state.data.upload_interval_secs,
            )
        };

        if token.is_empty() || !crate::wifi::is_connected() {
            continue;
        }

        let pending = pending_len();
        if pending == 0 {
            continue;
        }

        let interval = Duration::from_secs(upload_interval_secs as u64);
        let retry_due = retry_at.map_or(false, |retry| retry <= Instant::now());
        let due = pending >= MAX_TELEMETRY_POINTS_PER_UPLOAD
            || retry_due
            || last_upload.map_or(true, |last| last.elapsed() >= interval);

        if !due {
            continue;
        }

        let batch = drain_batch(MAX_TELEMETRY_POINTS_PER_UPLOAD);
        if batch.is_empty() {
            continue;
        }

        match crate::cloud::upload_points(&token, &batch) {
            Ok(()) => {
                last_upload = Some(Instant::now());
                retry_at = None;
                info!(
                    "Uploaded {} telemetry points ({} still buffered)",
                    batch.len(),
                    pending_len()
                );
            }
            Err(error) => {
                warn!("Telemetry upload failed: {}; retrying in 15s", error);
                retry_at = Some(Instant::now() + Duration::from_secs(15));
                requeue(batch);
            }
        }
    }
}
