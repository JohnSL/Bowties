//! Information channel types for layout persistence.
//!
//! An information channel is a typed, named representation of a single
//! piece of layout-meaningful information (e.g., "Block 7 Occupancy")
//! independent of protocol details.

use serde::{Deserialize, Serialize};

/// The kind of information a channel carries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ChannelType {
    BlockOccupancy,
}

/// Identifies the hardware backing a channel: which node, connector, and input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HardwareReference {
    /// Canonical node ID (uppercase hex, no dots) or `placeholder:<uuid>`.
    pub node_key: String,
    /// Connector slug, e.g. `connector-a`.
    pub connector: String,
    /// 1-based input ordinal.
    pub input: u32,
}

/// A single information channel in the layout inventory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InformationChannel {
    /// Unique identifier (UUID v4).
    pub id: String,
    /// User-assigned display name.
    pub name: String,
    /// Classification of what information this channel carries.
    pub channel_type: ChannelType,
    /// The hardware that backs this channel.
    pub hardware_ref: HardwareReference,
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
    pub const SCHEMA_VERSION: &'static str = "1.0";
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
    fn channel_type_serializes_to_kebab_case() {
        let json = serde_json::to_string(&ChannelType::BlockOccupancy).unwrap();
        assert_eq!(json, "\"block-occupancy\"");
    }

    #[test]
    fn information_channel_round_trips_yaml() {
        let channel = InformationChannel {
            id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
            name: "West Yard — Connector A — Input 1".to_string(),
            channel_type: ChannelType::BlockOccupancy,
            hardware_ref: HardwareReference {
                node_key: "05010101FF000001".to_string(),
                connector: "connector-a".to_string(),
                input: 1,
            },
        };

        let doc = ChannelsDocument::new(vec![channel.clone()]);
        let yaml = serde_yaml_ng::to_string(&doc).unwrap();
        let parsed: ChannelsDocument = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed.channels.len(), 1);
        assert_eq!(parsed.channels[0], channel);
    }

    #[test]
    fn empty_document_deserializes_from_empty_yaml() {
        let yaml = "schemaVersion: \"1.0\"\nchannels: []\n";
        let doc: ChannelsDocument = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(doc.channels.len(), 0);
    }
}
