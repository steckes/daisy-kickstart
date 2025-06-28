# Daisy Seed Kickstart

Figure out which board you have:
- Daisy Seed (codec AK4556), seed
- Daisy Seed 1.1 (codec WM8731), seed_1_1
- Daisy Seed 1.2 (codec PCM3060), seed_1_2
- Daisy Patch SM (codec PCM3060), patch_sm

then change the feature in the `Cargo.toml` to your board.

```toml
daisy = { version = "0.11", features = ["seed_1_1"] }
```

## Flash Firmware

### With ST-Link Mini Debug Probe

```sh
cargo run --release --bin firmware
```

### With dfu-util

```sh
cargo objcopy --release --bin firmware -- -O binary target/program.bin
dfu-util -a 0 -s 0x08000000:leave -D target/program.bin -d ,0483:df11
```

## Run Benchmark

```sh
cargo run --bin benchmark
```
## Environment Setup Fedora

```sh
sudo dnf install libusbx-devel libftdi-devel libudev-devel
# Install probe-rs
curl -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh
# Flip-link helps with stack overflow
cargo install flip-link
```

### Enable USB device access

```sh
sudo nano /etc/udev/rules.d/50-stm32-dfu.rules
```

### Add this content

```
# STM32 DFU Device (Daisy Seed)
SUBSYSTEMS=="usb", ATTRS{idVendor}=="0483", ATTRS{idProduct}=="df11", MODE="0666", GROUP="plugdev"
# ST-Link Mini v3
SUBSYSTEMS=="usb", ATTRS{idVendor}=="0483", ATTRS{idProduct}=="3754", MODE:="0666", SYMLINK+="stlinkv3_%n"
```

### Add user to groups

```sh
# Add your user to dialout and plugdev groups
sudo usermod -a -G plugdev $USER

# Check if groups exist, create if needed
getent group plugdev || sudo groupadd plugdev
```

### Reload user groups

```sh
# Reload udev rules
sudo udevadm control --reload-rules
sudo udevadm trigger

# Or restart udev service
sudo systemctl restart systemd-udevd
```
