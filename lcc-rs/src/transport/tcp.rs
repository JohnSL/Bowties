//! TCP transport implementation for LCC/OpenLCB
//!
//! Provides async TCP connection to LCC networks using GridConnect protocol.

use crate::{Error, Result, protocol::GridConnectFrame};
use socket2::SockRef;
use std::pin::Pin;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
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

    /// Split into independent read and write halves for concurrent use.
    ///
    /// The returned halves each own their side of the underlying I/O resource,
    /// eliminating the need for a shared mutex.
    fn into_halves(self: Box<Self>) -> (Box<dyn TransportReader>, Box<dyn TransportWriter>) {
        unimplemented!("This transport does not support splitting into halves")
    }
}

/// Read half of a split transport — blocks until a frame arrives.
#[async_trait::async_trait]
pub trait TransportReader: Send {
    /// Receive a single frame. Blocks until data arrives or an error (including
    /// ConnectionClosed) occurs. No timeout — the caller uses `tokio::select!`
    /// with a shutdown signal instead.
    async fn receive(&mut self) -> Result<GridConnectFrame>;
}

/// Write half of a split transport.
#[async_trait::async_trait]
pub trait TransportWriter: Send {
    /// Send a GridConnect frame.
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()>;
    /// Close the transport.
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

        // Enable TCP keepalive so the OS detects dead connections (e.g. when a
        // Wi-Fi bridge silently drops off the network). Without this, a broken
        // connection can hang indefinitely inside a blocking read.
        let sock = SockRef::from(&stream);
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(Duration::from_secs(60));
        // with_interval is not available on all platforms; ignore the error
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        let keepalive = keepalive.with_interval(Duration::from_secs(15));
        sock.set_tcp_keepalive(&keepalive)?;
        
        Ok(Self {
            stream: BufReader::new(stream),
            buffer: String::with_capacity(64),
        })
    }

    /// Extract and parse a complete GridConnect line ending at `newline_pos`
    /// from the accumulation buffer.
    fn extract_line(&mut self, newline_pos: usize) -> Result<Option<GridConnectFrame>> {
        let line = self.buffer[..newline_pos].trim().to_string();
        self.buffer.drain(..=newline_pos);
        if line.is_empty() {
            return Ok(None);
        }
        match GridConnectFrame::parse(&line) {
            Ok(frame) => Ok(Some(frame)),
            Err(e) => {
                eprintln!("Warning: Failed to parse frame '{}': {}", line, e);
                Ok(None)
            }
        }
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
        // Fast path: if previous fill_buf() calls left multiple frames in our
        // accumulation buffer, return the next one without touching the stream.
        if let Some(pos) = self.buffer.find('\n') {
            return self.extract_line(pos);
        }

        // Fill BufReader's internal buffer from the TCP stream.
        //
        // fill_buf() is cancellation-safe: any bytes already read into
        // BufReader's internal buffer persist if the future is cancelled by
        // a timeout — unlike read_line, whose *private* internal Vec<u8> is
        // dropped along with the future, permanently losing bytes that
        // BufReader already consumed from the stream.  That silent data loss
        // caused byte-drop corruption on TCP (but not CAN serial, which
        // reads one byte at a time via read_exact).
        let fill_result = if timeout_ms > 0 {
            timeout(
                Duration::from_millis(timeout_ms),
                self.stream.fill_buf(),
            )
            .await
        } else {
            Ok(self.stream.fill_buf().await)
        };

        match fill_result {
            Ok(Ok(available)) if available.is_empty() => {
                // EOF — connection closed.
                self.buffer.clear();
                Err(Error::ConnectionClosed)
            }
            Ok(Ok(available)) => {
                // Copy data out of BufReader (to release the immutable borrow)
                // then advance BufReader's cursor.
                let chunk = available.to_vec();
                Pin::new(&mut self.stream).consume(chunk.len());

                let text = std::str::from_utf8(&chunk)
                    .map_err(|_| Error::Transport("Invalid UTF-8 in TCP stream".to_string()))?;
                self.buffer.push_str(text);

                if let Some(pos) = self.buffer.find('\n') {
                    self.extract_line(pos)
                } else {
                    // Partial data — no complete line yet.
                    Ok(None)
                }
            }
            Ok(Err(e)) => {
                self.buffer.clear();
                Err(Error::Io(e))
            }
            Err(_) => {
                // Timeout — no data lost thanks to fill_buf's cancellation
                // safety guarantee.
                Ok(None)
            }
        }
    }
    
    async fn close(&mut self) -> Result<()> {
        self.stream.get_mut().shutdown().await?;
        Ok(())
    }

    fn into_halves(self: Box<Self>) -> (Box<dyn TransportReader>, Box<dyn TransportWriter>) {
        // Capture any bytes buffered in BufReader that haven't been consumed yet.
        let pending_bytes = self.stream.buffer().to_vec();
        let tcp_stream = self.stream.into_inner();
        let (read_half, write_half) = tcp_stream.into_split();

        // Build accumulation buffer: existing partial frame + any pending BufReader bytes.
        let mut buffer = self.buffer;
        if !pending_bytes.is_empty() {
            if let Ok(s) = std::str::from_utf8(&pending_bytes) {
                buffer.push_str(s);
            }
        }

        let reader = TcpTransportReader {
            stream: BufReader::new(read_half),
            buffer,
        };
        let writer = TcpTransportWriter { stream: write_half };
        (Box::new(reader), Box::new(writer))
    }
}

