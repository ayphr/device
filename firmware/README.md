# Geo Gen1 Firmware

ESP32-C3 (RISC-V) firmware for the Geo device using ESP-IDF and CMake.

## Prerequisites

- Rust installed
- ESPUP installed
  - `cargo install espup`
  - `espup install -t esp32c3`

> Note: The ESP32-C3 uses the RISC-V architecture. The `espup` install must
> include the `esp32c3` target so the `riscv32imc-esp-espidf` Rust target and
> RISC-V toolchains are available. To build for the classic ESP32 as well, run
> `espup install` without arguments (installs both Xtensa and RISC-V targets).

## Getting Started

From this folder:

```bash
cargo +esp build --release
```

The default target is `riscv32imc-esp-espidf` (ESP32-C3, RISC-V). The chip is
auto-detected from the target triple.

## Local Dev: Classic ESP32 (Xtensa)

For local development only, you can also build for the original ESP32:

```bash
cargo +esp build --release --target xtensa-esp32-espidf
```

## Flash and Monitor

```bash
cargo espflash flash --monitor
```

> Note: This also rebuilds the firmware, but does not run `cargo clean`

## Pin Mapping (ESP32-C3 Mini 1)

| Function | Pin |
|---|---|
| Serial TX (UART1) | GPIO5 |
| Serial RX (UART1) | GPIO4 |
| I2C SDA | GPIO0 |
| I2C SCL | GPIO1 |

> When building for the classic ESP32 (Xtensa), pins in `src/main.rs` map to
> the original layout: Serial TX=GPIO17, Serial RX=GPIO16, I2C SDA=GPIO21,
> I2C SCL=GPIO22.

## Project Structure

There is shared code between the companion app and this firmware in `../shared`