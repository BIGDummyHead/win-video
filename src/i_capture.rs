use std::{pin::Pin, sync::Arc};

use tokio::sync::{Mutex, mpsc::Receiver};

use crate::devices::Dimensions;

/// # I Capture
/// 
/// Trait that enables capturing of some sort of resource such as a monitor screen or camera.

pub trait ICapture: Send + Sync {

    type CaptureOutput;
    
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
}
