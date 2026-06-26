//! CDI field metadata resolution and value conversion.
//!
//! Pure functions for walking a parsed CDI tree to locate fields by
//! (space, address), and for converting between string representations
//! and raw byte / `ConfigValue` formats.

use crate::node_tree::{ConfigValue, LeafNode, LeafType};
use lcc_rs::cdi::{Cdi, DataElement};

// ── Field metadata ───────────────────────────────────────────────────────────

/// Metadata for a single CDI leaf element, resolved by walking the CDI tree.
#[derive(Debug, Clone)]
pub struct FieldMeta {
    pub leaf_type: LeafType,
    pub size: u32,
    pub field_label: String,
}

// ── Offset parsing ───────────────────────────────────────────────────────────

/// Parse a hex offset string like `"0x00000120"` into a `u32` address.
pub fn parse_offset(offset: &str) -> Option<u32> {
    let trimmed = offset
        .strip_prefix("0x")
        .or_else(|| offset.strip_prefix("0X"))
        .unwrap_or(offset);
    u32::from_str_radix(trimmed, 16).ok()
}

// ── Label helpers ────────────────────────────────────────────────────────────

fn join_label_path(path: &[String], leaf_name: String) -> String {
    let mut parts = path.to_vec();
    parts.push(leaf_name);
    parts.join(".")
}

fn group_label(group: &lcc_rs::cdi::Group, index: usize, instance: Option<u32>) -> String {
    let base = group
        .name
        .clone()
        .unwrap_or_else(|| format!("Group {}", index));
    match instance {
        Some(instance) => format!("{}({})", base, instance),
        None => base,
    }
}

fn int_label(element: &lcc_rs::cdi::IntElement, index: usize) -> String {
    element
        .name
        .clone()
        .unwrap_or_else(|| format!("Int {}", index))
}

fn string_label(element: &lcc_rs::cdi::StringElement, index: usize) -> String {
    element
        .name
        .clone()
        .unwrap_or_else(|| format!("String {}", index))
}

fn event_id_label(element: &lcc_rs::cdi::EventIdElement, index: usize) -> String {
    element
        .name
        .clone()
        .unwrap_or_else(|| format!("EventId {}", index))
}

fn float_label(element: &lcc_rs::cdi::FloatElement, index: usize) -> String {
    element
        .name
        .clone()
        .unwrap_or_else(|| format!("Float {}", index))
}

// ── CDI field search ─────────────────────────────────────────────────────────

/// Walk parsed CDI elements recursively to find a leaf at the given absolute
/// address within the given space.  Returns the leaf's type, size, and label.
pub fn find_field_meta_in_cdi(cdi: &Cdi, space: u8, address: u32) -> Option<FieldMeta> {
    for segment in &cdi.segments {
        if segment.space != space {
            continue;
        }
        let mut path = vec![segment
            .name
            .clone()
            .unwrap_or_else(|| format!("Space {}", segment.space))];
        if let Some(meta) = walk_elements_for_meta(
            &segment.elements,
            segment.origin as i32,
            0,
            address,
            &mut path,
        ) {
            return Some(meta);
        }
    }
    None
}

