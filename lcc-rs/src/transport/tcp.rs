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
        // NOTE: do NOT clear self.buffer here.
        //
        // BufReader::read_line is not cancellation-safe: when the timeout fires
        // mid-read, it drops the future after already consuming bytes from the
        // BufReader's internal buffer and appending them to self.buffer.  If we
        // cleared the buffer at entry, those bytes would be permanently lost on
        // the next call and the stream would be misaligned, causing silent frame
        // drops.  Instead we preserve the partial content so the next call picks
        // up where this one left off — identical to how GridConnectSerialTransport
        // accumulates bytes in read_buf across calls.

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
                // Connection closed — reset reassembly state
                self.buffer.clear();
                Err(Error::ConnectionClosed)
            }
            Ok(Ok(_)) => {
                // Successfully read a complete newline-terminated frame.
                // Extract the line, then clear so the next call starts fresh.
                let line = self.buffer.trim().to_string();
                self.buffer.clear();

                if line.is_empty() {
                    return Ok(None);
                }
                
                match GridConnectFrame::parse(&line) {
                    Ok(frame) => Ok(Some(frame)),
                    Err(e) => {
                        // Log parse error but don't fail - skip invalid frames
                        eprintln!("Warning: Failed to parse frame '{}': {}", line, e);
                        Ok(None)
                    }
                }
            }
            Ok(Err(e)) => {
                // I/O error — reset state so the next call doesn't try to
                // continue from a now-undefined buffer position.
                self.buffer.clear();
                Err(Error::Io(e))
            }
            Err(_) => {
                // Timeout — partial bytes (if any) are preserved in self.buffer
                // so the next call continues seamlessly.
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
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn test_frame_encoding() {
        let frame = GridConnectFrame::from_mti(
            MTI::VerifyNodeGlobal,
            0xAAA,
            vec![],
        ).unwrap();
        assert_eq!(frame.to_string(), ":X19490AAAN;");
    }

    /// Normal path: a complete frame is received and parsed correctly.
    #[tokio::test]
    async fn test_receive_complete_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut server, _) = listener.accept().await.unwrap();
            server.write_all(b":X19490AAAN;\n").await.unwrap();
            server.flush().await.unwrap();
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        stream.set_nodelay(true).unwrap();
        let mut transport = TcpTransport {
            stream: BufReader::new(stream),
            buffer: String::with_capacity(64),
        };

        let result = transport.receive(500).await.unwrap();
        assert!(result.is_some(), "Expected a frame, got None");
        assert_eq!(result.unwrap().to_string(), ":X19490AAAN;");
    }

    /// Regression test for the cancellation-safety bug.
    ///
    /// `BufReader::read_line` is not cancellation-safe: when a timeout fires
    /// mid-read, any bytes already copied from BufReader's internal buffer into
    /// `self.buffer` remain there.  The old code called `self.buffer.clear()`
    /// at the top of `receive()`, which silently discarded those bytes and
    /// misaligned the stream, causing sporadic frame drops over TCP (but not
    /// over CAN, whose serial transport never clears its accumulation buffer).
    ///
    /// This test simulates the state after a partial read (i.e. `self.buffer`
    /// contains the first portion of a frame but no `\n` yet) and verifies
    /// that the *next* `receive()` call completes the frame rather than
    /// discarding the prefix.
    ///
    /// Note: The actual mid-read cancellation is not exercised here because on
    /// Windows the IOCP read model returns `Pending` on the first poll, so the
    /// timeout fires before any bytes reach `self.buffer` — making a
    /// timing-based test unreliable. Testing the invariant directly (preserved
    /// buffer + tail data → complete frame) is equivalent and deterministic.
    #[tokio::test]
    async fn test_partial_frame_completed_after_timeout() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        // Server sends only the tail of the frame (the part after the partial
        // prefix already held in self.buffer).
        tokio::spawn(async move {
            let (mut server, _) = listener.accept().await.unwrap();
            server.write_all(b"N;\n").await.unwrap();
            server.flush().await.unwrap();
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        stream.set_nodelay(true).unwrap();
        let mut transport = TcpTransport {
            stream: BufReader::new(stream),
            buffer: String::with_capacity(64),
        };

        // Pre-populate self.buffer to mimic the state left by a
        // timeout-interrupted read_line call (the prefix was accumulated before
        // the timeout fired and must NOT be cleared at the start of receive()).
        transport.buffer.push_str(":X19490AAA");

        // receive() must append the tail sent by the server and return the
        // complete, correctly parsed frame.
        let result = transport.receive(500).await.unwrap();
        assert!(
            result.is_some(),
            "Expected complete frame after partial-buffer resume, got None"
        );
        assert_eq!(
            result.unwrap().to_string(),
            ":X19490AAAN;",
            "Frame reconstructed from partial buffer + tail should match original"
        );
    }
}
