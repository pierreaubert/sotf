// ============================================================================
// AVFoundation Camera Capture (macOS)
// ============================================================================
//
// Direct AVFoundation camera capture using objc2 bindings.
// More reliable than nokhwa on macOS.

use crate::HeadTrackerError;
use crate::camera::CameraFrame;
use crossbeam::queue::ArrayQueue;
use log::{info, trace, warn};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AllocAnyThread, DeclaredClass, define_class, msg_send};
use objc2_av_foundation::{
    AVCaptureConnection, AVCaptureDevice, AVCaptureDeviceInput, AVCaptureOutput, AVCaptureSession,
    AVCaptureVideoDataOutput, AVCaptureVideoDataOutputSampleBufferDelegate, AVMediaTypeVideo,
};
use objc2_core_media::CMSampleBuffer;
use objc2_core_video::{
    CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow, CVPixelBufferGetHeight,
    CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelFormatType_32BGRA,
};
use objc2_foundation::{NSNumber, NSObject, NSObjectProtocol, NSString};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Frame buffer shared between delegate and capture
struct FrameBuffer {
    queue: ArrayQueue<CameraFrame>,
    start_time_ms: AtomicU64,
}

impl FrameBuffer {
    fn new() -> Self {
        Self {
            queue: ArrayQueue::new(4), // Small queue, we only need latest
            start_time_ms: AtomicU64::new(0),
        }
    }
}

/// Delegate that receives video frames from AVFoundation
struct DelegateIvars {
    buffer: Arc<FrameBuffer>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = AllocAnyThread]
    #[name = "SotfCaptureDelegate"]
    #[ivars = DelegateIvars]
    struct CaptureDelegate;

    unsafe impl NSObjectProtocol for CaptureDelegate {}

    unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for CaptureDelegate {
        #[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
        fn capture_output_did_output(
            &self,
            _output: &AVCaptureOutput,
            sample_buffer: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            trace!("Delegate received sample buffer callback");
            self.handle_sample_buffer(sample_buffer);
        }

        #[unsafe(method(captureOutput:didDropSampleBuffer:fromConnection:))]
        fn capture_output_did_drop(
            &self,
            _output: &AVCaptureOutput,
            _sample_buffer: &CMSampleBuffer,
            _connection: &AVCaptureConnection,
        ) {
            warn!("Delegate dropped frame");
        }
    }
);

impl CaptureDelegate {
    fn new(buffer: Arc<FrameBuffer>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { buffer });
        // SAFETY: NSObject's init is safe
        unsafe { msg_send![super(this), init] }
    }

    fn handle_sample_buffer(&self, sample_buffer: &CMSampleBuffer) {
        // SAFETY: Getting image buffer from sample buffer is safe
        let pixel_buffer = match unsafe { sample_buffer.image_buffer() } {
            Some(buf) => buf,
            None => {
                warn!("Sample buffer has no image buffer");
                return;
            }
        };

        // Lock the pixel buffer for reading
        // SAFETY: Standard CVPixelBuffer locking pattern
        let read_only_flag = CVPixelBufferLockFlags(1); // kCVPixelBufferLock_ReadOnly = 1
        let lock_result = unsafe { CVPixelBufferLockBaseAddress(&pixel_buffer, read_only_flag) };

        if lock_result != 0 {
            warn!("Failed to lock pixel buffer: {}", lock_result);
            return;
        }

        // Get buffer properties (these are safe functions in objc2-core-video)
        let width = CVPixelBufferGetWidth(&pixel_buffer);
        let height = CVPixelBufferGetHeight(&pixel_buffer);
        let bytes_per_row = CVPixelBufferGetBytesPerRow(&pixel_buffer);
        let base_address = CVPixelBufferGetBaseAddress(&pixel_buffer);

        if base_address.is_null() {
            warn!("Pixel buffer base address is null");
            // SAFETY: Must unlock even on error
            unsafe { CVPixelBufferUnlockBaseAddress(&pixel_buffer, read_only_flag) };
            return;
        }

        // Convert BGRA to RGB
        let rgb_data = self.bgra_to_rgb(base_address, width, height, bytes_per_row);

        // Unlock the pixel buffer
        // SAFETY: We locked it above
        unsafe { CVPixelBufferUnlockBaseAddress(&pixel_buffer, read_only_flag) };

        // Calculate timestamp
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let start_ms = self.ivars().buffer.start_time_ms.load(Ordering::Relaxed);
        let timestamp_ms = now_ms.saturating_sub(start_ms);

        // Create frame and push to queue
        let frame = CameraFrame {
            data: rgb_data,
            width: width as u32,
            height: height as u32,
            timestamp_ms,
        };

        // Force push (drops old frame if full)
        let _ = self.ivars().buffer.queue.force_push(frame);
        trace!("Captured frame {}x{}", width, height);
    }

    fn bgra_to_rgb(
        &self,
        base_address: *mut std::ffi::c_void,
        width: usize,
        height: usize,
        bytes_per_row: usize,
    ) -> Vec<u8> {
        let mut rgb = Vec::with_capacity(width * height * 3);

        for y in 0..height {
            let row_start = base_address as usize + y * bytes_per_row;
            for x in 0..width {
                let pixel_offset = row_start + x * 4;
                // SAFETY: We know the buffer layout (BGRA)
                unsafe {
                    let b = *(pixel_offset as *const u8);
                    let g = *((pixel_offset + 1) as *const u8);
                    let r = *((pixel_offset + 2) as *const u8);
                    // Skip alpha at offset + 3
                    rgb.push(r);
                    rgb.push(g);
                    rgb.push(b);
                }
            }
        }

        rgb
    }
}

