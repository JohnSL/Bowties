//! SLCAN (Lawicel) serial transport for USB-to-CAN adapters
//!
//! Supports devices like Canable, Lawicel CANUSB, and any slcand-compatible adapter.
//! Uses `T<08X><len><data_hex>\r` framing for extended CAN frames and sends an init
//! sequence on open: `V\r` (version), `S4\r` (125 kbps CAN), `O\r` (open channel).

use crate::{Error, Result, protocol::GridConnectFrame};
use crate::transport::LccTransport;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::{SerialStream, SerialPortBuilderExt};

/// SLCAN serial transport for USB-to-CAN adapters
///
/// Communicates using SLCAN framing over a serial port.
/// The `open()` constructor sends the required init sequence automatically.
pub struct SlcanSerialTransport {
    port: SerialStream,
    read_buf: Vec<u8>,
}

// SAFETY: SerialStream's underlying Windows HANDLE is thread-safe. All serial
// access is serialized via `&mut self`, and in practice via Arc<Mutex<...>>.
unsafe impl Sync for SlcanSerialTransport {}

impl SlcanSerialTransport {
    /// Open a serial port and send the SLCAN init sequence.
    ///
    /// Init sequence:
    /// 1. `V\r` — request version (adapter may echo a version string)
    /// 2. `S4\r` — set CAN bus to 125 kbps (LCC standard)
    /// 3. `O\r` — open the CAN channel
    ///
    /// # Arguments
    /// * `path` — Serial port path (e.g. `"COM4"` on Windows, `"/dev/ttyACM0"` on Linux)
    /// * `baud_rate` — Serial baud rate (e.g. 115200 for Canable/Lawicel)
    pub async fn open(path: &str, baud_rate: u32) -> Result<Self> {
        let mut port = tokio_serial::new(path, baud_rate)
            .open_native_async()
            .map_err(|e| Error::Transport(format!("Failed to open serial port {}: {}", path, e)))?;

        // Send SLCAN init sequence with brief pauses between commands
        port.write_all(b"V\r").await?;
        port.flush().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        port.write_all(b"S4\r").await?;
        port.flush().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        port.write_all(b"O\r").await?;
        port.flush().await?;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        Ok(Self {
            port,
            read_buf: Vec::with_capacity(128),
        })
    }

    /// Encode a `GridConnectFrame` as an SLCAN extended frame string.
    ///
    /// Format: `T<08X><len><data_hex>\r`
    /// The GridConnect header IS the 29-bit CAN extended ID.
    fn encode_slcan(frame: &GridConnectFrame) -> String {
        let data_hex: String = frame.data.iter().map(|b| format!("{:02X}", b)).collect();
        format!("T{:08X}{}{}\r", frame.header, frame.data.len(), data_hex)
    }

    /// Decode an SLCAN extended frame string to a `GridConnectFrame`.
    ///
    /// * Accepts `T<08X><len><data_hex>` format.
    /// * Returns `Ok(None)` for ack echoes (`z`/`Z`), version responses (`V`), and
    ///   other non-data lines so the caller can silently skip them.
    fn decode_slcan(line: &str) -> Result<Option<GridConnectFrame>> {
        let line = line.trim();

        if line.is_empty() {
            return Ok(None);
        }

        // Silently skip ack echoes and version/status responses
        let first = line.chars().next().unwrap_or('\0');
        match first {
            'z' | 'Z' | 'V' | 'v' | 'F' => return Ok(None),
            _ => {}
        }

        // Only handle extended (29-bit) frames: T<08X><len><data_hex>
        if first != 'T' || line.len() < 10 {
            return Ok(None);
        }

        // Parse 8-char CAN ID
        let can_id = u32::from_str_radix(&line[1..9], 16)
            .map_err(|_| Error::Parse(format!("Invalid SLCAN CAN ID in: {}", line)))?;

        // Parse 1-char DLC (0-8)
        let dlc = usize::from_str_radix(&line[9..10], 16)
            .map_err(|_| Error::Parse(format!("Invalid SLCAN DLC in: {}", line)))?;

        // Parse data bytes
        let data_start = 10;
        let data_end = data_start + dlc * 2;
        if line.len() < data_end {
            return Err(Error::Parse(format!("SLCAN frame too short: {}", line)));
        }

        let mut data = Vec::with_capacity(dlc);
        for i in 0..dlc {
            let offset = data_start + i * 2;
            let byte = u8::from_str_radix(&line[offset..offset + 2], 16)
                .map_err(|_| Error::Parse(format!("Invalid byte in SLCAN data: {}", line)))?;
            data.push(byte);
        }

        // The SLCAN CAN extended ID IS the GridConnect header (29-bit value)
        GridConnectFrame::new(can_id, data).map(Some)
    }

