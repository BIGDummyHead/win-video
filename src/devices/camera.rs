use std::{pin::Pin, sync::Arc};

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

use crate::{devices::Dimensions, i_capture::ICapture};

/// Output Control
pub enum Output {
    /// Raw unprocesses data directly from the device
    NV12,
    /// Processes data as RGB32
    RGB32,
}

/// # Activated Device
///
/// Allows for the capturing of data via a IMFSourceReader.
///
/// Created from an IMFMediaSource (which can be obtained through the VideoDevices struct.)
///
/// Allows you to capture data (vec<u8>) data from a connected video device from your windows machine.
///
/// This could be a webcam or some other type of video device. This data can then be pushed through a pipeline like OpenCV for data capturing or other sorts of projects.
pub struct Camera {
    // source reader that allows to get the bytes from the device
    media_reader: IMFSourceReader,

    /// The receiver, can be used to grab data directly from the device.
    pub receiver: Arc<Mutex<Receiver<Vec<u8>>>>,

    // to send data
    sender: Sender<Vec<u8>>,

    // determines if the camera is capturing and sending data
    is_capturing: Arc<Mutex<bool>>,

    /// The type of output the camera will give back to the user
    pub output: Output,
}

impl Camera {
    /// Create a newly activated device from the IMFMediaSource provided by the activation and a name.
    ///
    /// The name should be the friendly name provided by the device before activation.
    ///
    /// Output is optional but will default to NV12 (raw)
    pub unsafe fn new(
        source: IMFMediaSource,
        output: Option<Output>,
    ) -> Result<Arc<Self>, windows::core::Error> {
        let output = output.unwrap_or(Output::NV12); //unwraps to NV12 by default
        let (tx, rx) = mpsc::channel(1);

        unsafe {
            let media_reader = Self::create_reader(&source)?;

            Self::set_stream_selection(&media_reader)?;
            Self::set_output_format(&media_reader, &output)?;

            let activated = Camera {
                media_reader,
                receiver: Arc::new(Mutex::new(rx)),
                sender: tx,
                is_capturing: Arc::new(Mutex::new(false)),
                output,
            };

            return Ok(Arc::new(activated));
        }
    }

    /// # Read Sample
    ///
    /// Using the existing media readers takes in the video stream to read from (defaults to first video stream if None) a stream.
    ///
    /// Reads a sample of the stream, converts to a buffer and retrieves the underlying data returned as Vec<u8>
    ///
    pub fn read_sample(&self, video_stream: Option<u32>) -> Result<Vec<u8>, windows::core::Error> {
        //initialize values for loading into the readsample func
        let video_stream = video_stream.unwrap_or(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32);
        let mut sample: Option<IMFSample> = None;
        let buffer: Option<IMFMediaBuffer>;
        let mut stream_index: u32 = 0;
        let mut stream_flags: u32 = 0;
        let mut time_stamp: i64 = 0;

        unsafe {
            self.media_reader.ReadSample(
                video_stream,
                0,
                Some(&mut stream_index),
                Some(&mut stream_flags),
                Some(&mut time_stamp),
                Some(&mut sample),
            )?;

            if sample.is_none() {
                return Ok(vec![]);
            }

            buffer = Some(sample.unwrap().ConvertToContiguousBuffer()?);
        }

        //ensure the buffer contains some value.
        if buffer.is_none() {
            return Err(windows::Win32::Foundation::E_FAIL.into());
        }

        let buffer = buffer.unwrap();

        Ok(Self::get_frame_data(&buffer)?)
    }

    pub fn get_frame_data(buffer: &IMFMediaBuffer) -> Result<Vec<u8>, windows::core::Error> {
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

            buffer.Unlock()?;

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
}

impl ICapture for Camera {
    type CaptureOutput = Vec<u8>;

    /// # Get Dimensions
    ///
    /// Get the device size of the video camera.
    fn get_dimensions(&self) -> Result<Dimensions, Box<dyn std::error::Error>> {
        let first_video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
        let size: Option<u64>;

        //create unsafe calls to get the media type and the dimensions store as a u64
        unsafe {
            let media_type = self.media_reader.GetCurrentMediaType(first_video_stream)?;

            size = Some(media_type.GetUINT64(&MF_MT_FRAME_SIZE)?);
        }

        if size.is_none() {
            return Err("Could not resolve size of device".into());
        }

        let size = size.unwrap();

        let width = (size >> 32) as u32;
        let height = (size & 0xFFFFFFFF) as u32;

        Ok(Dimensions { width, height })
    }

    /// ## Stop Captruing
    ///
    /// Safely stops capturing data.
    fn stop_capturing(
        self: Arc<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + Send>> {
        Box::pin(async move {
            let mut cap_guard = self.is_capturing.lock().await;

            if !*cap_guard {
                return Err("already stopped.".into());
            }

            *cap_guard = false;

            Ok(())
        })
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
    fn start_capturing(
        self: Arc<Self>,
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error>>> + Send>> {
        Box::pin(async move {
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
            loop {
                //check if capturing, drop immediately
                {
                    let is_capturing = is_capturing_ref.lock().await;

                    if !*is_capturing {
                        break;
                    }
                }

                let first_video_stream = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;

                let data = self.read_sample(Some(first_video_stream))?;

                sender.send(data).await?;
            }

            Ok(())
        })
    }

    /// # Clone Receiver
    ///
    /// Clones the receiver that is attached to the video camera buffer.
    fn clone_receiver(&self) -> Arc<Mutex<tokio::sync::mpsc::Receiver<Self::CaptureOutput>>> {
        self.receiver.clone()
    }
}

unsafe impl Send for Camera {}

unsafe impl Sync for Camera {}
