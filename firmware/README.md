# Geo Gen1 Firmware

ESP32-C3 (RISC-V) firmware for the Geo device.

## Prerequisites

- Rust installed
- ESPUP installed
  - `cargo install espup`
  - `espup install -t esp32c3`

## Getting Started

From this folder:

```bash
cargo +esp build --release --target riscv32imc-esp-espidf
```

## Flash and Monitor

```bash
cargo espflash flash --release --target riscv32imc-esp-espidf --monitor
```

> Note: This also rebuilds the firmware, but does not run `cargo clean`

## Project Structure

There is shared code between the companion app and this firmware in `../shared`
