//! Serde helpers for serializing `NodeID` as canonical hex strings in YAML.
//!
//! Usage on struct fields:
//! ```ignore
//! #[serde(with = "crate::layout::serde_node_id::canonical")]
//! pub node_id: NodeID,
//!
//! #[serde(with = "crate::layout::serde_node_id::canonical_option")]
//! pub node_id: Option<NodeID>,
//!
//! #[serde(with = "crate::layout::serde_node_id::canonical_vec")]
//! pub node_ids: Vec<NodeID>,
//! ```

use lcc_rs::NodeID;
use serde::{self, Deserialize, Deserializer, Serializer};

/// Serialize/deserialize `NodeID` as canonical hex string (e.g. "050201020200").
pub mod canonical {
    use super::*;

    pub fn serialize<S>(node_id: &NodeID, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&node_id.to_canonical())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<NodeID, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NodeID::from_hex_string(&s).map_err(serde::de::Error::custom)
    }
}

/// Serialize/deserialize `Option<NodeID>` as optional canonical hex string.
pub mod canonical_option {
    use super::*;

    pub fn serialize<S>(node_id: &Option<NodeID>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match node_id {
            Some(id) => serializer.serialize_str(&id.to_canonical()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<NodeID>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        match opt {
            Some(s) => NodeID::from_hex_string(&s)
                .map(Some)
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

/// Serialize/deserialize `Vec<NodeID>` as a list of canonical hex strings.
pub mod canonical_vec {
    use super::*;

    pub fn serialize<S>(node_ids: &Vec<NodeID>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(node_ids.len()))?;
        for id in node_ids {
            seq.serialize_element(&id.to_canonical())?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<NodeID>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let strings: Vec<String> = Vec::deserialize(deserializer)?;
        strings
            .iter()
            .map(|s| NodeID::from_hex_string(s).map_err(serde::de::Error::custom))
            .collect()
    }
}
