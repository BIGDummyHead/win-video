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


## Basic Data Capturing Examples

Below are some examples of how to capture data.

### Getting Device Camera Data

```rs
use win_video::{devices::{Camera, Cameras, camera::Output}, i_capture::ICapture};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    unsafe {

        //aggregate all your cameras
        let video_devices= Cameras::new()?;

        //get a device
        let webcam = video_devices.devices[0];

        //activate the device for use
        let activated_webcam: std::sync::Arc<Camera> = video_devices.activate_device(webcam, Some(Output::RGB32))?;

        //capturing using a looped thread with async behavior, it should be placed in a async thread like so:
        let cap_ref = activated_webcam.clone();
        tokio::spawn(async move {
            let _  = cap_ref.start_capturing().await;
        });

        //clone the receiver...
        let rx_ref = activated_webcam.clone_receiver();

        loop {

            // lock the receiver 
            let data = {
                let mut rx_lock = rx_ref.lock().await;

                rx_lock.recv().await
            };

            //device deactivated
            if data.is_none() {
                break;
            }

            let data = data.unwrap();

            //do whatever we need to with the data...
            println!("{}", data.len());
        }

        video_devices.free_devices();
    }

    Ok(())
}

```


### Capturing monitor data

Luckily for us, both the camera and the Monitor struct implement the trait ICapture, this means the code is generally similar for capturing monitor data

```rs

use win_video::{devices::Monitor, i_capture::ICapture};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    unsafe {

        //get the monitor by a zero based index
        //you may also use get_monitor_count
        let monitor = Monitor::from_monitor(0)?;

        //capturing using a looped thread with async behavior, it should be placed in a async thread like so:
        let cap_ref = monitor.clone();
        tokio::spawn(async move {
            let _ = cap_ref.start_capturing().await;
        });

        //clone the receiver...
        let rx_ref = monitor.clone_receiver();

        loop {
            // lock the receiver
            let data = {
                let mut rx_lock = rx_ref.lock().await;

                rx_lock.recv().await
            };

            //device deactivated
            if data.is_none() {
                break;
            }

            let data = data.unwrap();

            //do whatever we need to with the data...
            println!("{}", data.len());
        }
    }

    Ok(())
}
```

As you can see it is pretty straightforward to capture data from either a monitor or a camera on Windows. However, if we delve into the trait ICapture, it can be even more generic.

### ICapture

Both the monitor and activated camera implement the ICapture trait with the following functions below.

```rs

    /// # Get Dimensions
    /// 
    /// Retrieve the device dimensions of the capture.
    /// 
    /// This could be used to capture the size of a monitor for example (1920x1080)
    fn get_dimensions(&self) -> Result<Dimensions, Box<dyn std::error::Error>>;

    /// # Stop Capturing
    /// 
    /// Indicates that the device should stop sending some sort of data
    fn stop_capturing(self: Arc<Self>) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + Send>>;

    /// # Start Capturing
    /// 
    /// Indicates the device should start sending some sort of data
    fn start_capturing(self: Arc<Self>) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + Send>>;

    /// # Get Receiver
    /// 
    /// Get the receiver reference associated with sending data.
    fn clone_receiver(&self) -> Arc<Mutex<Receiver<Self::CaptureOutput>>>;
```

This means that we could hypothetically ask the user for their desired capture device and then provide them with an ICapture rather than a specific Monitor or Camera.

We could do it like so:

```rs

enum CaptureType {
    Camera,
    Monitor(u32)
}


unsafe fn get_capture(cap_type: &CaptureType) -> Result< Arc<dyn ICapture<CaptureOutput = Vec<u8>>>, Box<dyn std::error::Error + 'static>> {

    match cap_type {
        CaptureType::Camera => {
            unsafe {
                let cameras = Cameras::new()?;

                let camera = cameras.activate_device(cameras.devices[0], Some(win_video::devices::camera::Output::RGB32))?;

                return Ok(camera as Arc<dyn ICapture<CaptureOutput = Vec<u8>>>);
            }
        },
        CaptureType::Monitor(ind) => {
            
            unsafe {
                let monitor = Monitor::from_monitor(*ind)?;

                return Ok(monitor as Arc<dyn ICapture<CaptureOutput = Vec<u8>>>);
            }
        }
    }
}

```

In where the code would be usable with the examples given like so:

```rs

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
    unsafe {
        let capture = get_capture(&CaptureType::Camera)?;

        //capturing using a looped thread with async behavior, it should be placed in a async thread like so:
        let cap_ref = capture.clone();
        tokio::spawn(async move {
            let _ = cap_ref.start_capturing().await;
        });

        //clone the receiver...
        let rx_ref = capture.clone_receiver();

        loop {
            // lock the receiver
            let data = {
                let mut rx_lock = rx_ref.lock().await;

                rx_lock.recv().await
            };

            //device deactivated
            if data.is_none() {
                break;
            }

            let data = data.unwrap();

            //do whatever we need to with the data...
            println!("{}", data.len());
        }
    }

    Ok(())
}

```
