//! CDI XML parsing using roxmltree
//!
//! This module provides functions for parsing CDI XML documents
//! into the structured types defined in the parent module.

use super::*;
use roxmltree::{Document, Node};

/// Parse CDI XML into structured Cdi type
///
/// T111: This is the main entry point for CDI XML parsing. It constructs
/// a Document tree using roxmltree and validates the root element.
/// 
/// The CDI XML structure is defined by the LCC CDI specification and consists of:
/// - Optional <identification> element (manufacturer, model, versions)
/// - Optional <acdi> element (user config: name, description)
/// - One or more <segment> elements (configuration memory spaces)
///
/// # Arguments
/// * `xml_content` - CDI XML document as string
///
/// # Returns
/// * `Ok(Cdi)` - Successfully parsed CDI structure
/// * `Err(String)` - Parse error message
///
/// # Example CDI XML
/// ```xml
/// <cdi>
///   <identification>
///     <manufacturer>Example Corp</manufacturer>
///     <model>ABC-123</model>
///   </identification>
///   <segment space="253" origin="0">
///     <name>Config</name>
///     <int size="1" offset="0"><name>Setting</name></int>
///   </segment>
/// </cdi>
/// ```
pub fn parse_cdi(xml_content: &str) -> Result<Cdi, String> {
    // Parse XML into document tree (validates well-formedness)
    let doc = Document::parse(xml_content)
        .map_err(|e| format!("XML parse error: {}", e))?;
    
    // Root element must be <cdi> per LCC specification
    let root = doc.root_element();
    if root.tag_name().name() != "cdi" {
        return Err(format!("Root element must be 'cdi', found '{}'", root.tag_name().name()));
    }
    
    let mut identification = None;
    let mut acdi = None;
    let mut segments = Vec::new();
    
    // Iterate through child elements, parsing known types
    // Unknown elements are ignored (forward compatibility)
    for child in root.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "identification" => {
                identification = Some(parse_identification(child));
            }
            "acdi" => {
                acdi = Some(parse_acdi(child));
            }
            "segment" => {
                segments.push(parse_segment(child)?);
            }
            _ => {} // Ignore unknown elements (forward compatibility)
        }
    }
    
    Ok(Cdi {
        identification,
        acdi,
        segments,
    })
}

/// Parse identification element
/// 
/// T111: Extracts device identification metadata from the CDI.
/// This data is typically displayed in device lists and used for
/// CDI file caching (manufacturer_model_version.cdi.xml).
/// 
/// All child elements are optional per LCC CDI spec.
fn parse_identification(node: Node) -> Identification {
    let mut manufacturer = None;
    let mut model = None;
    let mut hardware_version = None;
    let mut software_version = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "manufacturer" => manufacturer = child.text().map(|s| s.to_string()),
            "model" => model = child.text().map(|s| s.to_string()),
            "hardwareVersion" => hardware_version = child.text().map(|s| s.to_string()),
            "softwareVersion" => software_version = child.text().map(|s| s.to_string()),
            _ => {}
        }
    }
    
    Identification {
        manufacturer,
        model,
        hardware_version,
        software_version,
    }
}

/// Parse ACDI element
/// 
/// T111: ACDI (Abbreviated Configuration Description Information) contains
/// user-configurable device identity fields. Users can customize these
/// via the LCC protocol to give devices friendly names and descriptions.
/// 
/// Example: Renaming "Node 05.01.01.01.03.01" to "Living Room Lights"
fn parse_acdi(node: Node) -> Acdi {
    let mut user_name = None;
    let mut user_desc = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => user_name = child.text().map(|s| s.to_string()),
            "description" => user_desc = child.text().map(|s| s.to_string()),
            _ => {}
        }
    }
    
    Acdi {
        user_name,
        user_desc,
    }
}

/// Parse segment element (extract space, origin, elements)
/// 
/// T111: A segment represents a contiguous memory space in the device's
/// configuration. Common spaces:
/// - 253 (0xFD): Configuration Definition Information space
/// - 254 (0xFE): All Memory space
/// - 0-252: Manufacturer-defined spaces
/// 
/// The 'origin' attribute defines the base address (default: 0).
/// Child elements define the configuration structure within this space.
fn parse_segment(node: Node) -> Result<Segment, String> {
    // 'space' attribute is mandatory per LCC spec
    let space = node.attribute("space")
        .and_then(|s| s.parse::<u8>().ok())
        .ok_or_else(|| "Segment missing 'space' attribute".to_string())?;
    
    // 'origin' is optional, defaults to 0
    let origin = node.attribute("origin")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let mut name = None;
    let mut description = None;
    let mut elements = Vec::new();
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => name = child.text().map(|s| s.to_string()),
            "description" => description = child.text().map(|s| s.to_string()),
            _ => {
                // Try to parse as data element
                if let Some(element) = parse_data_element(child)? {
                    elements.push(element);
                }
            }
        }
    }
    
    Ok(Segment {
        name,
        description,
        space,
        origin,
        elements,
    })
}

