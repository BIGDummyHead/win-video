pub mod video_devices;
pub mod activated_device;
pub mod device_size;

pub use crate::devices::video_devices::VideoDevices;
pub use crate::devices::activated_device::ActivatedDevice;
pub use crate::devices::device_size::DeviceSize;
/// # Get Device Name
/// 
/// From an activateable device, get the friendly device name.
/// 
/// This is useful when you want to enumerate over devices and want to select a device by the friendly based name.


use windows::Win32::Media::MediaFoundation::{IMFActivate, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME};

/// # Get Device Name
/// 
/// From an activated device retrieves the name of the device that is friendly (meaning readible)
/// 
/// This can be used to sort and find device names.
pub unsafe fn get_device_name(device: &IMFActivate) -> Result<String, windows::core::Error> {
    unsafe {
        let mut name_len: u32 = 0;
        let mut pw_name: windows::core::PWSTR = windows::core::PWSTR::null();

        device.GetAllocatedString(
            &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
            &mut pw_name,
            &mut name_len,
        )?;

        let name = String::from_utf16_lossy(pw_name.as_wide());

        Ok(name)
    }
}