    /// Inner receive loop — reads bytes until `\r` and attempts SLCAN decode.
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

            let ch = byte[0];

            if ch == b'\r' {
                let line = String::from_utf8_lossy(&self.read_buf).to_string();
                self.read_buf.clear();

                match Self::decode_slcan(&line) {
                    Ok(Some(frame)) => return Ok(Some(frame)),
                    Ok(None) => continue, // Ack echo, version response — skip
                    Err(e) => {
                        eprintln!("Warning: Failed to decode SLCAN frame '{}': {}", line, e);
                        continue;
                    }
                }
            } else if ch != b'\n' {
                // Ignore bare LF (only CR terminates SLCAN frames)
                self.read_buf.push(ch);

                // Prevent unbounded buffer growth from line noise
                if self.read_buf.len() > 512 {
                    self.read_buf.clear();
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl LccTransport for SlcanSerialTransport {
    async fn send(&mut self, frame: &GridConnectFrame) -> Result<()> {
        let slcan_str = Self::encode_slcan(frame);
        self.port.write_all(slcan_str.as_bytes()).await?;
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
        // Send CAN channel close command before the port is dropped
        let _ = self.port.write_all(b"C\r").await;
        let _ = self.port.flush().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::MTI;

    #[test]
    fn test_encode_slcan_no_data() {
        let frame = GridConnectFrame::from_mti(MTI::VerifyNodeGlobal, 0xAAA, vec![]).unwrap();
        let encoded = SlcanSerialTransport::encode_slcan(&frame);
        // header 0x19490AAA → T19490AAA0\r
        assert_eq!(encoded, "T19490AAA0\r");
    }

    #[test]
    fn test_encode_slcan_with_data() {
        let frame = GridConnectFrame::new(0x19170123, vec![0x01, 0x02]).unwrap();
        let encoded = SlcanSerialTransport::encode_slcan(&frame);
        assert_eq!(encoded, "T191701232 0102\r".replace(' ', ""));
    }

    #[test]
    fn test_decode_slcan_no_data() {
        let frame = SlcanSerialTransport::decode_slcan("T19490AAA0")
            .unwrap()
            .unwrap();
        assert_eq!(frame.header, 0x19490AAA);
        assert!(frame.data.is_empty());
    }

    #[test]
    fn test_decode_slcan_with_data() {
        let frame = SlcanSerialTransport::decode_slcan("T1917012320102")
            .unwrap()
            .unwrap();
        assert_eq!(frame.header, 0x19170123);
        assert_eq!(frame.data, vec![0x01, 0x02]);
    }

    #[test]
    fn test_decode_slcan_skip_ack() {
        assert!(SlcanSerialTransport::decode_slcan("z").unwrap().is_none());
        assert!(SlcanSerialTransport::decode_slcan("Z").unwrap().is_none());
    }

    #[test]
    fn test_decode_slcan_skip_version() {
        assert!(SlcanSerialTransport::decode_slcan("V1010").unwrap().is_none());
    }

    #[test]
    fn test_roundtrip() {
        let original = GridConnectFrame::new(0x195B4AAA, vec![0xDE, 0xAD, 0xBE, 0xEF]).unwrap();
        let encoded = SlcanSerialTransport::encode_slcan(&original);
        // Strip trailing \r for decode_slcan
        let decoded = SlcanSerialTransport::decode_slcan(encoded.trim_end_matches('\r'))
            .unwrap()
            .unwrap();
        assert_eq!(decoded.header, original.header);
        assert_eq!(decoded.data, original.data);
    }
}