/// Parse data element (recursive, handles all DataElement types)
///
/// T111: This is a recursive function that handles all CDI element types:
/// - <group>: Container for other elements (can be nested, replicated)
/// - <int>: Integer configuration variable
/// - <string>: String configuration variable
/// - <eventid>: LCC Event ID (8 bytes)
/// - <float>: Floating-point variable
/// - <action>: Trigger/button element
/// - <blob>: Binary data blob
/// 
/// Recursion enables nested group structures (groups within groups).
/// 
/// Returns None if the element should be filtered out (e.g., empty group per Footnote 4)
fn parse_data_element(node: Node) -> Result<Option<DataElement>, String> {
    let element = match node.tag_name().name() {
        "group" => {
            let group = parse_group(node)?;
            // Apply Footnote 4 filtering: Hide empty groups and groups with
            // only metadata (description, name) but no actual config elements
            if !group.should_render() {
                return Ok(None);
            }
            DataElement::Group(group)
        }
        "int" => DataElement::Int(parse_int_element(node)?),
        "string" => DataElement::String(parse_string_element(node)?),
        "eventid" => DataElement::EventId(parse_eventid_element(node)?),
        "float" => DataElement::Float(parse_float_element(node)?),
        "action" => DataElement::Action(parse_action_element(node)?),
        "blob" => DataElement::Blob(parse_blob_element(node)?),
        _ => return Ok(None), // Ignore unknown elements
    };
    
    Ok(Some(element))
}

/// Parse group element (handle replication, nested groups)
/// 
/// T111: Groups can be replicated to create arrays of identical structures.
/// Example: A decoder with 8 identical output configurations.
/// 
/// Attributes:
/// - offset: Base memory address for this group (default: 0)
/// - replication: Number of instances (default: 1)
/// 
/// Child elements:
/// - <repname>: Template for instance names (e.g., "Output {}", "Channel {}")
/// - Any data elements (int, string, eventid, nested groups, etc.)
/// 
/// Replication creates N identical copies with computed addresses:
/// Instance N address = base_offset + (N * total_size_of_group)
fn parse_group(node: Node) -> Result<Group, String> {
    // Extract offset attribute (default: 0 if not specified)
    let offset = node.attribute("offset")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    // Extract replication count (default: 1 for non-replicated groups)
    let replication = node.attribute("replication")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(1);
    
    let mut name = None;
    let mut description = None;
    let mut repname = Vec::new();
    let mut elements = Vec::new();
    let hints = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => name = child.text().map(|s| s.to_string()),
            "description" => description = child.text().map(|s| s.to_string()),
            "repname" => {
                if let Some(text) = child.text() {
                    repname.push(text.to_string());
                }
            }
            _ => {
                // Try to parse as data element (recursive)
                if let Some(element) = parse_data_element(child)? {
                    elements.push(element);
                }
            }
        }
    }
    
    Ok(Group {
        name,
        description,
        offset,
        replication,
        repname,
        elements,
        hints,
    })
}

/// Parse int element (extract size, min, max, default, map)
fn parse_int_element(node: Node) -> Result<IntElement, String> {
    let size = node.attribute("size")
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(1);
    
    // Validate size
    if ![1, 2, 4, 8].contains(&size) {
        return Err(format!("Invalid int size: {}. Must be 1, 2, 4, or 8", size));
    }
    
    let offset = node.attribute("offset")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let mut name = None;
    let mut description = None;
    let mut min = None;
    let mut max = None;
    let mut default = None;
    let mut map = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => name = child.text().map(|s| s.to_string()),
            "description" => description = child.text().map(|s| s.to_string()),
            "min" => min = child.text().and_then(|s| s.parse::<i64>().ok()),
            "max" => max = child.text().and_then(|s| s.parse::<i64>().ok()),
            "default" => default = child.text().and_then(|s| s.parse::<i64>().ok()),
            "map" => map = Some(parse_map(child)),
            _ => {}
        }
    }
    
    Ok(IntElement {
        name,
        description,
        size,
        offset,
        min,
        max,
        default,
        map,
    })
}

