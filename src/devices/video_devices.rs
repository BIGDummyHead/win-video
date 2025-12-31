use std::ffi::c_void;

use windows::Win32::{
    Media::MediaFoundation::{
        IMFActivate, IMFAttributes, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID, MFCreateAttributes, MFEnumDeviceSources,
    },
    System::Com::CoTaskMemFree,
};

use windows::Win32::Foundation::E_FAIL;

use crate::devices::{ActivatedDevice, activated_device::Output, get_device_name};

/// # Device
///
/// Represents a Video Device interface that can be activated from your Windows machine.
///
/// # Examples
///
/// Examples to come!
pub struct VideoDevices<'a> {
    pub devices: Vec<&'a IMFActivate>,
    pp_devices: *mut Option<IMFActivate>,
}

impl<'a> VideoDevices<'a> {
    /// # New
    ///
    /// Creates a new video devices struct.
    ///
    /// Aggregates all connected video devices on your window sytem and creates a struct containing them.
    pub unsafe fn new() -> Result<Self, windows::core::Error> {
        unsafe {
            let mut ppmfattributes: Option<IMFAttributes> = None;

            MFCreateAttributes(&mut ppmfattributes as *mut _, 1)?;

            let ppmfattributes = ppmfattributes.unwrap();

            ppmfattributes.SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )?;

            let mut pp_devices: *mut Option<IMFActivate> = std::ptr::null_mut();
            let mut count: u32 = 0;

            MFEnumDeviceSources(&ppmfattributes, &mut pp_devices, &mut count)?;

            if count == 0 {
                return Err(E_FAIL.into());
            }

            let valid_devices_iter = std::slice::from_raw_parts(pp_devices, count as usize)
                .iter()
                .flatten();

            let valid_devices: Vec<&IMFActivate> = valid_devices_iter.collect();

            Ok(Self {
                devices: valid_devices,
                pp_devices,
            })
        }
    }

    /// # Activate Device
    ///
    /// Creates an Activated Device structure that gives you the ability to read data from the device (this turns it on)
    ///
    /// You may choose an Output type or None (for NV12) but this will set the type of output you will receive from the receiver.
    ///
    /// After activating any devices or after completing all operations with this struct you should call free_devices.
    pub unsafe fn activate_device(
        &self,
        device: &IMFActivate,
        output_type: Option<Output>,
    ) -> Result<ActivatedDevice, windows::core::Error> {
        unsafe {
            let name = get_device_name(device)?;

            let media_src = device
                .ActivateObject::<windows::Win32::Media::MediaFoundation::IMFMediaSource>()?;

            Ok(ActivatedDevice::new(name, media_src, output_type)?)
        }
    }

    /// # Free Devices
    /// 
    /// Uses CoTaskMemFree to free all devices that have been collected, this is essential for memory.
    pub unsafe fn free_devices(&self) {
        unsafe {
            if !self.pp_devices.is_null() {
                CoTaskMemFree(Some(self.pp_devices as *const c_void));
            }
        }
    }
}

