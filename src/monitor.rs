use std::sync::Arc;

use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::{Mutex, mpsc};
use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_FLAG, D3D11_CPU_ACCESS_READ, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE,
    D3D11_RESOURCE_MISC_FLAG, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, ID3D11DeviceContext,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{DXGI_OUTDUPL_MOVE_RECT, IDXGIDevice, IDXGIOutput1};
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct3D::D3D_DRIVER_TYPE_HARDWARE,
        Direct3D11::{
            D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice, ID3D11Device,
        },
        Dxgi::*,
    },
};
use windows::core::Interface;

use crate::devices::DeviceSize;
use crate::monitor_frame::MonitorFrame;

pub type Frame = Vec<u8>;

pub struct Monitor {
    /// The IDXGIOutputDuplication interface accesses and manipulates the duplicated desktop image.
    duplication_output: IDXGIOutputDuplication,

    pub receiver: Arc<Mutex<Receiver<Frame>>>,
    sender: Sender<Frame>,

    is_sending: Arc<Mutex<bool>>,

    frame: Arc<Mutex<MonitorFrame>>,

    device_context: ID3D11DeviceContext,

    //texture that is used to copy from the GPU to CPU, expensive, so made on init
    staging_texture: ID3D11Texture2D,

    pub desktop_size: DeviceSize,

    pub name: String,
}

impl Monitor {
    /// ## Create
    ///
    /// Create device information for a given monitor of your system.
    ///
    /// Provides a Monitor struct that has the ability to duplicate the data and do other manipulation.
    pub unsafe fn from_monitor(monitor: u32) -> Result<Self, windows::core::Error> {
        unsafe {
            //choose default adapater
            let adapter = None;

            //A hardware driver, which implements Direct3D features in hardware.
            //This is the primary driver that you should use in your Direct3D applications because it provides the best performance.
            let driver_type = D3D_DRIVER_TYPE_HARDWARE;

            //add support for duplication
            let flags = D3D11_CREATE_DEVICE_BGRA_SUPPORT;

            //use default
            let p_feature_levels = None;

            //device should be unwrappable after the following function call:
            let mut device = None;

            //this is okay to be null since we are using a NON-Software type for the driver_type
            //A handle to a DLL that implements a software rasterizer. If DriverType is D3D_DRIVER_TYPE_SOFTWARE, Software must not be NULL.
            let module_handle = HMODULE(std::ptr::null_mut());

            let mut device_context: Option<ID3D11DeviceContext> = None;

            D3D11CreateDevice(
                adapter,
                driver_type,
                module_handle,
                flags,
                p_feature_levels,
                D3D11_SDK_VERSION,
                Some(&mut device),
                None, //we do not need to determine the feature level at this time
                Some(&mut device_context), //we do no tneed the device context at this time
            )?;

            let device: ID3D11Device = device.unwrap();

            let dxgi_device: IDXGIDevice = device.cast()?;
            let adapter: IDXGIAdapter = dxgi_device.GetAdapter()?;

            let monitor_output: windows::Win32::Graphics::Dxgi::IDXGIOutput =
                adapter.EnumOutputs(monitor)?;

            let monitor_output1: IDXGIOutput1 = monitor_output.cast()?;

            let desc = monitor_output1.GetDesc()?;

            //get the size of the monitor
            let coordinates = &desc.DesktopCoordinates;
            let device_size = DeviceSize {
                width: (coordinates.right - coordinates.left) as u32,
                height: (coordinates.bottom - coordinates.top) as u32,
            };

            let dup_output = monitor_output1.DuplicateOutput(&device)?;

            let (tx, rx) = mpsc::channel(1);

            let staging_texture = Self::create_staging_texture(&device, &device_size)?;

            Ok(Self {
                duplication_output: dup_output,
                sender: tx,
                receiver: Arc::new(Mutex::new(rx)),
                is_sending: Arc::new(Mutex::new(false)),
                frame: Arc::new(Mutex::new(MonitorFrame::default())),
                device_context: device_context.unwrap(),
                staging_texture,
                desktop_size: device_size,
                name: String::from_utf16_lossy(&desc.DeviceName),
            })
        }
    }

