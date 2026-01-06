pub mod camera;
pub mod cameras;
pub mod dimensions;
pub mod monitor;
pub mod monitor_frame;
pub mod monitor_info;

pub use crate::devices::camera::Camera;
pub use crate::devices::cameras::Cameras;
pub use crate::devices::dimensions::Dimensions;
pub use crate::devices::monitor::Monitor;
pub use crate::devices::monitor_frame::MonitorFrame;
use crate::devices::monitor_info::MonitorInfo;

use windows::Win32::{
    Graphics::Gdi::{DISPLAY_DEVICEW, EnumDisplayDevicesW},
    Media::MediaFoundation::{IMFActivate, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME},
    UI::WindowsAndMessaging::{EDD_GET_DEVICE_INTERFACE_NAME, GetSystemMetrics, SM_CMONITORS},
};

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

/// # Get Monitor Count
///
/// The numer of display monitors on a desktop.
pub unsafe fn get_monitor_count() -> i32 {
    unsafe { GetSystemMetrics(SM_CMONITORS) }
}

/// # Get All Monitor Info
///
/// Retrieves pertinent information about all monitors on your system and returns them as a Vec.
///
/// These can then be filitered through and be used to create a Monitor object.
pub unsafe fn get_all_monitor_info() -> Vec<MonitorInfo> {
    let mut monitors = vec![];

    //the current index of monitor
    let mut device_index: u32 = 0;
    
    unsafe {
        //loop over all monitors in the system
        loop {

            //generate device info
            let mut device_info = DISPLAY_DEVICEW::default();

            //gets information about the display, returns false if the device index was out of bounds.
            let exist = EnumDisplayDevicesW(
                None,
                device_index,
                &mut device_info,
                EDD_GET_DEVICE_INTERFACE_NAME,
            ).as_bool();

            // no more devices exist.
            if !exist {
                break;
            }

            //save and generate a MonitorInfo object
            let device_name = String::from_utf16_lossy(&device_info.DeviceName);
            let device_desc = String::from_utf16_lossy(&device_info.DeviceString);

            let info = MonitorInfo::new(device_name, device_desc, device_index);

            //push
            monitors.push(info);

            device_index += 1;
        }
    }

    monitors
}
