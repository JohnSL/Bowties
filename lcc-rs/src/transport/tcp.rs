//! TCP transport implementation for LCC/OpenLCB
//!
//! Provides async TCP connection to LCC networks using GridConnect protocol.

use crate::{Error, Result, protocol::GridConnectFrame};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

/// Transport trait for sending and receiving frames
#[async_trait::async_trait]
pub trait LccTransport: Send + Sync {
    /// Send a GridConnect frame
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()>;
    
    /// Receive a GridConnect frame with timeout
    async fn receive(&mut self, timeout_ms: u64) -> Result<Option<GridConnectFrame>>;
    
    /// Close the connection
    async fn close(&mut self) -> Result<()>;
}

/// TCP transport implementation
pub struct TcpTransport {
    stream: BufReader<TcpStream>,
    buffer: String,
}

impl TcpTransport {
    /// Connect to an LCC network via TCP
    /// 
    /// # Arguments
    /// * `host` - Hostname or IP address
    /// * `port` - Port number (typically 12021 for native OpenLCB TCP)
    /// 
    /// # Example
    /// ```no_run
    /// use lcc_rs::transport::TcpTransport;
    /// 
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let mut transport = TcpTransport::connect("localhost", 12021).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect(&addr).await.map_err(|e| {
            Error::Transport(format!("Failed to connect to {}: {}", addr, e))
        })?;
        
        // Set TCP nodelay for lower latency
        stream.set_nodelay(true)?;
        
        Ok(Self {
            stream: BufReader::new(stream),
            buffer: String::with_capacity(64),
        })
    }
}

#[async_trait::async_trait]
impl LccTransport for TcpTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let frame_str = frame.to_string();
        self.stream.get_mut().write_all(frame_str.as_bytes()).await?;
        self.stream.get_mut().write_all(b"\n").await?;
        self.stream.get_mut().flush().await?;
        Ok(())
    }
    
    async fn receive(&mut self, timeout_ms: u64) -> Result<Option<GridConnectFrame>> {
        self.buffer.clear();
        
        let read_result = if timeout_ms > 0 {
            timeout(
                Duration::from_millis(timeout_ms),
                self.stream.read_line(&mut self.buffer),
            )
            .await
        } else {
            Ok(self.stream.read_line(&mut self.buffer).await)
        };
        
        match read_result {
            Ok(Ok(0)) => {
                // Connection closed
                Err(Error::ConnectionClosed)
            }
            Ok(Ok(_)) => {
                // Successfully read a line
                let line = self.buffer.trim();
                if line.is_empty() {
                    return Ok(None);
                }
                
                match GridConnectFrame::parse(line) {
                    Ok(frame) => Ok(Some(frame)),
                    Err(e) => {
                        // Log parse error but don't fail - skip invalid frames
                        eprintln!("Warning: Failed to parse frame '{}': {}", line, e);
                        Ok(None)
                    }
                }
            }
            Ok(Err(e)) => Err(Error::Io(e)),
            Err(_) => {
                // Timeout
                Ok(None)
            }
        }
    }
    
    async fn close(&mut self) -> Result<()> {
        self.stream.get_mut().shutdown().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::MTI;

    // Note: These are unit tests that would require a mock transport
    // For now, we'll test the basic structure

    #[tokio::test]
    async fn test_frame_encoding() {
        // Test that we can create frames for sending
        let frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            0xAAA,
            vec![],
        ).unwrap();
        
        let encoded = frame.to_string();
        assert_eq!(encoded, ":X19490AAAN;");
    }

    // Integration tests would go in tests/ directory
    // and would require either a real LCC network or a mock server
}
