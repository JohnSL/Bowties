//! Optional Interaction Rejected (OIR) payload parsing.
//!
//! Wire format per S-9.7.3 §3.3.4: the addressed-destination-alias prefix
//! (2 bytes, already consumed by frame addressing) is followed by
//! `error code` (2 bytes) and `rejected MTI` (2 bytes), matching OpenLCB
//! Java's `OptionalIntRejectedMessage.toPayload` (destination alias, then
//! `HostToNetworkUint16(code)`, then `HostToNetworkUint16(mti)`) and the
//! decode in `MessageBuilder.java`'s `OptionalInteractionRejected` case.

/// Parse an OIR frame's data payload into `(rejected_mti, error_code)`.
///
/// `data` is the full frame payload starting at the destination-alias
/// prefix (i.e. callers pass `&frame.data` directly) — this function skips
/// `data[0..2]` internally and reads the error code from `data[2..4]` and
/// the rejected MTI from `data[4..6]`. Any bytes beyond `data[6]` (optional
/// info) are ignored.
///
/// Truncated payloads degrade gracefully, matching the peer-cleanup
/// contract that must always be able to complete the exchange even from a
/// malformed or short OIR:
/// - `data.len() >= 6`: full payload — `(rejected_mti, error_code)`.
/// - `4 <= data.len() < 6`: error code only — `(0, error_code)`.
/// - `data.len() < 4`: neither present — `(0, 0)`.
pub fn parse_oir_payload(data: &[u8]) -> (u32, u16) {
    if data.len() >= 6 {
        let code = ((data[2] as u16) << 8) | data[3] as u16;
        let mti = ((data[4] as u32) << 8) | data[5] as u32;
        (mti, code)
    } else if data.len() >= 4 {
        let code = ((data[2] as u16) << 8) | data[3] as u16;
        (0u32, code)
    } else {
        (0u32, 0u16)
    }
}

/// Build an OIR frame's data payload from `(dest_alias, error_code,
/// rejected_mti)`.
///
/// Mirrors OpenLCB Java's `OptionalIntRejectedMessage.toPayload` (S-9.7.3
/// §3.3.4): destination alias first (top nibble zeroed per the addressed-
/// message convention), then `HostToNetworkUint16(code)`, then
/// `HostToNetworkUint16(mti)`. The returned bytes are exactly what
/// `parse_oir_payload` consumes (it is called with `&frame.data`, which
/// includes this same destination-alias prefix), so the two functions form
/// a round-trip pair at the frame-payload seam.
///
/// Note the parameter order `(dest_alias, error_code, rejected_mti)` follows
/// wire byte order, which is the reverse of `parse_oir_payload`'s return
/// tuple `(rejected_mti, error_code)` — that return order is fixed by the
/// existing call-site destructures (`let (wrapped_mti, error_code) = ...`)
/// and is not changed here.
pub fn build_oir_payload(dest_alias: u16, error_code: u16, rejected_mti: u16) -> Vec<u8> {
    vec![
        ((dest_alias >> 8) & 0x0F) as u8,
        (dest_alias & 0xFF) as u8,
        (error_code >> 8) as u8,
        (error_code & 0xFF) as u8,
        (rejected_mti >> 8) as u8,
        (rejected_mti & 0xFF) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full 6+ byte payload with distinct values matching a canonical
    /// JMRI-emitted frame (S-9.7.3 §3.5.5 / S-9.7.3.2 §4.2).
    #[test]
    fn parses_full_payload() {
        let data = [0x01, 0x02, 0x10, 0x40, 0x0A, 0x28];
        assert_eq!(parse_oir_payload(&data), (0x0A28, 0x1040));
    }

    /// 4-byte truncated payload: only the error code is present.
    #[test]
    fn parses_truncated_error_code_only() {
        let data = [0x01, 0x02, 0x10, 0x40];
        assert_eq!(parse_oir_payload(&data), (0, 0x1040));
    }

    /// Fewer than 4 bytes: neither field is present.
    #[test]
    fn parses_empty_below_minimum_length() {
        let data = [0x01, 0x02];
        assert_eq!(parse_oir_payload(&data), (0, 0));
    }

    /// Longer-than-6-byte payload: trailing optional-info bytes are ignored.
    #[test]
    fn ignores_trailing_optional_info_bytes() {
        let data = [0x01, 0x02, 0x10, 0x40, 0x0A, 0x28, 0xFF, 0xEE, 0xDD];
        assert_eq!(parse_oir_payload(&data), (0x0A28, 0x1040));
    }

    /// Encoded bytes match OpenLCB Java's `toPayload` byte order exactly,
    /// and round-tripping through `parse_oir_payload` recovers the inputs.
    #[test]
    fn builds_payload_round_trips_through_parse() {
        let payload = build_oir_payload(0x123, 0x1040, 0x0A28);
        assert_eq!(payload, vec![0x01, 0x23, 0x10, 0x40, 0x0A, 0x28]);
        assert_eq!(parse_oir_payload(&payload), (0x0A28, 0x1040));
    }

    /// The top nibble of the destination alias is masked off per the
    /// addressed-message convention, regardless of what's passed in.
    #[test]
    fn builds_payload_masks_dest_alias_top_nibble() {
        let payload = build_oir_payload(0xF123, 0x1040, 0x0A28);
        assert_eq!(payload[0], 0x01);
    }

    /// The payload is always exactly 6 bytes (no optional info appended).
    #[test]
    fn builds_payload_length_is_six() {
        let payload = build_oir_payload(0x123, 0x1040, 0x0A28);
        assert_eq!(payload.len(), 6);
    }

    /// `parse_oir_payload(build_oir_payload(dest, code, mti))` recovers
    /// `(mti, code)` for a handful of arbitrary values.
    #[test]
    fn round_trips_arbitrary_values() {
        let cases = [
            (0x000u16, 0x0000u16, 0x0000u16),
            (0x7FFu16, 0xFFFFu16, 0xFFFFu16),
            (0x321u16, 0x1234u16, 0x5678u16),
        ];
        for (dest, code, mti) in cases {
            let payload = build_oir_payload(dest, code, mti);
            assert_eq!(parse_oir_payload(&payload), (mti as u32, code));
        }
    }
}
