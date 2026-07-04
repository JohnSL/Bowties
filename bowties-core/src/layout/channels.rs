//! Information channel types for layout persistence.
//!
//! An information channel is a typed, named representation of a single
//! piece of layout-meaningful information (e.g., "Block 7 Occupancy")
//! independent of protocol details.
//!
//! Spec 018 / S2 (ADR-0013) introduced the role / style / ownership /
//! binding schema. The legacy `channelType` and `hardwareRef` were retired
//! in the same change set; pre-018 layouts that already shipped a
//! `channels.yaml` will fail to load (acceptable per spec 018 FR-009 —
//! Bowties is pre-1.0 and the user manages pre-018 layouts manually).

use serde::{Deserialize, Serialize};

/// The state-vocabulary contract a facility slot binds by. Declared in
/// Rust (mirrored as a TS string-literal union) so production code matches
/// state values exhaustively. ADR-0013.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ChannelRole {
    BlockOccupancy,
    /// `lamp-indicator` lands with Spec 018 / S5; declared here so the
    /// enum reads as the role universe rather than as a one-element list.
    LampIndicator,
}

/// Lifecycle classification: who creates and destroys this channel.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ChannelOwnership {
    /// Auto-created by hardware-config selection (e.g., BOD daughterboard).
    HardwareOwned,
    /// Created via a facility slot's Add-channel action. Lands with S5.
    UserOwned,
}

/// What the channel is bound to. Discriminated union; the `kind` MUST match
/// the channel's style's declared binding shape (enforced at channel
/// creation; not by the type system because styles are declared in YAML).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChannelBinding {
    /// A single pin on a TowerLCC-family connector input. The S2 variant.
    #[serde(rename_all = "camelCase")]
    ConnectorInput {
        node_key: String,
        connector: String,
        input: u32,
    },
    /// A Direct Lamp Control row on a Signal-LCC node. Lands with S5.
    #[serde(rename_all = "camelCase")]
    LampRow { node_key: String, row_ordinal: u32 },
}

/// A single information channel in the layout inventory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InformationChannel {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// User-assigned display name.
    pub name: String,
    /// State-vocabulary contract (interface).
    pub role: ChannelRole,
    /// Hardware-shape realisation (implementation). Looked up in the
    /// profile YAML style catalog. ADR-0013.
    pub style: String,
    /// Who owns this channel's lifecycle.
    pub ownership: ChannelOwnership,
    /// What hardware target this channel addresses.
    pub binding: ChannelBinding,
}

/// Root structure for `channels.yaml` persistence.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsDocument {
    #[serde(default)]
    pub schema_version: String,
    #[serde(default)]
    pub channels: Vec<InformationChannel>,
}

impl ChannelsDocument {
    /// Bumped from "1.0" → "2.0" with the Spec 018 / S2 schema change.
    pub const SCHEMA_VERSION: &'static str = "2.0";
    pub const FILE_NAME: &'static str = "channels.yaml";

    pub fn new(channels: Vec<InformationChannel>) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            channels,
        }
    }
}

/// Errors `apply_channel_deltas` may return per delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelApplyError {
    /// `CreateChannel` referenced an id already present in the doc.
    DuplicateChannelId { channel_id: String },
    /// `RenameChannel` referenced an id not present in the doc.
    UnknownRenameTarget { channel_id: String },
}

impl std::fmt::Display for ChannelApplyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateChannelId { channel_id } => {
                write!(f, "duplicate channel id: {}", channel_id)
            }
            Self::UnknownRenameTarget { channel_id } => {
                write!(f, "unknown channel id for rename: {}", channel_id)
            }
        }
    }
}

impl std::error::Error for ChannelApplyError {}