/// Recursively walk CDI elements using cursor-based addressing to locate a
/// leaf at `target_address`.
fn walk_elements_for_meta(
    elements: &[DataElement],
    segment_origin: i32,
    base_offset: i32,
    target_address: u32,
    path: &mut Vec<String>,
) -> Option<FieldMeta> {
    let mut cursor: i32 = 0;

    for (index, element) in elements.iter().enumerate() {
        match element {
            DataElement::Group(g) => {
                cursor += g.offset;
                let group_start = base_offset + cursor;
                let stride = g.calculate_size();
                let effective_replication =
                    if stride == 0 && g.replication > 1 { 1u32 } else { g.replication };

                for instance in 0..effective_replication {
                    let instance_base = group_start + instance as i32 * stride;
                    path.push(group_label(
                        g,
                        index,
                        if effective_replication > 1 {
                            Some(instance)
                        } else {
                            None
                        },
                    ));
                    if let Some(meta) = walk_elements_for_meta(
                        &g.elements,
                        segment_origin,
                        instance_base,
                        target_address,
                        path,
                    ) {
                        return Some(meta);
                    }
                    let _ = path.pop();
                }
                cursor += effective_replication as i32 * stride;
            }
            DataElement::Int(e) => {
                cursor += e.offset;
                let abs = (segment_origin + base_offset + cursor) as u32;
                if abs == target_address {
                    return Some(FieldMeta {
                        leaf_type: LeafType::Int,
                        size: e.size as u32,
                        field_label: join_label_path(path, int_label(e, index)),
                    });
                }
                cursor += e.size as i32;
            }
            DataElement::String(e) => {
                cursor += e.offset;
                let abs = (segment_origin + base_offset + cursor) as u32;
                if abs == target_address {
                    return Some(FieldMeta {
                        leaf_type: LeafType::String,
                        size: e.size as u32,
                        field_label: join_label_path(path, string_label(e, index)),
                    });
                }
                cursor += e.size as i32;
            }
            DataElement::EventId(e) => {
                cursor += e.offset;
                let abs = (segment_origin + base_offset + cursor) as u32;
                if abs == target_address {
                    return Some(FieldMeta {
                        leaf_type: LeafType::EventId,
                        size: 8,
                        field_label: join_label_path(path, event_id_label(e, index)),
                    });
                }
                cursor += 8;
            }
            DataElement::Float(e) => {
                cursor += e.offset;
                let abs = (segment_origin + base_offset + cursor) as u32;
                if abs == target_address {
                    return Some(FieldMeta {
                        leaf_type: LeafType::Float,
                        size: e.size as u32,
                        field_label: join_label_path(path, float_label(e, index)),
                    });
                }
                cursor += e.size as i32;
            }
            DataElement::Action(e) => {
                cursor += e.offset + 1;
            }
            DataElement::Blob(e) => {
                cursor += e.offset + e.size as i32;
            }
        }
    }
    None
}

// ── Value conversion ─────────────────────────────────────────────────────────

