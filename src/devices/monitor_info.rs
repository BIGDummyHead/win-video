/// # Monitor Info
/// 
/// Pertinent info on a monitor, can be used for selection and creation of a Monitor struct
pub struct MonitorInfo {

    /// The device name of the adapter or monitor.
    pub name: String,

    /// The description of the display adapter or the display monitor
    pub description: String,

    /// The monitor index. Based on all of your monitors.
    /// 
    /// For example if you have two monitors this may be 0 or 1 and so on
    pub index: u32
}

impl MonitorInfo {
    pub fn new(name: String, desc: String, index: u32) -> Self {
        return MonitorInfo { name, description: desc, index };
    }
}
