# Geo Gen1 Firmware

ESP32 firmware for the Geo device using ESP-IDF and CMake.

## Prerequisites

- Rust installed
- ESPUP installed
  - `cargo install espup`
  - `espup install`

## Getting Started

From this folder:

```bash
cargo +esp build --release --target xtensa-esp32-espidf
```

## Flash and Monitor

```bash
cargo espflash flash --release --target xtensa-esp32-espidf --monitor
```

> Note: This also rebuilds the firmware, but does not run `cargo clean`

## Project Structure

There is shared code between the companion app and this firmware in `../shared`
