# enc-video

A Rust library for enumerating, activating, and capturing video devices and desktop frames on Windows using Media Foundation and DirectX.

## Features

- Enumerate all connected video devices (e.g., webcams) on your Windows system.
- Retrieve friendly names for video devices.
- Activate video devices and capture frames in various formats (NV12, RGB32).
- Capture monitor/desktop frames using DirectX Desktop Duplication.
- Asynchronous frame capture using Tokio and MPSC channels.

## Requirements

- Windows 10 or later
- Rust (edition 2021 or newer)
- [windows](https://crates.io/crates/windows) crate

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
windows = "0.48"
tokio = { version = "1", features = ["full"] }
```

### Enumerate Video Devices

```rust
use enc_video::devices::{VideoDevices, get_device_name};

unsafe {
    let devices = VideoDevices::new().unwrap();
    for device in &devices.devices {
        let name = get_device_name(*device).unwrap();
        println!("Device: {}", name);
    }
    devices.free_devices();
}
```

### Activate and Capture from a Video Device

```rust
use enc_video::devices::VideoDevices;

#[tokio::main]
async fn main() {
    unsafe {
        let devices = VideoDevices::new().unwrap();
        let activated = devices.activate_device(devices.devices[0], None).unwrap();
        let receiver = activated.receiver.clone();

        tokio::spawn(async move {
            let mut rx = receiver.lock().await;
            while let Some(frame) = rx.recv().await {
                println!("Received frame of size: {}", frame.len());
                break;
            }
        });

        activated.start_capturing().await.unwrap();
        devices.free_devices();
    }
}
```

### Capture Desktop Frames

```rust
use enc_video::monitor::Monitor;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    unsafe {
        let monitor = Arc::new(Monitor::from_monitor(0).unwrap());
        let receiver = monitor.receiver.clone();

        tokio::spawn(async move {
            let mut rx = receiver.lock().await;
            while let Some(frame) = rx.recv().await {
                println!("Received desktop frame of size: {}", frame.len());
                break;
            }
        });

        monitor.start_cloning().await.unwrap();
    }
}
```

## Tests

Run tests with:

```sh
cargo test
```

## Project Structure

- `src/devices.rs`: Device management module.
- `src/devices/video_devices.rs`: Video device enumeration and activation.
- `src/devices/activated_device.rs`: Activated device and frame capture logic.
- `src/devices/device_size.rs`: Device size utilities.
- `src/monitor.rs`: Desktop duplication and monitor frame capture.
- `src/monitor_frame.rs`: Frame data structures.
- `src/lib.rs`: Library entry point.

## License

MIT

---

**Note:** This library uses unsafe code and Windows APIs. Use with care and always free resources (e.g., call `free_devices()` after use).
