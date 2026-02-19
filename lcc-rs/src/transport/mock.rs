//! Mock transport for testing

use crate::protocol::GridConnectFrame;
use crate::transport::LccTransport;
use crate::Error;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Mock transport for testing
pub struct MockTransport {
    receive_queue: Arc<Mutex<VecDeque<String>>>,
    sent_frames: Arc<Mutex<Vec<String>>>,
}

impl MockTransport {
    /// Create a new mock transport
    pub fn new() -> Self {
        Self {
            receive_queue: Arc::new(Mutex::new(VecDeque::new())),
            sent_frames: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a frame to be received
    pub fn add_receive_frame(&mut self, frame: String) {
        self.receive_queue.lock().unwrap().push_back(frame);
    }

    /// Get all sent frames
    pub fn get_sent_frames(&self) -> Vec<String> {
        self.sent_frames.lock().unwrap().clone()
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LccTransport for MockTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<(), Error> {
        self.sent_frames.lock().unwrap().push(frame.to_string());
        Ok(())
    }

    async fn receive(&mut self, _timeout_ms: u64) -> Result<Option<GridConnectFrame>, Error> {
        let frame_str = self.receive_queue.lock().unwrap().pop_front();
        
        match frame_str {
            Some(s) => {
                let frame = GridConnectFrame::parse(&s)?;
                Ok(Some(frame))
            }
            None => {
                // Simulate timeout
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                Ok(None)
            }
        }
    }

    async fn close(&mut self) -> Result<(), Error> {
        Ok(())
    }
}
