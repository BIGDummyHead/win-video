use std::sync::Arc;

use tokio::sync::{
    Mutex,
    mpsc::{self, Receiver, Sender},
};
use windows::Win32::{
    Foundation::E_ABORT,
    Media::MediaFoundation::{
        IMFAttributes, IMFMediaBuffer, IMFMediaSource, IMFSample, IMFSourceReader,
        MF_MT_FRAME_SIZE, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE, MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS,
        MF_SOURCE_READER_ALL_STREAMS, MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING,
        MF_SOURCE_READER_FIRST_VIDEO_STREAM, MFCreateAttributes, MFCreateMediaType,
        MFCreateSourceReaderFromMediaSource, MFMediaType_Video, MFVideoFormat_NV12,
        MFVideoFormat_RGB32,
    },
};

use crate::devices::DeviceSize;

/// Output Control
pub enum Output {
    /// Raw unprocesses data directly from the device
    NV12,
    /// Processes data as RGB32
    RGB32,
}

pub type VideoFrame = Vec<u8>;

/// # Activated Device
///
/// Allows for the capturing of data via a IMFSourceReader.
///
/// Created from an IMFMediaSource (which can be obtained through the VideoDevices struct.)
///
/// Allows you to capture data (vec<u8>) data from a connected video device from your windows machine.
///
/// This could be a webcam or some other type of video device. This data can then be pushed through a pipeline like OpenCV for data capturing or other sorts of projects.
pub struct ActivatedDevice {
    pub name: String,
    media_reader: IMFSourceReader,

    pub receiver: Arc<Mutex<Receiver<VideoFrame>>>,
    sender: Sender<VideoFrame>,
    is_capturing: Arc<Mutex<bool>>,
    pub size: Arc<DeviceSize>,
    pub output: Output,
}

impl ActivatedDevice {
    /// Create a newly activated device from the IMFMediaSource provided by the activation and a name.
    ///
    /// The name should be the friendly name provided by the device before activation.
    ///
    /// Output is optional but will default to NV12 (raw)
    pub unsafe fn new(
        name: String,
        source: IMFMediaSource,
        output: Option<Output>,
    ) -> Result<Self, windows::core::Error> {
        let output = output.unwrap_or(Output::NV12); //unwraps to NV12 by default

        unsafe {
            let reader = Self::create_reader(&source)?;

            Self::set_stream_selection(&reader)?;
            Self::set_output_format(&reader, &output)?;

            let size = Self::get_size(&reader)?;

            let (tx, rx) = mpsc::channel(1);

            let activated_device = ActivatedDevice {
                name,
                media_reader: reader,
                receiver: Arc::new(Mutex::new(rx)),
                sender: tx,
                is_capturing: Arc::new(Mutex::new(false)),
                size: Arc::new(size),
                output,
            };

            return Ok(activated_device);
        }
    }

    /// ## Stop Captruing
    ///
    /// Safely stops capturing data.
    pub async fn stop_capturing(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut cap_guard = self.is_capturing.lock().await;

        if !*cap_guard {
            return Err("already stopped.".into());
        }

        *cap_guard = false;

        Ok(())
    }