/// Parse eventid element (always 8 bytes)
fn parse_eventid_element(node: Node) -> Result<EventIdElement, String> {
    let offset = node.attribute("offset")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let mut name = None;
    let mut description = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => name = child.text().map(|s| s.to_string()),
            "description" => description = child.text().map(|s| s.to_string()),
            _ => {}
        }
    }
    
    Ok(EventIdElement {
        name,
        description,
        offset,
    })
}

/// Parse string element
fn parse_string_element(node: Node) -> Result<StringElement, String> {
    let size = node.attribute("size")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(64);
    
    let offset = node.attribute("offset")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let mut name = None;
    let mut description = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => name = child.text().map(|s| s.to_string()),
            "description" => description = child.text().map(|s| s.to_string()),
            _ => {}
        }
    }
    
    Ok(StringElement {
        name,
        description,
        size,
        offset,
    })
}

/// Parse float element
fn parse_float_element(node: Node) -> Result<FloatElement, String> {
    let offset = node.attribute("offset")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let mut name = None;
    let mut description = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => name = child.text().map(|s| s.to_string()),
            "description" => description = child.text().map(|s| s.to_string()),
            _ => {}
        }
    }
    
    Ok(FloatElement {
        name,
        description,
        offset,
    })
}

/// Parse action element
fn parse_action_element(node: Node) -> Result<ActionElement, String> {
    let offset = node.attribute("offset")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let mut name = None;
    let mut description = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => name = child.text().map(|s| s.to_string()),
            "description" => description = child.text().map(|s| s.to_string()),
            _ => {}
        }
    }
    
    Ok(ActionElement {
        name,
        description,
        offset,
    })
}

/// Parse blob element
fn parse_blob_element(node: Node) -> Result<BlobElement, String> {
    let size = node.attribute("size")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(64);
    
    let offset = node.attribute("offset")
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let mut name = None;
    let mut description = None;
    
    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "name" => name = child.text().map(|s| s.to_string()),
            "description" => description = child.text().map(|s| s.to_string()),
            _ => {}
        }
    }
    
    Ok(BlobElement {
        name,
        description,
        size,
        offset,
    })
}