    /// creates a texture that can be used to copy GPU based monitor data to the CPU
    fn create_staging_texture(
        device: &ID3D11Device,
        device_size: &DeviceSize,
    ) -> Result<ID3D11Texture2D, windows::core::Error> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: device_size.width,
            Height: device_size.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM, // Standard for Desktop Duplication
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING, // Crucial: Allows CPU reading
            BindFlags: D3D11_BIND_FLAG(0).0 as u32, // Staging textures have no bind flags
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32, // Allows us to call .Map()
            MiscFlags: D3D11_RESOURCE_MISC_FLAG(0).0 as u32,
        };

        let mut staging_texture = None;
        unsafe {
            device.CreateTexture2D(&desc, None, Some(&mut staging_texture))?;
        }

        Ok(staging_texture.unwrap())
    }

    /// # Stop Cloning
    ///
    /// Safely stops the cloning of the monitor.
    pub async fn stop_cloning(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut is_sending = self.is_sending.lock().await;

        if !*is_sending {
            return Err("Not sending any data".into());
        }

        *is_sending = false;
        Ok(())
    }

    /// # Start Cloning
    ///
    /// Starts cloning the given monitor from the system.
    ///
    /// Once cloning begins for the monitor it will start sending data to the monitor receiver.
    ///
    /// Please refer to MPSC on how to receive data asynchonously.
    ///
    /// ## Warning
    ///
    /// It is very important to note that this operation may only occurr on the main thread and is thread blocking.
    ///
    /// You must start a task that reads the data before starting cloning, you can then stop cloning the data inside of the newly started task.
    pub async unsafe fn start_cloning(&self) -> Result<(), Box<dyn std::error::Error>> {

        {
            let mut sending_lock = self.is_sending.lock().await;

            if *sending_lock {
                return Err("you are already cloning data".into());
            }

            *sending_lock = true;
        }

        loop {
            //take the lock, the value, and drop
            let is_sending_currently = { *self.is_sending.lock().await };
            if !is_sending_currently {
                break;
            }

            unsafe {
                let monitor_frame = self.acquire_data().await;

                if let Err(e) = monitor_frame {
                    //this is forgiveable, just no new data was accquired within the specified window time.
                    if e.code() == DXGI_ERROR_WAIT_TIMEOUT.into() {
                        continue;
                    }

                    // this is another error.
                    return Err(Box::new(e));
                }

                let monitor_frame = monitor_frame.unwrap();

                let mut frame_lock = self.frame.lock().await;
                *frame_lock = monitor_frame;

                let acquired_image = frame_lock.acquired_image.clone();

                drop(frame_lock);

                self.device_context
                    .CopyResource(&self.staging_texture, acquired_image.as_ref().unwrap());

                self.device_context.Flush();

                //we now have access to the data
                let mut mapped_resource = D3D11_MAPPED_SUBRESOURCE::default();

                self.device_context.Map(
                    &self.staging_texture,
                    0,
                    D3D11_MAP_READ,
                    0,
                    Some(&mut mapped_resource),
                )?;

                let row_pitch = mapped_resource.RowPitch as usize;
                let total_size_bytes = row_pitch * self.desktop_size.height as usize;

                let data = std::slice::from_raw_parts(
                    mapped_resource.pData as *const u8,
                    total_size_bytes,
                )
                .to_vec();

                let send_res = self.sender.send(data).await;

                //release all data.
                self.device_context.Unmap(&self.staging_texture, 0);

                self.release_frames().await?;

                if let Err(e) = send_res {
                    return Err(format!("Failed to send frame: {}", e).into());
                }
            }
        }

        Ok(())
    }

    // releases the frames and readies the monitor for another batch of duplication
    async unsafe fn release_frames(&self) -> Result<(), windows::core::Error> {
        unsafe {
            //release the frames
            self.duplication_output.ReleaseFrame()?;
        }
        self.frame.lock().await.acquired_image = None;
        Ok(())
    }

    /// acquires a monitory frame based on previous monitor frames
    async unsafe fn acquire_data(&self) -> Result<MonitorFrame, windows::core::Error> {
        unsafe {
            let timeout_ms = 500;
            let mut desktop_resource: Option<windows::Win32::Graphics::Dxgi::IDXGIResource> = None;

            let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();

            //acquire new frame
            self.duplication_output.AcquireNextFrame(
                timeout_ms,
                &mut frame_info,
                &mut desktop_resource,
            )?;

            // release the desktop resource after drop
            let desktop_resource = desktop_resource.unwrap();

            let acquired_image = Some(desktop_resource.cast::<ID3D11Texture2D>()?);

            // indicates to use the previous
            let mut moved_buffer = None;
            let mut dirty_buffer = None;
            let mut metadata_size = None;

            let frame_lock = self.frame.lock().await;

            // old buffer too small, this is always called on the first hit
            if frame_info.TotalMetadataBufferSize > frame_lock.metadata_size {
                let new_buffer_size = frame_info.TotalMetadataBufferSize as usize;

                //allocate new metadata buffer
                moved_buffer = Some(vec![DXGI_OUTDUPL_MOVE_RECT::default(); new_buffer_size]);
                dirty_buffer = Some(vec![RECT::default(); new_buffer_size]);

                metadata_size = Some(new_buffer_size as u32);
            }

            let metadata_size = metadata_size.unwrap_or(frame_lock.metadata_size);

            let mut moved_buffer = moved_buffer.unwrap_or(frame_lock.moved_buffer.clone());
            let mut dirty_buffer = dirty_buffer.unwrap_or(frame_lock.dirty_buffer.clone());

            //drop the ref
            drop(frame_lock);

            let mut move_bytes_returned: u32 = 0;
            self.duplication_output.GetFrameMoveRects(
                metadata_size, // Use the full buffer capacity
                moved_buffer.as_mut_ptr(),
                &mut move_bytes_returned,
            )?;

            let moved_count =
                move_bytes_returned / std::mem::size_of::<DXGI_OUTDUPL_MOVE_RECT>() as u32;

            let mut dirty_bytes_returned: u32 = 0;
            self.duplication_output.GetFrameDirtyRects(
                metadata_size,
                dirty_buffer.as_mut_ptr(),
                &mut dirty_bytes_returned,
            )?;

            let dirty_count = dirty_bytes_returned / std::mem::size_of::<RECT>() as u32;

            Ok(MonitorFrame {
                acquired_image,
                metadata_size,
                moved_buffer,
                dirty_buffer,
                dirty_count,
                moved_count,
                frame_info,
            })
        }
    }
}

unsafe impl Send for Monitor {}

unsafe impl Sync for Monitor {}