    /// # Start Capturing
    ///
    /// Safely start capturing data. Once started you may use the attached receiver on the activated device.
    ///
    /// Please see MPSC if you are not familiar with how to receive the data.
    ///
    /// ## Warning
    ///
    /// This operation contains a loop and will block until stop_capturing is called...
    ///
    /// You must start this on your main thread. You may then create a task that controls the stop_capturing function as this struct is send+sync safe.
    pub async fn start_capturing(&self) -> Result<(), Box<dyn std::error::Error>> {
        // lock the capguard, check if already capturing, if not set as true and continue
        {
            let mut cap_guard = self.is_capturing.lock().await;

            if *cap_guard {
                return Err("already capturing".into());
            }

            *cap_guard = true;
        }

        //clone all resources that need to be moved
        let is_capturing_ref = self.is_capturing.clone();
        let sender = self.sender.clone();
        let media_reader = self.media_reader.clone();

        loop {
            //check if capturing, drop immediately
            {
                let is_capturing = is_capturing_ref.lock().await;

                if !*is_capturing {
                    break;
                }
            }

            let first_video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            let mut pdwactualstreamindex: u32 = 0;
            let mut pdwstreamflags: u32 = 0;
            let mut plltimestamp: i64 = 0;

            let mut sample: Option<IMFSample> = None;

            unsafe {
                media_reader.ReadSample(
                    first_video_stream,
                    0,
                    Some(&mut pdwactualstreamindex),
                    Some(&mut pdwstreamflags),
                    Some(&mut plltimestamp),
                    Some(&mut sample),
                )?;

                if sample.is_none() {
                    continue;
                }

                let sample = sample.unwrap();

                let buffer = sample.ConvertToContiguousBuffer()?;

                let data = Self::get_frame_data(&buffer)?;
                sender.send(data).await?;

                buffer.Unlock()?;
            }
        }

        Ok(())
    }

    unsafe fn get_frame_data(
        buffer: &IMFMediaBuffer,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut pcbmaxlength: u32 = 0;
        let mut pcbcurrentlength: u32 = 0;

        let mut ppbbuffer: *mut u8 = std::ptr::null_mut();

        unsafe {
            buffer.Lock(
                &mut ppbbuffer,
                Some(&mut pcbmaxlength),
                Some(&mut pcbcurrentlength),
            )?;

            //ppbbuffer now contains our video frame data.

            let frame_data =
                std::slice::from_raw_parts(ppbbuffer, pcbcurrentlength as usize).to_vec();

            Ok(frame_data)
        }
    }

    // sets the output format for the receiver.
    unsafe fn set_output_format(
        reader: &IMFSourceReader,
        output: &Output,
    ) -> Result<(), windows::core::Error> {
        unsafe {
            let media_type = MFCreateMediaType()?;

            media_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;

            let guid_value = match output {
                Output::NV12 => &MFVideoFormat_NV12,
                Output::RGB32 => &MFVideoFormat_RGB32,
            };

            media_type.SetGUID(&MF_MT_SUBTYPE, guid_value)?;

            let first_video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            reader.SetCurrentMediaType(first_video_stream, None, &media_type)?;
        }

        Ok(())
    }

    // set the stream selection, this is by default the first video stream from all rendering streams.
    unsafe fn set_stream_selection(reader: &IMFSourceReader) -> Result<(), windows::core::Error> {
        unsafe {
            let all_streams = MF_SOURCE_READER_ALL_STREAMS.0 as u32;
            reader.SetStreamSelection(all_streams, false)?;

            let first_video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            reader.SetStreamSelection(first_video_stream, true)?;
        }

        Ok(())
    }

    // creates the IMFSource reader for video processing and enables hardware transforms
    unsafe fn create_reader(
        source: &IMFMediaSource,
    ) -> Result<IMFSourceReader, windows::core::Error> {
        unsafe {
            let mut options: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut options, 2)?;

            if options.is_none() {
                return Err(E_ABORT.into());
            }

            let attrs = options.unwrap();
            attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)?;

            attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)?;

            let reader: IMFSourceReader = MFCreateSourceReaderFromMediaSource(source, &attrs)?;

            Ok(reader)
        }
    }

    // retrieves the size of the stream (width + height) this is cached to the struct on creation
    unsafe fn get_size(reader: &IMFSourceReader) -> Result<DeviceSize, windows::core::Error> {
        unsafe {
            let first_video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
            let media_type = reader.GetCurrentMediaType(first_video_stream)?;

            let size: u64 = media_type.GetUINT64(&MF_MT_FRAME_SIZE)?;

            let width = (size >> 32) as u32;
            let height = (size & 0xFFFFFFFF) as u32;

            let device_size = DeviceSize { width, height };

            Ok(device_size)
        }
    }
}

unsafe impl Send for ActivatedDevice {}

unsafe impl Sync for ActivatedDevice {}
