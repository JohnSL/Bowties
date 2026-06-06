//! NodeKey — unified backend identifier for live LCC nodes and placeholder boards.
//!
//! See ADR-0010 (`product/architecture/adr/0010-nodekey-sum-type.md`).
//!
//! Wire form:
//! - Live:        canonical 12-hex uppercase (e.g. `020157000002D9`)
//! - Placeholder: `placeholder:<uuid>`
//!
//! Parsing accepts dotted (`02.01.57.00.02.D9`) and canonical hex inputs for
//! live nodes; both round-trip to the canonical form on serialize.

use lcc_rs::NodeID;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

pub const PLACEHOLDER_PREFIX: &str = "placeholder:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NodeKey {
    Live(NodeID),
    Placeholder(Uuid),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeKeyParseError(pub String);

impl fmt::Display for NodeKeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid NodeKey: {}", self.0)
    }
}

impl std::error::Error for NodeKeyParseError {}

impl NodeKey {
    pub fn from_node_id(id: NodeID) -> Self {
        NodeKey::Live(id)
    }

    pub fn placeholder(uuid: Uuid) -> Self {
        NodeKey::Placeholder(uuid)
    }

    pub fn parse(input: &str) -> Result<Self, NodeKeyParseError> {
        if let Some(rest) = input.strip_prefix(PLACEHOLDER_PREFIX) {
            return Uuid::parse_str(rest)
                .map(NodeKey::Placeholder)
                .map_err(|e| NodeKeyParseError(format!("bad placeholder uuid '{}': {}", rest, e)));
        }
        NodeID::from_hex_string(input)
            .map(NodeKey::Live)
            .map_err(NodeKeyParseError)
    }

    pub fn is_placeholder(&self) -> bool {
        matches!(self, NodeKey::Placeholder(_))
    }

    pub fn is_live(&self) -> bool {
        matches!(self, NodeKey::Live(_))
    }

    pub fn as_node_id(&self) -> Option<NodeID> {
        match self {
            NodeKey::Live(id) => Some(*id),
            NodeKey::Placeholder(_) => None,
        }
    }
}

impl fmt::Display for NodeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeKey::Live(id) => write!(f, "{}", id.to_canonical()),
            NodeKey::Placeholder(uuid) => write!(f, "{}{}", PLACEHOLDER_PREFIX, uuid),
        }
    }
}

impl FromStr for NodeKey {
    type Err = NodeKeyParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        NodeKey::parse(s)
    }
}

impl Serialize for NodeKey {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        ser.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for NodeKey {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        let s = String::deserialize(de)?;
        NodeKey::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl From<NodeID> for NodeKey {
    fn from(id: NodeID) -> Self {
        NodeKey::Live(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_id() -> NodeID {
        NodeID::from_hex_string("02.01.57.00.02.D9").unwrap()
    }

    #[test]
    fn live_from_node_id_displays_canonical() {
        let key = NodeKey::from_node_id(sample_id());
        assert_eq!(key.to_string(), "0201570002D9");
    }

    #[test]
    fn placeholder_displays_with_prefix() {
        let uuid = Uuid::parse_str("7c9e6b1a-0000-4000-8000-000000000001").unwrap();
        let key = NodeKey::placeholder(uuid);
        assert_eq!(key.to_string(), "placeholder:7c9e6b1a-0000-4000-8000-000000000001");
    }

    #[test]
    fn parse_dotted_live() {
        let key = NodeKey::parse("02.01.57.00.02.D9").unwrap();
        assert_eq!(key, NodeKey::Live(sample_id()));
    }

    #[test]
    fn parse_canonical_live() {
        let key = NodeKey::parse("0201570002D9").unwrap();
        assert_eq!(key, NodeKey::Live(sample_id()));
    }

    #[test]
    fn parse_placeholder() {
        let uuid = Uuid::parse_str("7c9e6b1a-0000-4000-8000-000000000001").unwrap();
        let key = NodeKey::parse("placeholder:7c9e6b1a-0000-4000-8000-000000000001").unwrap();
        assert_eq!(key, NodeKey::Placeholder(uuid));
    }

    #[test]
    fn parse_garbage_errors() {
        assert!(NodeKey::parse("garbage").is_err());
        assert!(NodeKey::parse("placeholder:not-a-uuid").is_err());
        assert!(NodeKey::parse("").is_err());
    }

    #[test]
    fn serde_round_trip_live() {
        let key = NodeKey::from_node_id(sample_id());
        let s = serde_json::to_string(&key).unwrap();
        assert_eq!(s, "\"0201570002D9\"");
        let back: NodeKey = serde_json::from_str(&s).unwrap();
        assert_eq!(back, key);
    }

    #[test]
    fn serde_round_trip_placeholder() {
        let uuid = Uuid::new_v4();
        let key = NodeKey::placeholder(uuid);
        let s = serde_json::to_string(&key).unwrap();
        let back: NodeKey = serde_json::from_str(&s).unwrap();
        assert_eq!(back, key);
    }

    #[test]
    fn hash_eq_parity_across_input_forms() {
        use std::collections::HashSet;
        let dotted = NodeKey::parse("02.01.57.00.02.D9").unwrap();
        let canon = NodeKey::parse("0201570002D9").unwrap();
        assert_eq!(dotted, canon);
        let mut set = HashSet::new();
        set.insert(dotted);
        assert!(set.contains(&canon));
    }

    #[test]
    fn is_placeholder_and_is_live() {
        let live = NodeKey::from_node_id(sample_id());
        let ph = NodeKey::placeholder(Uuid::new_v4());
        assert!(live.is_live());
        assert!(!live.is_placeholder());
        assert!(ph.is_placeholder());
        assert!(!ph.is_live());
    }
}
