use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Dxgi::*;

/// # Monitor Frame
/// 
/// Represents a captured singular frame from a Monitor struct.
pub struct MonitorFrame {
    /// The image acquired from the monitor
    pub acquired_image: Option<ID3D11Texture2D>,
    /// The size of the buffers.
    pub metadata_size: u32,

    /// Frames that moved
    pub moved_buffer: Vec<DXGI_OUTDUPL_MOVE_RECT>,

    /// Dirty frames from the monitor
    pub dirty_buffer: Vec<RECT>,

    /// Count of the dirty frames
    pub dirty_count: u32,

    /// Moved frames count
    pub moved_count: u32,

    /// Info from the frame, containing meta data.
    pub frame_info: DXGI_OUTDUPL_FRAME_INFO,
}

impl Default for MonitorFrame {
    fn default() -> Self {
        Self {
            acquired_image: None,
            moved_buffer: vec![],
            dirty_buffer: vec![],
            metadata_size: 0,
            dirty_count: 0,
            moved_count: 0,
            frame_info: DXGI_OUTDUPL_FRAME_INFO::default(),
        }
    }
}
