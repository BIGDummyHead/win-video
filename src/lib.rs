pub mod devices;
pub mod monitor;
pub mod monitor_frame;

#[cfg(test)]
mod tests {

    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

    use std::sync::Arc;

    use crate::devices::{VideoDevices, get_device_name};
    use crate::monitor::Monitor;

    use windows::Win32::{
        Media::MediaFoundation::{
            IMFActivate, IMFAttributes, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
            MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID, MFCreateAttributes,
            MFEnumDeviceSources,
        },
        System::Com::CoTaskMemFree,
    };

    #[tokio::test]
    async fn test_desktop_duplication() -> () {
        unsafe {
            let monitor_index = 1;
            let monitor = Monitor::from_monitor(monitor_index);

            assert!(
                monitor.is_ok(),
                "Desktop Duplicator failed: {:?}",
                monitor.err()
            );

            let monitor = Arc::new(monitor.unwrap());

            let monitor_clone = monitor.clone();

            let recv = monitor_clone.receiver.clone();

            tokio::spawn(async move {
                let mut recv = recv.lock().await;

                loop {
                    let data = recv.recv().await;

                    assert!(data.is_some());

                    let data = data.unwrap();

                    let mut had_data = false;
                    for d in &data {
                        if *d != 0 {
                            had_data = true;
                            break;
                        }
                    }

                    if had_data {
                        break;
                    }
                }
                
                let stopped = monitor_clone.stop_cloning().await;
                assert!(stopped.is_ok());
            });

            let clone = monitor.start_cloning().await;

            assert!(clone.is_ok(), "{clone:?}");
        }
    }

    #[test]
    fn find_video_devices() -> () {
        unsafe {
            let mut ppmfattributes: Option<IMFAttributes> = None;

            let hr = MFCreateAttributes(&mut ppmfattributes as *mut _, 1);

            assert!(hr.is_ok(), "MFCreateAttributes failed: {:?}", hr);
            assert!(
                ppmfattributes.is_some(),
                "Failed to set PPMF Attributes, cannot unwrap."
            );

            let ppmfattributes = ppmfattributes.unwrap();

            let hr = ppmfattributes.SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            );
            assert!(hr.is_ok(), "Failed to Set video GUID: {:?}", hr);

            let mut pp_devices: *mut Option<IMFActivate> = std::ptr::null_mut();
            let mut count: u32 = 0;

            let hr = MFEnumDeviceSources(&ppmfattributes, &mut pp_devices, &mut count);
            assert!(hr.is_ok(), "MFEnumDeviceSources failed: {:?}", hr);
            assert!(
                !pp_devices.is_null(),
                "After enumerating, devices were null."
            );

            if count == 0 {
                todo!("There are no devices, error will be implemented soon for this.");
            }

            let valid_devices = std::slice::from_raw_parts(pp_devices, count as usize)
                .iter()
                .flatten();

            assert!(
                valid_devices.clone().count() > 0,
                "No devices found after flatten."
            );

            CoTaskMemFree(Some(pp_devices as *const std::ffi::c_void));
        }
    }

    #[test]
    fn collect_devices() {
        unsafe {
            let devices = VideoDevices::new();

            assert!(devices.is_ok());

            let video_devices = devices.unwrap();

            assert!(!video_devices.devices.is_empty());

            for device in &video_devices.devices {
                let name = get_device_name(*device);

                assert!(name.is_ok());

                let name = name.unwrap();

                println!("Name of device: '{name}'");
            }

            video_devices.free_devices();
        }
    }

    #[test]
    fn test_activation() {
        unsafe {
            let init = CoInitializeEx(None, COINIT_MULTITHREADED);

            assert!(
                init == windows::Win32::Foundation::S_OK,
                "Co Initialize was not OK: {init}"
            );

            let devices = VideoDevices::new();

            assert!(devices.is_ok());

            let devices = devices.unwrap();

            assert!(!devices.devices.is_empty());

            let activated_device = devices.activate_device(devices.devices[0], None);

            assert!(activated_device.is_ok(), "{:?}", activated_device.err());

            let activated_device = activated_device.unwrap();

            println!("Activated Media Device Name: '{}'", activated_device.name);
        }
    }

    #[tokio::test]
    async fn capture_image() {
        unsafe {
            let init = CoInitializeEx(None, COINIT_MULTITHREADED);

            assert!(
                init == windows::Win32::Foundation::S_OK,
                "Co Initialize was not OK: {init}"
            );

            let devices = VideoDevices::new();

            assert!(devices.is_ok());

            let devices = devices.unwrap();

            assert!(!devices.devices.is_empty());

            let activated_device = devices.activate_device(devices.devices[0], None);

            assert!(activated_device.is_ok(), "{:?}", activated_device.err());

            let activated_device = Arc::new(activated_device.unwrap());

            println!("Activated Media Device Name: '{}'", activated_device.name);

            let receiver = activated_device.receiver.clone();

            let activated_device_clone = activated_device.clone();

            tokio::spawn(async move {
                let mut receiver_guard = receiver.lock().await;
                println!("Thread spawned, receiver initialized.");
                loop {
                    let data = receiver_guard.recv().await;

                    assert!(data.is_some());

                    let data = data.unwrap();

                    if data.len() > 0 {
                        let stopped = (*activated_device_clone).stop_capturing().await;

                        assert!(stopped.is_ok());
                        break;
                    }
                }
            });

            let capturing = activated_device.start_capturing().await;
            assert!(capturing.is_ok());

            devices.free_devices();
        }
    }
}
