//! CDI (Configuration Description Information) data structures
//!
//! This module provides types and functions for parsing and navigating
//! CDI XML documents per the S-9.7.4.1 specification.

pub mod parser;
pub mod hierarchy;
pub mod role;

pub use role::{EventRole, classify_event_slot};
pub use hierarchy::walk_event_slots;

use serde::{Deserialize, Serialize};

/// Root CDI structure representing a complete Configuration Description Information document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cdi {
    /// Optional identification information (manufacturer, model, versions)
    pub identification: Option<Identification>,
    
    /// Optional ACDI (standardized node info)
    pub acdi: Option<Acdi>,
    
    /// Configuration segments (0 or more)
    pub segments: Vec<Segment>,
}

/// Node identification information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identification {
    /// Manufacturer name
    pub manufacturer: Option<String>,
    
    /// Model name
    pub model: Option<String>,
    
    /// Hardware version
    pub hardware_version: Option<String>,
    
    /// Software version
    pub software_version: Option<String>,
}

/// ACDI (Abbreviated Configuration Description Information)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Acdi {
    /// User-provided node name
    pub user_name: Option<String>,
    
    /// User-provided node description
    pub user_desc: Option<String>,
}

/// Top-level organizational unit within a CDI, representing a memory space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Segment name (optional, user-visible)
    pub name: Option<String>,
    
    /// Segment description (optional, user-visible)
    pub description: Option<String>,
    
    /// Memory space number (e.g., 253 for configuration memory)
    pub space: u8,
    
    /// Starting address in memory space
    pub origin: i32,
    
    /// Child elements (groups and primitive elements)
    pub elements: Vec<DataElement>,
}

/// Discriminated union of all possible CDI element types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DataElement {
    Group(Group),
    Int(IntElement),
    String(StringElement),
    EventId(EventIdElement),
    Float(FloatElement),
    Action(ActionElement),
    Blob(BlobElement),
}

/// Collection of related configuration elements, supporting replication for repeated structures
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Group name (optional, user-visible)
    pub name: Option<String>,
    
    /// Group description (optional, user-visible)
    pub description: Option<String>,
    
    /// Memory offset from parent address
    pub offset: i32,
    
    /// Number of times this group is replicated (default: 1)
    pub replication: u32,
    
    /// Template for instance naming (e.g., ["Line"] → "Line 1", "Line 2")
    pub repname: Vec<String>,
    
    /// Child elements (can include nested groups - RECURSIVE)
    pub elements: Vec<DataElement>,
    
    /// Optional rendering hints (future use)
    pub hints: Option<GroupHints>,
}

impl Group {
    /// Check if group should be rendered (Footnote 4 compliance)
    /// 
    /// Per S-9.7.4.1 Footnote 4: Filter groups with no name AND no description AND no elements
    pub fn should_render(&self) -> bool {
        self.name.is_some() 
            || self.description.is_some() 
            || !self.elements.is_empty()
    }
}

/// Optional rendering hints for groups
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupHints {
    /// Display hint (e.g., "compact", "expanded")
    pub display: Option<String>,
}

/// Integer configuration element with optional constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Size in bytes (1, 2, 4, or 8)
    pub size: u8,
    
    /// Memory offset from parent address
    pub offset: i32,
    
    /// Minimum allowed value (optional constraint)
    pub min: Option<i64>,
    
    /// Maximum allowed value (optional constraint)
    pub max: Option<i64>,
    
    /// Default value (optional)
    pub default: Option<i64>,
    
    /// Value mapping (e.g., 0="Inactive", 1="Active")
    pub map: Option<Map>,
}

/// Event ID configuration element (always 8 bytes)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventIdElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Memory offset from parent address
    pub offset: i32,
}

/// String configuration element with length constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Maximum string length in bytes
    pub size: usize,
    
    /// Memory offset from parent address
    pub offset: i32,
}

/// Float configuration element
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FloatElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Memory offset from parent address
    pub offset: i32,
}