/// Parse raw bytes from a memory read into a string value matching the format
/// used by `ConfigValue::to_snapshot_string`.
pub fn raw_bytes_to_value_string(meta: &FieldMeta, raw: &[u8]) -> Option<String> {
    match meta.leaf_type {
        LeafType::Int => {
            let val = match meta.size {
                1 => raw.first().map(|&b| b as i64),
                2 if raw.len() >= 2 => Some(i16::from_be_bytes([raw[0], raw[1]]) as i64),
                4 if raw.len() >= 4 => {
                    Some(i32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as i64)
                }
                8 if raw.len() >= 8 => Some(i64::from_be_bytes([
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                ])),
                _ => None,
            };
            val.map(|v| v.to_string())
        }
        LeafType::String => {
            let s: String = raw
                .iter()
                .take(meta.size as usize)
                .take_while(|&&b| b != 0)
                .filter(|&&b| b != 0xFF)
                .map(|&b| b as char)
                .collect();
            Some(s)
        }
        LeafType::EventId => {
            if raw.len() >= 8 {
                let bytes: [u8; 8] = raw[..8].try_into().unwrap();
                Some(lcc_rs::EventID::new(bytes).to_canonical())
            } else {
                None
            }
        }
        LeafType::Float => {
            if meta.size == 4 && raw.len() >= 4 {
                let val = f32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
                Some((val as f64).to_string())
            } else if meta.size == 8 && raw.len() >= 8 {
                let val = f64::from_be_bytes([
                    raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7],
                ]);
                Some(val.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse a string value back into a `ConfigValue` using the leaf's type/size metadata.
pub fn string_to_config_value(s: &str, leaf: &LeafNode) -> Option<ConfigValue> {
    match leaf.element_type {
        LeafType::Int => {
            let v: i64 = s.parse().ok()?;
            Some(ConfigValue::Int { value: v })
        }
        LeafType::String => Some(ConfigValue::String {
            value: s.to_string(),
        }),
        LeafType::Float => {
            let v: f64 = s.parse().ok()?;
            Some(ConfigValue::Float { value: v })
        }
        LeafType::EventId => {
            // Accept both canonical contiguous and legacy dotted formats.
            let bytes = crate::node_tree::parse_event_id_hex(s)?;
            let hex = lcc_rs::EventID::new(bytes).to_canonical();
            Some(ConfigValue::EventId { bytes, hex })
        }
        _ => None,
    }
}

/// Build a synthetic `LeafNode` from CDI field metadata, used by
/// `string_to_config_value` and `serialize_config_value` in `apply_sync_changes`.
pub fn field_meta_to_leaf(meta: &FieldMeta, space: u8, address: u32) -> LeafNode {
    LeafNode {
        name: String::new(),
        description: None,
        element_type: meta.leaf_type,
        address,
        size: meta.size,
        space,
        path: Vec::new(),
        value: None,
        event_role: None,
        constraints: None,
        button_text: None,
        dialog_text: None,
        action_value: 0,
        hint_slider: None,
        hint_radio: false,
        modified_value: None,
        write_state: None,
        write_error: None,
        read_only: false,
    }
}

// ── Snapshot field label resolution ──────────────────────────────────────────

/// Try to resolve a human-readable field label from a saved snapshot's
/// config tree by matching (space, offset).
pub fn find_snapshot_field_label(
    snapshot: &crate::layout::node_snapshot::NodeSnapshot,
    space: u8,
    offset: &str,
) -> Option<String> {
    snapshot
        .flattened_config_entries()
        .into_iter()
        .find(|entry| {
            entry.leaf.space == Some(space)
                && entry
                    .leaf
                    .offset
                    .as_deref()
                    .is_some_and(|leaf_offset| leaf_offset.eq_ignore_ascii_case(offset))
        })
        .map(|entry| entry.path.join("."))
}

/// Fallback label when neither snapshot nor CDI provide a field label.
pub fn fallback_field_label(space: Option<u8>, offset: Option<&str>) -> Option<String> {
    match (space, offset) {
        (Some(space), Some(offset)) => Some(format!("Space {} @ {}", space, offset)),
        _ => None,
    }
}

/// Resolve the display name for a node from its snapshot SNIP data.
pub fn resolve_snapshot_node_name(
    snapshot: &crate::layout::node_snapshot::NodeSnapshot,
    node_key: &str,
) -> String {
    let user_name = snapshot.snip.user_name.trim();
    if user_name.is_empty() {
        node_key.to_string()
    } else {
        user_name.to_string()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::node_snapshot::{
        CaptureStatus, CdiReference, NodeSnapshot, NodeSnapshotLifecycle, SnapshotLeafValue,
        SnipSnapshot,
    };
    use lcc_rs::cdi::{Cdi, DataElement, EventIdElement, Group, Segment};

    #[test]
    fn parse_offset_hex() {
        assert_eq!(parse_offset("0x00000120"), Some(0x120));
        assert_eq!(parse_offset("0X0010"), Some(0x10));
        assert_eq!(parse_offset("FF"), Some(0xFF));
    }

    #[test]
    fn finds_snapshot_field_label_from_saved_path_tree() {
        let mut snapshot = NodeSnapshot {
            node_key: "020157000200".to_string(),
            node_id: Some(lcc_rs::NodeID::new([0x02, 0x01, 0x57, 0x00, 0x02, 0xD9])),
            profile_stem: None,
            lifecycle: NodeSnapshotLifecycle::Persisted,
            captured_at: "2026-04-26T00:00:00Z".to_string(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot::default(),
            cdi_ref: CdiReference {
                cache_key: "cache".to_string(),
                version: "1.0".to_string(),
                fingerprint: "fp".to_string(),
            },
            config: Default::default(),
            producer_identified_events: Vec::new(),
        };

        snapshot.add_config_leaf(
            &[
                "Port I/O".to_string(),
                "Line(2)".to_string(),
                "Event(0)".to_string(),
                "Indicator".to_string(),
            ],
            SnapshotLeafValue {
                value: "02.01.57.00.02.D9.02.66".to_string(),
                space: Some(253),
                offset: Some("0x00000010".to_string()),
            },
        );

        assert_eq!(
            find_snapshot_field_label(&snapshot, 253, "0x00000010").as_deref(),
            Some("Port I/O.Line(2).Event(0).Indicator")
        );
    }

    #[test]
    fn finds_cdi_field_label_for_replicated_event_path() {
        let cdi = Cdi {
            identification: None,
            acdi: None,
            segments: vec![Segment {
                name: Some("Port I/O".to_string()),
                description: None,
                space: 253,
                origin: 0,
                elements: vec![DataElement::Group(Group {
                    name: Some("Line".to_string()),
                    description: None,
                    offset: 0,
                    replication: 3,
                    repname: Vec::new(),
                    elements: vec![DataElement::Group(Group {
                        name: Some("Event".to_string()),
                        description: None,
                        offset: 0,
                        replication: 2,
                        repname: Vec::new(),
                        elements: vec![DataElement::EventId(EventIdElement {
                            name: Some("Indicator".to_string()),
                            description: None,
                            offset: 0,
                        })],
                        hints: None,
                    })],
                    hints: None,
                })],
            }],
        };

        let meta = find_field_meta_in_cdi(&cdi, 253, 32).expect("expected field metadata");
        assert_eq!(meta.field_label, "Port I/O.Line(2).Event(0).Indicator");
    }

    #[test]
    fn raw_bytes_to_int_value() {
        let meta = FieldMeta {
            leaf_type: LeafType::Int,
            size: 2,
            field_label: "test".into(),
        };
        // Big-endian 0x0042 = 66
        assert_eq!(raw_bytes_to_value_string(&meta, &[0x00, 0x42]), Some("66".to_string()));
    }

    #[test]
    fn raw_bytes_to_event_id() {
        let meta = FieldMeta {
            leaf_type: LeafType::EventId,
            size: 8,
            field_label: "test".into(),
        };
        let raw = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        assert_eq!(
            raw_bytes_to_value_string(&meta, &raw),
            Some("0102030405060708".to_string())
        );
    }

    #[test]
    fn string_to_config_value_int() {
        let leaf = field_meta_to_leaf(
            &FieldMeta {
                leaf_type: LeafType::Int,
                size: 4,
                field_label: String::new(),
            },
            253,
            0,
        );
        let cv = string_to_config_value("42", &leaf).unwrap();
        assert_eq!(cv, ConfigValue::Int { value: 42 });
    }

    #[test]
    fn string_to_config_value_event_id() {
        let leaf = field_meta_to_leaf(
            &FieldMeta {
                leaf_type: LeafType::EventId,
                size: 8,
                field_label: String::new(),
            },
            253,
            0,
        );
        let cv = string_to_config_value("01.02.03.04.05.06.07.08", &leaf).unwrap();
        match cv {
            ConfigValue::EventId { bytes, hex } => {
                assert_eq!(bytes, [1, 2, 3, 4, 5, 6, 7, 8]);
                assert_eq!(hex, "0102030405060708");
            }
            _ => panic!("expected EventId"),
        }
    }

    #[test]
    fn fallback_label_with_space_and_offset() {
        assert_eq!(
            fallback_field_label(Some(253), Some("0x00000010")),
            Some("Space 253 @ 0x00000010".to_string())
        );
    }

    #[test]
    fn fallback_label_none_when_missing() {
        assert_eq!(fallback_field_label(None, None), None);
    }

    #[test]
    fn resolve_snapshot_node_name_uses_user_name() {
        let snapshot = NodeSnapshot {
            node_key: "020157000200".to_string(),
            node_id: None,
            profile_stem: None,
            lifecycle: NodeSnapshotLifecycle::Persisted,
            captured_at: String::new(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot {
                user_name: "My Turnout".to_string(),
                ..SnipSnapshot::default()
            },
            cdi_ref: CdiReference {
                cache_key: String::new(),
                version: String::new(),
                fingerprint: String::new(),
            },
            config: Default::default(),
            producer_identified_events: Vec::new(),
        };
        assert_eq!(resolve_snapshot_node_name(&snapshot, "020157000200"), "My Turnout");
    }

    #[test]
    fn resolve_snapshot_node_name_falls_back_to_key() {
        let snapshot = NodeSnapshot {
            node_key: "020157000200".to_string(),
            node_id: None,
            profile_stem: None,
            lifecycle: NodeSnapshotLifecycle::Persisted,
            captured_at: String::new(),
            capture_status: CaptureStatus::Complete,
            missing: Vec::new(),
            snip: SnipSnapshot::default(),
            cdi_ref: CdiReference {
                cache_key: String::new(),
                version: String::new(),
                fingerprint: String::new(),
            },
            config: Default::default(),
            producer_identified_events: Vec::new(),
        };
        assert_eq!(resolve_snapshot_node_name(&snapshot, "020157000200"), "020157000200");
    }
}
