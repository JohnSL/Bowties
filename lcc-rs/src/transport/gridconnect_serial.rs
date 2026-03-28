//! GridConnect serial transport for USB-to-CAN adapters
//!
//! Supports devices like RR-Cirkits Buffer LCC, CAN2USBINO, and CANRS that use
//! the GridConnect framing protocol over a serial (USB CDC) connection.
//! No init sequence is required.

use crate::{Error, Result, protocol::GridConnectFrame};
use crate::transport::{LccTransport, TransportReader, TransportWriter};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio_serial::{SerialStream, SerialPortBuilderExt, SerialPort};

/// GridConnect serial transport for USB-to-CAN adapters
///
/// Communicates using GridConnect framing (`:X<header>N<data>;`) over a serial
/// port. No init sequence is required — the adapter is ready to use immediately
/// after the port is opened.
pub struct GridConnectSerialTransport {
    port: SerialStream,
    read_buf: Vec<u8>,
}

// SAFETY: SerialStream's underlying Windows HANDLE is thread-safe. All serial
// access is serialized via `&mut self`, and in practice via Arc<Mutex<...>>.
unsafe impl Sync for GridConnectSerialTransport {}

impl GridConnectSerialTransport {
    /// Open a serial port and return the ready transport.
    ///
    /// # Arguments
    /// * `path` — Serial port path (e.g. `"COM3"` on Windows, `"/dev/ttyUSB0"` on Linux)
    /// * `baud_rate` — Serial baud rate (e.g. 57600 for Buffer LCC)
    pub async fn open(path: &str, baud_rate: u32) -> Result<Self> {
        let mut port = tokio_serial::new(path, baud_rate)
            .data_bits(tokio_serial::DataBits::Eight)
            .stop_bits(tokio_serial::StopBits::One)
            .parity(tokio_serial::Parity::None)
            .flow_control(tokio_serial::FlowControl::None)
            .open_native_async()
            .map_err(|e| Error::Transport(format!("Failed to open serial port {}: {}", path, e)))?;

        // Assert RTS and DTR so USB-CDC bridge firmware knows the host is ready.
        // JMRI's GcSerialDriverAdapter calls setRTS()+setDTR() (both true) with
        // FlowControl=NONE for all GridConnect adapters including the Buffer LCC.
        // Without RTS asserted, the Buffer LCC will not forward frames to CAN.
        if let Err(e) = port.write_request_to_send(true) {
            eprintln!("Warning: Could not assert RTS on {}: {}", path, e);
        }
        if let Err(e) = port.write_data_terminal_ready(true) {
            eprintln!("Warning: Could not assert DTR on {}: {}", path, e);
        }

        Ok(Self {
            port,
            read_buf: Vec::with_capacity(128),
        })
    }

    /// Inner receive loop — reads bytes until a complete GridConnect frame is found.
    async fn receive_inner(&mut self) -> Result<Option<GridConnectFrame>> {
        loop {
            let mut byte = [0u8; 1];
            match self.port.read_exact(&mut byte).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(Error::ConnectionClosed);
                }
                Err(e) => return Err(Error::Io(e)),
            }

            self.read_buf.push(byte[0]);

            if byte[0] == b';' {
                // Find the last ':' to locate the frame start (ignore preceding noise/garbage)
                if let Some(start) = self.read_buf.iter().rposition(|&b| b == b':') {
                    let frame_bytes = self.read_buf[start..].to_vec();
                    self.read_buf.clear();

                    match String::from_utf8(frame_bytes) {
                        Ok(frame_str) => match GridConnectFrame::parse(&frame_str) {
                            Ok(frame) => return Ok(Some(frame)),
                            Err(e) => {
                                eprintln!("Warning: Failed to parse GridConnect frame: {}", e);
                                continue;
                            }
                        },
                        Err(_) => {
                            // Invalid UTF-8 — skip
                            continue;
                        }
                    }
                } else {
                    // No ':' found — clear garbage and continue
                    self.read_buf.clear();
                }
            }

            // Prevent unbounded buffer growth from line noise
            if self.read_buf.len() > 1024 {
                self.read_buf.clear();
            }
        }
    }
}

#[async_trait::async_trait]
impl LccTransport for GridConnectSerialTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let frame_str = format!("{}\r\n", frame.to_string());
        self.port.write_all(frame_str.as_bytes()).await?;
        self.port.flush().await?;
        Ok(())
    }

    async fn receive(&mut self, timeout_ms: u64) -> Result<Option<GridConnectFrame>> {
        if timeout_ms > 0 {
            match tokio::time::timeout(
                tokio::time::Duration::from_millis(timeout_ms),
                self.receive_inner(),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Ok(None),
            }
        } else {
            self.receive_inner().await
        }
    }

    async fn close(&mut self) -> Result<()> {
        // Port is closed on drop
        Ok(())
    }

    fn into_halves(self: Box<Self>) -> (Box<dyn TransportReader>, Box<dyn TransportWriter>) {
        let (reader, writer) = tokio::io::split(self.port);
        (
            Box::new(GridConnectSerialReader {
                reader,
                read_buf: self.read_buf,
            }),
            Box::new(GridConnectSerialWriter { writer }),
        )
    }
}

/// Read half of a split GridConnect serial transport.
pub struct GridConnectSerialReader {
    reader: ReadHalf<SerialStream>,
    read_buf: Vec<u8>,
}

#[async_trait::async_trait]
impl TransportReader for GridConnectSerialReader {
    async fn receive(&mut self) -> Result<GridConnectFrame> {
        loop {
            let mut byte = [0u8; 1];
            match self.reader.read_exact(&mut byte).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Err(Error::ConnectionClosed);
                }
                Err(e) => return Err(Error::Io(e)),
            }

            self.read_buf.push(byte[0]);

            if byte[0] == b';' {
                if let Some(start) = self.read_buf.iter().rposition(|&b| b == b':') {
                    let frame_bytes = self.read_buf[start..].to_vec();
                    self.read_buf.clear();

                    match String::from_utf8(frame_bytes) {
                        Ok(frame_str) => match GridConnectFrame::parse(&frame_str) {
                            Ok(frame) => return Ok(frame),
                            Err(e) => {
                                eprintln!("Warning: Failed to parse GridConnect frame: {}", e);
                                continue;
                            }
                        },
                        Err(_) => continue,
                    }
                } else {
                    self.read_buf.clear();
                }
            }

            if self.read_buf.len() > 1024 {
                self.read_buf.clear();
            }
        }
    }
}

/// Write half of a split GridConnect serial transport.
pub struct GridConnectSerialWriter {
    writer: WriteHalf<SerialStream>,
}

#[async_trait::async_trait]
impl TransportWriter for GridConnectSerialWriter {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let frame_str = format!("{}\r\n", frame.to_string());
        self.writer.write_all(frame_str.as_bytes()).await?;
        self.writer.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        Ok(())
    }
}