/// Parse map element
fn parse_map(node: Node) -> Map {
    let mut entries = Vec::new();
    
    for child in node.children().filter(|n| n.is_element()) {
        if child.tag_name().name() == "relation" {
            // Try parsing with property/value child elements (Tower-LCC.xml format)
            let mut property_val: Option<i64> = None;
            let mut value_text: Option<String> = None;
            
            for rel_child in child.children().filter(|n| n.is_element()) {
                match rel_child.tag_name().name() {
                    "property" => {
                        if let Some(text) = rel_child.text() {
                            property_val = text.parse::<i64>().ok();
                        }
                    }
                    "value" => {
                        value_text = rel_child.text().map(|s| s.to_string());
                    }
                    _ => {}
                }
            }
            
            if let (Some(value), Some(label)) = (property_val, value_text) {
                entries.push(MapEntry { value, label });
                continue;
            }
            
            // Fallback: Try parsing with attribute-based format (older format)
            if let (Some(value_str), Some(label)) = (child.attribute("value"), child.text()) {
                if let Ok(value) = value_str.parse::<i64>() {
                    entries.push(MapEntry {
                        value,
                        label: label.to_string(),
                    });
                }
            }
        }
    }
    
    Map { entries }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_cdi() {
        let xml = r#"
            <cdi>
                <segment space="253" origin="0">
                    <name>Test Segment</name>
                    <int size="1" offset="0">
                        <name>Test Int</name>
                    </int>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        assert_eq!(cdi.segments.len(), 1);
        assert_eq!(cdi.segments[0].name, Some("Test Segment".to_string()));
        assert_eq!(cdi.segments[0].space, 253);
    }

    #[test]
    fn test_group_should_render() {
        // Group with name should render
        let group = Group {
            name: Some("Test".to_string()),
            description: None,
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![],
            hints: None,
        };
        assert!(group.should_render());

        // Empty group should not render (Footnote 4)
        let empty_group = Group {
            name: None,
            description: None,
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![],
            hints: None,
        };
        assert!(!empty_group.should_render());
    }

    // T043f: Unit tests for parse_segment
    #[test]
    fn test_parse_segment_basic() {
        let xml = r#"
            <cdi>
                <segment space="253" origin="128">
                    <name>Config Memory</name>
                    <description>Main configuration space</description>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        assert_eq!(cdi.segments.len(), 1);
        
        let seg = &cdi.segments[0];
        assert_eq!(seg.space, 253);
        assert_eq!(seg.origin, 128);
        assert_eq!(seg.name, Some("Config Memory".to_string()));
        assert_eq!(seg.description, Some("Main configuration space".to_string()));
    }

    #[test]
    fn test_parse_segment_default_origin() {
        let xml = r#"
            <cdi>
                <segment space="251">
                    <name>ACDI</name>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        assert_eq!(cdi.segments[0].origin, 0, "Origin should default to 0");
    }

    #[test]
    fn test_parse_segment_with_elements() {
        let xml = r#"
            <cdi>
                <segment space="253" origin="0">
                    <int size="1">
                        <name>Element 1</name>
                    </int>
                    <string size="32">
                        <name>Element 2</name>
                    </string>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        assert_eq!(cdi.segments[0].elements.len(), 2);
    }

    // T043g: Unit tests for parse_group
    #[test]
    fn test_parse_group_simple() {
        let xml = r#"
            <cdi>
                <segment space="253">
                    <group>
                        <name>Test Group</name>
                        <description>A test group</description>
                    </group>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::Group(group) = &cdi.segments[0].elements[0] else {
            panic!("Expected Group element");
        };
        
        assert_eq!(group.name, Some("Test Group".to_string()));
        assert_eq!(group.description, Some("A test group".to_string()));
        assert_eq!(group.replication, 1, "Default replication should be 1");
    }

    #[test]
    fn test_parse_group_with_replication() {
        let xml = r#"
            <cdi>
                <segment space="253">
                    <group replication="16">
                        <name>Line</name>
                        <repname>Line</repname>
                    </group>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::Group(group) = &cdi.segments[0].elements[0] else {
            panic!("Expected Group element");
        };
        
        assert_eq!(group.replication, 16);
        assert_eq!(group.repname, vec!["Line".to_string()]);
    }

    #[test]
    fn test_parse_nested_groups() {
        let xml = r#"
            <cdi>
                <segment space="253">
                    <group>
                        <name>Outer</name>
                        <group>
                            <name>Inner</name>
                            <int size="1">
                                <name>Value</name>
                            </int>
                        </group>
                    </group>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::Group(outer) = &cdi.segments[0].elements[0] else {
            panic!("Expected Group element");
        };
        
        assert_eq!(outer.name, Some("Outer".to_string()));
        assert_eq!(outer.elements.len(), 1);
        
        let DataElement::Group(inner) = &outer.elements[0] else {
            panic!("Expected nested Group element");
        };
        
        assert_eq!(inner.name, Some("Inner".to_string()));
        assert_eq!(inner.elements.len(), 1);
    }

    #[test]
    fn test_parse_group_footnote4_filtering() {
        // Empty groups should be filtered out per Footnote 4
        let xml = r#"
            <cdi>
                <segment space="253">
                    <group>
                        <!-- Empty group with no name, description, or elements -->
                    </group>
                    <group>
                        <name>Valid Group</name>
                    </group>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        assert_eq!(cdi.segments[0].elements.len(), 1, "Empty group should be filtered");
        
        let DataElement::Group(group) = &cdi.segments[0].elements[0] else {
            panic!("Expected Group element");
        };
        assert_eq!(group.name, Some("Valid Group".to_string()));
    }

    // T043h: Unit tests for parse_int_element
    #[test]
    fn test_parse_int_element_basic() {
        let xml = r#"
            <cdi>
                <segment space="253">
                    <int size="2" offset="10">
                        <name>Speed</name>
                        <description>Motor speed</description>
                    </int>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::Int(int_elem) = &cdi.segments[0].elements[0] else {
            panic!("Expected Int element");
        };
        
        assert_eq!(int_elem.size, 2);
        assert_eq!(int_elem.offset, 10);
        assert_eq!(int_elem.name, Some("Speed".to_string()));
    }

    #[test]
    fn test_parse_int_element_size_validation() {
        // Valid sizes: 1, 2, 4, 8
        for size in [1, 2, 4, 8] {
            let xml = format!(
                r#"<cdi><segment space="253"><int size="{}"><name>Test</name></int></segment></cdi>"#,
                size
            );
            assert!(parse_cdi(&xml).is_ok(), "Size {} should be valid", size);
        }
        
        // Invalid size: 3
        let xml = r#"<cdi><segment space="253"><int size="3"><name>Test</name></int></segment></cdi>"#;
        assert!(parse_cdi(xml).is_err(), "Size 3 should be invalid");
    }

    #[test]
    fn test_parse_int_element_with_constraints() {
        let xml = r#"
            <cdi>
                <segment space="253">
                    <int size="1">
                        <name>Volume</name>
                        <min>0</min>
                        <max>100</max>
                        <default>50</default>
                    </int>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::Int(int_elem) = &cdi.segments[0].elements[0] else {
            panic!("Expected Int element");
        };
        
        assert_eq!(int_elem.min, Some(0));
        assert_eq!(int_elem.max, Some(100));
        assert_eq!(int_elem.default, Some(50));
    }

    #[test]
    fn test_parse_int_element_with_map() {
        let xml = r#"
            <cdi>
                <segment space="253">
                    <int size="1">
                        <name>Mode</name>
                        <map>
                            <relation><property>0</property><value>Disabled</value></relation>
                            <relation><property>1</property><value>Active Hi</value></relation>
                            <relation><property>2</property><value>Active Lo</value></relation>
                        </map>
                    </int>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::Int(int_elem) = &cdi.segments[0].elements[0] else {
            panic!("Expected Int element");
        };
        
        let map = int_elem.map.as_ref().expect("Map should be present");
        assert_eq!(map.entries.len(), 3);
        assert_eq!(map.entries[0].value, 0);
        assert_eq!(map.entries[0].label, "Disabled");
        assert_eq!(map.entries[2].value, 2);
        assert_eq!(map.entries[2].label, "Active Lo");
    }

    // T043i: Unit tests for parse_eventid_element
    #[test]
    fn test_parse_eventid_element() {
        let xml = r#"
            <cdi>
                <segment space="253">
                    <eventid offset="20">
                        <name>Power OK</name>
                        <description>Sent when power is restored</description>
                    </eventid>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::EventId(eventid) = &cdi.segments[0].elements[0] else {
            panic!("Expected EventId element");
        };
        
        assert_eq!(eventid.offset, 20);
        assert_eq!(eventid.name, Some("Power OK".to_string()));
        assert_eq!(eventid.description, Some("Sent when power is restored".to_string()));
    }

    #[test]
    fn test_parse_eventid_default_offset() {
        let xml = r#"
            <cdi>
                <segment space="253">
                    <eventid>
                        <name>Event</name>
                    </eventid>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::EventId(eventid) = &cdi.segments[0].elements[0] else {
            panic!("Expected EventId element");
        };
        
        assert_eq!(eventid.offset, 0, "Offset should default to 0");
    }

    #[test]
    fn test_parse_cdi_with_identification() {
        let xml = r#"
            <cdi>
                <identification>
                    <manufacturer>RR-CirKits</manufacturer>
                    <model>Tower-LCC</model>
                    <hardwareVersion>rev-D</hardwareVersion>
                    <softwareVersion>rev-C6</softwareVersion>
                </identification>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let ident = cdi.identification.expect("Identification should be present");
        
        assert_eq!(ident.manufacturer, Some("RR-CirKits".to_string()));
        assert_eq!(ident.model, Some("Tower-LCC".to_string()));
        assert_eq!(ident.hardware_version, Some("rev-D".to_string()));
        assert_eq!(ident.software_version, Some("rev-C6".to_string()));
    }

    #[test]
    fn test_parse_string_element() {
        let xml = r#"
            <cdi>
                <segment space="251">
                    <string size="63" offset="5">
                        <name>Node Name</name>
                    </string>
                </segment>
            </cdi>
        "#;
        
        let cdi = parse_cdi(xml).expect("Failed to parse CDI");
        let DataElement::String(string_elem) = &cdi.segments[0].elements[0] else {
            panic!("Expected String element");
        };
        
        assert_eq!(string_elem.size, 63);
        assert_eq!(string_elem.offset, 5);
        assert_eq!(string_elem.name, Some("Node Name".to_string()));
    }

    #[test]
    fn test_parse_malformed_xml() {
        let xml = r#"<cdi><segment"#; // Incomplete XML
        
        assert!(parse_cdi(xml).is_err(), "Malformed XML should return error");
    }

    #[test]
    fn test_parse_invalid_root_element() {
        let xml = r#"<notcdi><segment space="253"/></notcdi>"#;
        
        let result = parse_cdi(xml);
        assert!(result.is_err(), "Invalid root element should return error");
        assert!(result.unwrap_err().contains("Root element must be 'cdi'"));
    }

    #[test]
    fn test_parse_segment_missing_space() {
        let xml = r#"<cdi><segment><name>Test</name></segment></cdi>"#;
        
        let result = parse_cdi(xml);
        assert!(result.is_err(), "Missing space attribute should return error");
        assert!(result.unwrap_err().contains("Segment missing 'space' attribute"));
    }
}