/// Read half of a split TCP transport.
pub struct TcpTransportReader {
    stream: BufReader<OwnedReadHalf>,
    buffer: String,
}

impl TcpTransportReader {
    /// Extract and parse a complete GridConnect line ending at `newline_pos`.
    fn extract_line(&mut self, newline_pos: usize) -> Result<Option<GridConnectFrame>> {
        let line = self.buffer[..newline_pos].trim().to_string();
        self.buffer.drain(..=newline_pos);
        if line.is_empty() {
            return Ok(None);
        }
        match GridConnectFrame::parse(&line) {
            Ok(frame) => Ok(Some(frame)),
            Err(e) => {
                eprintln!("Warning: Failed to parse frame '{}': {}", line, e);
                Ok(None)
            }
        }
    }
}

#[async_trait::async_trait]
impl TransportReader for TcpTransportReader {
    async fn receive(&mut self) -> Result<GridConnectFrame> {
        loop {
            // Fast path: complete frame already in buffer.
            if let Some(pos) = self.buffer.find('\n') {
                if let Some(frame) = self.extract_line(pos)? {
                    return Ok(frame);
                }
                continue;
            }

            // Block until data arrives from the network.
            let available = self.stream.fill_buf().await?;
            if available.is_empty() {
                return Err(Error::ConnectionClosed);
            }

            let chunk = available.to_vec();
            Pin::new(&mut self.stream).consume(chunk.len());

            let text = std::str::from_utf8(&chunk)
                .map_err(|_| Error::Transport("Invalid UTF-8 in TCP stream".to_string()))?;
            self.buffer.push_str(text);
        }
    }
}

/// Write half of a split TCP transport.
pub struct TcpTransportWriter {
    stream: OwnedWriteHalf,
}

#[async_trait::async_trait]
impl TransportWriter for TcpTransportWriter {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let frame_str = frame.to_string();
        self.stream.write_all(frame_str.as_bytes()).await?;
        self.stream.write_all(b"\n").await?;
        self.stream.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.stream.shutdown().await?;
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

    /// Regression test: partial frame data in the accumulation buffer is
    /// correctly completed when the rest of the frame arrives.
    ///
    /// The previous implementation used `BufReader::read_line` wrapped in a
    /// timeout, which is not cancellation-safe.  When the timeout fired
    /// mid-read, bytes consumed from BufReader into the future's private
    /// `Vec<u8>` were silently lost — the output `String` was never written.
    /// This caused byte-drop corruption on TCP (but not CAN serial, which
    /// reads one byte at a time via `read_exact`).
    ///
    /// The current implementation uses `fill_buf()`/`consume()`, which IS
    /// cancellation-safe: bytes stay in BufReader's internal buffer until we
    /// explicitly consume them.
    ///
    /// This test pre-populates `self.buffer` with a partial frame and sends
    /// the remaining bytes from the server, verifying the frame is correctly
    /// reassembled.
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

    /// When the server sends multiple frames in one TCP segment, fill_buf()
    /// returns all of them at once.  The first receive() should return the
    /// first frame and buffer the rest; subsequent calls should return
    /// buffered frames without hitting the network.
    #[tokio::test]
    async fn test_multiple_frames_in_single_segment() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            let (mut server, _) = listener.accept().await.unwrap();
            // Send two frames in a single write (single TCP segment).
            server.write_all(b":X19490AAAN;\n:X195B4BBBN010203;\n").await.unwrap();
            server.flush().await.unwrap();
        });

        let stream = TcpStream::connect(addr).await.unwrap();
        stream.set_nodelay(true).unwrap();
        let mut transport = TcpTransport {
            stream: BufReader::new(stream),
            buffer: String::with_capacity(64),
        };

        let frame1 = transport.receive(500).await.unwrap();
        assert!(frame1.is_some(), "First frame should be returned");
        assert_eq!(frame1.unwrap().to_string(), ":X19490AAAN;");

        // Second frame should come from the accumulation buffer (no network I/O).
        let frame2 = transport.receive(500).await.unwrap();
        assert!(frame2.is_some(), "Second frame should be available from buffer");
        assert_eq!(frame2.unwrap().to_string(), ":X195B4BBBN010203;");
    }
}