/// AVFoundation camera capture
pub struct AVFoundationCapture {
    session: Option<Retained<AVCaptureSession>>,
    delegate: Option<Retained<CaptureDelegate>>,
    buffer: Arc<FrameBuffer>,
    is_running: Arc<AtomicBool>,
    camera_index: usize,
}

impl std::fmt::Debug for AVFoundationCapture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AVFoundationCapture")
            .field("camera_index", &self.camera_index)
            .field("is_running", &self.is_running.load(Ordering::Relaxed))
            .finish()
    }
}

impl AVFoundationCapture {
    /// Create a new AVFoundation camera capture
    pub fn new(camera_index: usize) -> Self {
        Self {
            session: None,
            delegate: None,
            buffer: Arc::new(FrameBuffer::new()),
            is_running: Arc::new(AtomicBool::new(false)),
            camera_index,
        }
    }

    /// Start camera capture
    pub fn start(&mut self) -> Result<(), HeadTrackerError> {
        if self.is_running.load(Ordering::Relaxed) {
            return Err(HeadTrackerError::AlreadyRunning);
        }

        info!(
            "Starting AVFoundation camera capture (index {})",
            self.camera_index
        );

        // Create capture session
        // SAFETY: AVCaptureSession::new is safe
        let session = unsafe { AVCaptureSession::new() };

        // Get the video media type
        // SAFETY: AVMediaTypeVideo is safe to access
        let video_type = unsafe { AVMediaTypeVideo }.ok_or_else(|| {
            HeadTrackerError::Camera("AVMediaTypeVideo not available".to_string())
        })?;

        // Get the video device
        // SAFETY: defaultDeviceWithMediaType is safe
        let device = unsafe { AVCaptureDevice::defaultDeviceWithMediaType(video_type) }
            .ok_or_else(|| HeadTrackerError::Camera("No video device found".to_string()))?;

        info!("Using camera: {}", unsafe { device.localizedName() });

        // Create device input
        // SAFETY: deviceInputWithDevice returns Option or error
        let input = unsafe {
            AVCaptureDeviceInput::deviceInputWithDevice_error(&device).map_err(|e| {
                HeadTrackerError::Camera(format!(
                    "Failed to create input: {}",
                    e.localizedDescription()
                ))
            })?
        };

        // Create video data output
        // SAFETY: AVCaptureVideoDataOutput::new is safe
        let output = unsafe { AVCaptureVideoDataOutput::new() };

        // Configure output for BGRA format (easier to convert to RGB)
        // SAFETY: Setting video settings is safe
        unsafe {
            // Use the proper key for pixel format - NSString version
            let key = NSString::from_str("PixelFormatType");
            let pixel_format = NSNumber::new_u32(kCVPixelFormatType_32BGRA);

            // Build settings dictionary using dictionaryWithObject:forKey:
            let dict_class = objc2::class!(NSDictionary);
            let settings: Retained<objc2_foundation::NSDictionary<NSString>> = msg_send![
                dict_class,
                dictionaryWithObject: &*pixel_format,
                forKey: &*key
            ];
            output.setVideoSettings(Some(&*settings));
        }

        // Don't drop late frames (we want the latest)
        // SAFETY: Setting alwaysDiscardsLateVideoFrames is safe
        unsafe {
            output.setAlwaysDiscardsLateVideoFrames(true);
        }

        // Create delegate
        let delegate = CaptureDelegate::new(Arc::clone(&self.buffer));

        // Set delegate with queue
        // We must provide a valid dispatch queue for sample buffer callbacks
        // SAFETY: setSampleBufferDelegate:queue: with valid queue
        unsafe {
            let delegate_protocol: &ProtocolObject<
                dyn AVCaptureVideoDataOutputSampleBufferDelegate,
            > = ProtocolObject::from_ref(&*delegate);

            // Create a serial dispatch queue for camera callbacks
            let queue_label = std::ffi::CString::new("sotf.head-tracker.camera").unwrap();
            let queue = dispatch::ffi::dispatch_queue_create(
                queue_label.as_ptr(),
                dispatch::ffi::DISPATCH_QUEUE_SERIAL,
            );

            // Use raw objc_msgSend since dispatch_queue_t doesn't implement objc2's Encode trait
            // Transmute objc_msgSend to the correct function signature
            type MsgSendFn = unsafe extern "C" fn(
                *const AVCaptureVideoDataOutput,
                objc2::runtime::Sel,
                *const std::ffi::c_void, // delegate
                *mut std::ffi::c_void,   // queue
            );
            let send_fn: MsgSendFn = std::mem::transmute(objc2::ffi::objc_msgSend as *const ());
            let sel = objc2::sel!(setSampleBufferDelegate:queue:);
            send_fn(
                &*output as *const _,
                sel,
                delegate_protocol as *const _ as *const std::ffi::c_void,
                queue as *mut std::ffi::c_void,
            );
        }

        // Configure session
        // SAFETY: beginConfiguration/commitConfiguration are safe
        unsafe {
            session.beginConfiguration();

            if session.canAddInput(&input) {
                session.addInput(&input);
            } else {
                session.commitConfiguration();
                return Err(HeadTrackerError::Camera(
                    "Cannot add input to session".to_string(),
                ));
            }

            if session.canAddOutput(&output) {
                session.addOutput(&output);
            } else {
                session.commitConfiguration();
                return Err(HeadTrackerError::Camera(
                    "Cannot add output to session".to_string(),
                ));
            }

            session.commitConfiguration();
        }

        // Set start time
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        self.buffer.start_time_ms.store(now_ms, Ordering::Relaxed);

        // Start running
        // SAFETY: startRunning is safe
        unsafe {
            session.startRunning();
        }

        // Wait a moment for the camera to warm up and verify it's running
        std::thread::sleep(std::time::Duration::from_millis(100));
        let running = unsafe { session.isRunning() };
        info!("AVFoundation session running: {}", running);

        if !running {
            return Err(HeadTrackerError::Camera(
                "AVCaptureSession failed to start".to_string(),
            ));
        }

        self.session = Some(session);
        self.delegate = Some(delegate);
        self.is_running.store(true, Ordering::Relaxed);

        info!("AVFoundation camera started successfully");
        Ok(())
    }

    /// Stop camera capture
    pub fn stop(&mut self) {
        if let Some(session) = &self.session {
            info!("Stopping AVFoundation camera");
            // SAFETY: stopRunning is safe
            unsafe {
                session.stopRunning();
            }
        }
        self.session = None;
        self.delegate = None;
        self.is_running.store(false, Ordering::Relaxed);
    }

    /// Check if capture is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::Relaxed)
    }

    /// Get the latest captured frame (non-blocking)
    pub fn get_frame(&self) -> Option<CameraFrame> {
        // Drain queue and return latest
        let mut latest = None;
        while let Some(frame) = self.buffer.queue.pop() {
            latest = Some(frame);
        }
        latest
    }

    /// Wait for a frame with timeout
    pub fn capture_frame(&self, timeout_ms: u64) -> Result<CameraFrame, HeadTrackerError> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(timeout_ms);

        loop {
            if let Some(frame) = self.get_frame() {
                return Ok(frame);
            }

            if start.elapsed() > timeout {
                return Err(HeadTrackerError::Camera(
                    "Frame capture timeout".to_string(),
                ));
            }

            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

impl Drop for AVFoundationCapture {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capture_creation() {
        let capture = AVFoundationCapture::new(0);
        assert!(!capture.is_running());
    }
}