/// Apply the channel-relevant variants of [`crate::layout::types::LayoutEditDelta`]
/// to a `ChannelsDocument`, mutating it in place.
///
/// Sibling of [`crate::layout::facilities::apply_facility_deltas`] — both are
/// called from `save_layout_directory` against the same delta list. Non-channel
/// deltas are skipped silently.
///
/// Always ensures `schema_version` is set to the current schema after applying
/// any change, mirroring the facility apply.
///
/// The atomic-save fold generalized this from `CreateUserOwnedChannel` only to
/// the full `Create/Rename/Delete` triplet. All channel edits — hardware- and
/// user-owned — travel one delta path (ADR-0002).
pub fn apply_channel_deltas(
    doc: &mut ChannelsDocument,
    deltas: &[crate::layout::types::LayoutEditDelta],
) -> Result<(), ChannelApplyError> {
    use crate::layout::types::LayoutEditDelta;

    let mut touched = false;
    for delta in deltas {
        match delta {
            LayoutEditDelta::CreateChannel { channel } => {
                if doc.channels.iter().any(|c| c.id == channel.id) {
                    return Err(ChannelApplyError::DuplicateChannelId {
                        channel_id: channel.id.clone(),
                    });
                }
                doc.channels.push(channel.clone());
                touched = true;
            }
            LayoutEditDelta::RenameChannel {
                channel_id,
                new_name,
            } => {
                let target = doc.channels.iter_mut().find(|c| c.id == *channel_id);
                let Some(channel) = target else {
                    return Err(ChannelApplyError::UnknownRenameTarget {
                        channel_id: channel_id.clone(),
                    });
                };
                channel.name = new_name.clone();
                touched = true;
            }
            LayoutEditDelta::DeleteChannel { channel_id } => {
                let before = doc.channels.len();
                doc.channels.retain(|c| c.id != *channel_id);
                if doc.channels.len() != before {
                    touched = true;
                }
                // Idempotent: unknown ids are ignored (mirrors DeleteFacility).
            }
            _ => {}
        }
    }
    if touched && doc.schema_version.is_empty() {
        doc.schema_version = ChannelsDocument::SCHEMA_VERSION.to_string();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_role_serializes_to_kebab_case() {
        assert_eq!(
            serde_json::to_string(&ChannelRole::BlockOccupancy).unwrap(),
            "\"block-occupancy\"",
        );
        assert_eq!(
            serde_json::to_string(&ChannelRole::LampIndicator).unwrap(),
            "\"lamp-indicator\"",
        );
    }

    #[test]
    fn channel_ownership_serializes_to_kebab_case() {
        assert_eq!(
            serde_json::to_string(&ChannelOwnership::HardwareOwned).unwrap(),
            "\"hardware-owned\"",
        );
        assert_eq!(
            serde_json::to_string(&ChannelOwnership::UserOwned).unwrap(),
            "\"user-owned\"",
        );
    }

    #[test]
    fn channel_binding_connector_input_serializes_with_kind_discriminator() {
        let binding = ChannelBinding::ConnectorInput {
            node_key: "05010101FF000001".to_string(),
            connector: "connector-a".to_string(),
            input: 3,
        };
        let json = serde_json::to_value(&binding).unwrap();
        assert_eq!(json["kind"], "connectorInput");
        assert_eq!(json["nodeKey"], "05010101FF000001");
        assert_eq!(json["connector"], "connector-a");
        assert_eq!(json["input"], 3);
    }

    #[test]
    fn channel_binding_lamp_row_serializes_with_kind_discriminator() {
        let binding = ChannelBinding::LampRow {
            node_key: "05010101FF000002".to_string(),
            row_ordinal: 7,
        };
        let json = serde_json::to_value(&binding).unwrap();
        assert_eq!(json["kind"], "lampRow");
        assert_eq!(json["nodeKey"], "05010101FF000002");
        assert_eq!(json["rowOrdinal"], 7);
    }

    #[test]
    fn information_channel_round_trips_yaml_with_connector_input_binding() {
        let channel = InformationChannel {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "West Yard — Connector A — Input 1".to_string(),
            role: ChannelRole::BlockOccupancy,
            style: "bod-block-detector-input".to_string(),
            ownership: ChannelOwnership::HardwareOwned,
            binding: ChannelBinding::ConnectorInput {
                node_key: "05010101FF000001".to_string(),
                connector: "connector-a".to_string(),
                input: 1,
            },
        };

        let doc = ChannelsDocument::new(vec![channel.clone()]);
        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        let parsed: ChannelsDocument = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed.schema_version, "2.0");
        assert_eq!(parsed.channels.len(), 1);
        assert_eq!(parsed.channels[0], channel);
    }

    #[test]
    fn information_channel_round_trips_yaml_with_lamp_row_binding() {
        let channel = InformationChannel {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "Block 5 Indicator".to_string(),
            role: ChannelRole::LampIndicator,
            style: "single-led-direct-lamp".to_string(),
            ownership: ChannelOwnership::UserOwned,
            binding: ChannelBinding::LampRow {
                node_key: "05010101FF000002".to_string(),
                row_ordinal: 4,
            },
        };

        let doc = ChannelsDocument::new(vec![channel.clone()]);
        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        let parsed: ChannelsDocument = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed.channels[0], channel);
    }

    #[test]
    fn empty_document_deserializes_from_empty_yaml() {
        let yaml = "schemaVersion: \"2.0\"\nchannels: []\n";
        let doc: ChannelsDocument = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(doc.channels.len(), 0);
    }

    #[test]
    fn schema_version_constant_is_v2() {
        // Locks the ADR-0013 schema bump so it cannot regress silently.
        assert_eq!(ChannelsDocument::SCHEMA_VERSION, "2.0");
    }

    // ── apply_channel_deltas (Spec 018 / S5 — D2) ──────────────────────────

    fn lamp_indicator_channel(id: &str, ordinal: u32) -> InformationChannel {
        InformationChannel {
            id: id.to_string(),
            name: format!("Channel {ordinal}"),
            role: ChannelRole::LampIndicator,
            style: "single-led-direct-lamp".to_string(),
            ownership: ChannelOwnership::UserOwned,
            binding: ChannelBinding::LampRow {
                node_key: "05010101FF000002".to_string(),
                row_ordinal: ordinal,
            },
        }
    }

    fn create_delta(channel: &InformationChannel) -> crate::layout::types::LayoutEditDelta {
        crate::layout::types::LayoutEditDelta::CreateChannel {
            channel: channel.clone(),
        }
    }

    #[test]
    fn apply_channel_deltas_creates_on_empty_document() {
        let mut doc = ChannelsDocument::default();
        let ch = lamp_indicator_channel("ch-1", 1);

        apply_channel_deltas(&mut doc, &[create_delta(&ch)]).unwrap();

        assert_eq!(doc.channels.len(), 1);
        assert_eq!(doc.channels[0], ch);
        assert_eq!(doc.schema_version, ChannelsDocument::SCHEMA_VERSION);
    }

    #[test]
    fn apply_channel_deltas_creates_on_populated_document() {
        let existing = lamp_indicator_channel("ch-existing", 0);
        let mut doc = ChannelsDocument::new(vec![existing.clone()]);
        let new_ch = lamp_indicator_channel("ch-new", 2);

        apply_channel_deltas(&mut doc, &[create_delta(&new_ch)]).unwrap();

        assert_eq!(doc.channels.len(), 2);
        assert_eq!(doc.channels[0], existing);
        assert_eq!(doc.channels[1], new_ch);
    }

    #[test]
    fn apply_channel_deltas_rejects_duplicate_id() {
        let ch = lamp_indicator_channel("ch-dup", 1);
        let mut doc = ChannelsDocument::new(vec![ch.clone()]);

        let err = apply_channel_deltas(&mut doc, &[create_delta(&ch)]).unwrap_err();
        assert_eq!(
            err,
            ChannelApplyError::DuplicateChannelId {
                channel_id: "ch-dup".to_string(),
            }
        );
        // Doc unchanged.
        assert_eq!(doc.channels.len(), 1);
    }

    #[test]
    fn apply_channel_deltas_renames_channel_by_id() {
        let ch = lamp_indicator_channel("ch-1", 1);
        let mut doc = ChannelsDocument::new(vec![ch.clone()]);

        apply_channel_deltas(
            &mut doc,
            &[crate::layout::types::LayoutEditDelta::RenameChannel {
                channel_id: "ch-1".to_string(),
                new_name: "Renamed".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(doc.channels[0].name, "Renamed");
    }

    #[test]
    fn apply_channel_deltas_rejects_rename_of_unknown_id() {
        let ch = lamp_indicator_channel("ch-1", 1);
        let mut doc = ChannelsDocument::new(vec![ch]);

        let err = apply_channel_deltas(
            &mut doc,
            &[crate::layout::types::LayoutEditDelta::RenameChannel {
                channel_id: "ch-missing".to_string(),
                new_name: "Renamed".to_string(),
            }],
        )
        .unwrap_err();
        assert_eq!(
            err,
            ChannelApplyError::UnknownRenameTarget {
                channel_id: "ch-missing".to_string(),
            }
        );
    }

    #[test]
    fn apply_channel_deltas_deletes_channel_by_id() {
        let keep = lamp_indicator_channel("ch-keep", 1);
        let drop = lamp_indicator_channel("ch-drop", 2);
        let mut doc = ChannelsDocument::new(vec![keep.clone(), drop.clone()]);

        apply_channel_deltas(
            &mut doc,
            &[crate::layout::types::LayoutEditDelta::DeleteChannel {
                channel_id: "ch-drop".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(doc.channels.len(), 1);
        assert_eq!(doc.channels[0], keep);
    }

    #[test]
    fn apply_channel_deltas_delete_is_idempotent_for_unknown_id() {
        let ch = lamp_indicator_channel("ch-1", 1);
        let mut doc = ChannelsDocument::new(vec![ch.clone()]);

        apply_channel_deltas(
            &mut doc,
            &[crate::layout::types::LayoutEditDelta::DeleteChannel {
                channel_id: "ch-nope".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(doc.channels, vec![ch]);
    }

    #[test]
    fn apply_channel_deltas_applies_mixed_variants_in_order() {
        let existing = lamp_indicator_channel("ch-existing", 0);
        let new_ch = lamp_indicator_channel("ch-new", 3);
        let mut doc = ChannelsDocument::new(vec![existing]);

        apply_channel_deltas(
            &mut doc,
            &[
                create_delta(&new_ch),
                crate::layout::types::LayoutEditDelta::RenameChannel {
                    channel_id: "ch-existing".to_string(),
                    new_name: "Existing Renamed".to_string(),
                },
                crate::layout::types::LayoutEditDelta::DeleteChannel {
                    channel_id: "ch-existing".to_string(),
                },
            ],
        )
        .unwrap();

        assert_eq!(doc.channels.len(), 1);
        assert_eq!(doc.channels[0].id, "ch-new");
    }

    #[test]
    fn apply_channel_deltas_ignores_facility_deltas() {
        use crate::layout::types::LayoutEditDelta;

        let mut doc = ChannelsDocument::default();
        let other = LayoutEditDelta::AttachChannelToSlot {
            facility_id: "fac-1".to_string(),
            slot_label: "input".to_string(),
            channel_id: "ch-1".to_string(),
        };

        apply_channel_deltas(&mut doc, &[other]).unwrap();

        assert!(doc.channels.is_empty());
        assert!(doc.schema_version.is_empty(), "untouched doc should not bump schema");
    }

    #[test]
    fn create_channel_delta_serializes_as_camel_case() {
        let ch = lamp_indicator_channel("ch-1", 1);
        let delta = create_delta(&ch);
        let json = serde_json::to_value(&delta).unwrap();
        assert_eq!(json["type"], "createChannel");
        assert_eq!(json["channel"]["id"], "ch-1");
        assert_eq!(json["channel"]["binding"]["kind"], "lampRow");
    }

    #[test]
    fn rename_and_delete_channel_deltas_serialize_as_camel_case() {
        let rename = crate::layout::types::LayoutEditDelta::RenameChannel {
            channel_id: "ch-1".to_string(),
            new_name: "New".to_string(),
        };
        let json = serde_json::to_value(&rename).unwrap();
        assert_eq!(json["type"], "renameChannel");
        assert_eq!(json["channelId"], "ch-1");
        assert_eq!(json["newName"], "New");

        let delete = crate::layout::types::LayoutEditDelta::DeleteChannel {
            channel_id: "ch-1".to_string(),
        };
        let json = serde_json::to_value(&delete).unwrap();
        assert_eq!(json["type"], "deleteChannel");
        assert_eq!(json["channelId"], "ch-1");
    }
}
