# win-video

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


## Examples to come