/// Action configuration element (triggers an action, no value)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Memory offset from parent address
    pub offset: i32,
}

/// Blob configuration element (arbitrary binary data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobElement {
    /// Element name (optional, user-visible)
    pub name: Option<String>,
    
    /// Element description (optional, user-visible)
    pub description: Option<String>,
    
    /// Size in bytes
    pub size: usize,
    
    /// Memory offset from parent address
    pub offset: i32,
}

/// Value-to-label mapping for constrained selections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Map {
    /// Map entries (value → label)
    pub entries: Vec<MapEntry>,
}

/// Single map entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapEntry {
    /// Numeric value
    pub value: i64,
    
    /// User-visible label
    pub label: String,
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_should_render_with_name() {
        let group = Group {
            name: Some("Test Group".to_string()),
            description: None,
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![],
            hints: None,
        };
        assert!(group.should_render(), "Group with name should render");
    }

    #[test]
    fn test_group_should_render_with_description() {
        let group = Group {
            name: None,
            description: Some("Test description".to_string()),
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![],
            hints: None,
        };
        assert!(group.should_render(), "Group with description should render");
    }

    #[test]
    fn test_group_should_render_with_elements() {
        let group = Group {
            name: None,
            description: None,
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![
                DataElement::Int(IntElement {
                    name: Some("Test".to_string()),
                    description: None,
                    size: 1,
                    offset: 0,
                    min: None,
                    max: None,
                    default: None,
                    map: None,
                }),
            ],
            hints: None,
        };
        assert!(group.should_render(), "Group with elements should render");
    }

    #[test]
    fn test_group_should_not_render_empty() {
        // Footnote 4 compliance: Filter groups with no name AND no description AND no elements
        let group = Group {
            name: None,
            description: None,
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![],
            hints: None,
        };
        assert!(!group.should_render(), "Empty group should not render per Footnote 4");
    }

    #[test]
    fn test_cdi_struct_creation() {
        let cdi = Cdi {
            identification: Some(Identification {
                manufacturer: Some("Test Mfg".to_string()),
                model: Some("Test Model".to_string()),
                hardware_version: Some("v1.0".to_string()),
                software_version: Some("v2.0".to_string()),
            }),
            acdi: Some(Acdi {
                user_name: Some("My Node".to_string()),
                user_desc: Some("Test Node".to_string()),
            }),
            segments: vec![],
        };
        
        assert!(cdi.identification.is_some());
        assert_eq!(cdi.identification.unwrap().manufacturer, Some("Test Mfg".to_string()));
    }

    #[test]
    fn test_segment_creation() {
        let segment = Segment {
            name: Some("Config Memory".to_string()),
            description: Some("Main configuration".to_string()),
            space: 253,
            origin: 0,
            elements: vec![],
        };
        
        assert_eq!(segment.space, 253);
        assert_eq!(segment.origin, 0);
    }

    #[test]
    fn test_int_element_validation() {
        let int_elem = IntElement {
            name: Some("Test Int".to_string()),
            description: None,
            size: 2,
            offset: 10,
            min: Some(0),
            max: Some(100),
            default: Some(50),
            map: None,
        };
        
        assert_eq!(int_elem.size, 2);
        assert_eq!(int_elem.min, Some(0));
        assert_eq!(int_elem.max, Some(100));
    }

    #[test]
    fn test_eventid_element_creation() {
        let eventid = EventIdElement {
            name: Some("Test Event".to_string()),
            description: Some("Event description".to_string()),
            offset: 0,
        };
        
        assert_eq!(eventid.name, Some("Test Event".to_string()));
    }

    #[test]
    fn test_map_with_entries() {
        let map = Map {
            entries: vec![
                MapEntry { value: 0, label: "Disabled".to_string() },
                MapEntry { value: 1, label: "Enabled".to_string() },
            ],
        };
        
        assert_eq!(map.entries.len(), 2);
        assert_eq!(map.entries[0].value, 0);
        assert_eq!(map.entries[1].label, "Enabled");
    }
}