// T043e: Property-based tests using proptest
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    // Generate valid CDI space values (common spaces: 251=ACDI, 253=Config, 254=All, 255=Config-All)
    fn valid_space() -> impl Strategy<Value = u8> {
        prop::sample::select(vec![251u8, 253, 254, 255])
    }

    // Generate valid int sizes
    fn valid_int_size() -> impl Strategy<Value = u8> {
        prop::sample::select(vec![1u8, 2, 4, 8])
    }

    // Property: Parsing a segment should preserve space and origin
    proptest! {
        #[test]
        fn prop_segment_roundtrip(
            space in valid_space(),
            origin in 0i32..10000i32,
            name in "[a-zA-Z0-9 ]{1,50}",
        ) {
            let xml = format!(
                r#"<cdi><segment space="{}" origin="{}"><name>{}</name></segment></cdi>"#,
                space, origin, name
            );
            
            let cdi = parse_cdi(&xml).expect("Valid XML should parse");
            assert_eq!(cdi.segments.len(), 1);
            assert_eq!(cdi.segments[0].space, space);
            assert_eq!(cdi.segments[0].origin, origin);
            assert_eq!(cdi.segments[0].name, Some(name));
        }
    }

    // Property: Parsing an int element should preserve size and constraints
    proptest! {
        #[test]
        fn prop_int_element_roundtrip(
            size in valid_int_size(),
            offset in 0i32..1000i32,
            min_val in 0i64..100i64,
            max_val in 100i64..1000i64,
        ) {
            let xml = format!(
                r#"<cdi><segment space="253"><int size="{}" offset="{}"><min>{}</min><max>{}</max></int></segment></cdi>"#,
                size, offset, min_val, max_val
            );
            
            let cdi = parse_cdi(&xml).expect("Valid XML should parse");
            let DataElement::Int(int_elem) = &cdi.segments[0].elements[0] else {
                panic!("Expected Int element");
            };
            
            assert_eq!(int_elem.size, size);
            assert_eq!(int_elem.offset, offset);
            assert_eq!(int_elem.min, Some(min_val));
            assert_eq!(int_elem.max, Some(max_val));
        }
    }

    // Property: Parsing groups with replication should preserve count
    proptest! {
        #[test]
        fn prop_group_replication(
            replication in 1u32..100u32,
        ) {
            let xml = format!(
                r#"<cdi><segment space="253"><group replication="{}"><name>Test</name></group></segment></cdi>"#,
                replication
            );
            
            let cdi = parse_cdi(&xml).expect("Valid XML should parse");
            let DataElement::Group(group) = &cdi.segments[0].elements[0] else {
                panic!("Expected Group element");
            };
            
            assert_eq!(group.replication, replication);
        }
    }

    // Property: EventId elements should always parse with correct offset
    proptest! {
        #[test]
        fn prop_eventid_offset(
            offset in -1000i32..1000i32,
        ) {
            let xml = format!(
                r#"<cdi><segment space="253"><eventid offset="{}"><name>Event</name></eventid></segment></cdi>"#,
                offset
            );
            
            let cdi = parse_cdi(&xml).expect("Valid XML should parse");
            let DataElement::EventId(eventid) = &cdi.segments[0].elements[0] else {
                panic!("Expected EventId element");
            };
            
            assert_eq!(eventid.offset, offset);
        }
    }

    // Property: Empty groups should be filtered (Footnote 4)
    proptest! {
        #[test]
        fn prop_empty_group_filtered(
            valid_name in "[a-zA-Z ]{1,20}",
        ) {
            // Create XML with one empty group and one valid group
            let xml = format!(
                r#"<cdi><segment space="253"><group></group><group><name>{}</name></group></segment></cdi>"#,
                valid_name
            );
            
            let cdi = parse_cdi(&xml).expect("Valid XML should parse");
            // Only the named group should remain (empty one filtered)
            assert_eq!(cdi.segments[0].elements.len(), 1);
            
            let DataElement::Group(group) = &cdi.segments[0].elements[0] else {
                panic!("Expected Group element");
            };
            assert_eq!(group.name, Some(valid_name));
        }
    }}