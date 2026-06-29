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
}
