//! CDI Parsing Integration Tests
//! 
//! T043j-T043k: Integration tests with real CDI XML samples and malformed XML tests

use lcc_rs::cdi::parser::parse_cdi;
use lcc_rs::cdi::{DataElement, Cdi};

// Tower-LCC.xml is a real-world CDI from the archived 003 Miller Columns spec.
const TOWER_LCC_XML: &str = include_str!("../../specs/archive/003-miller-columns/Tower-LCC.xml");

// T043j: Integration tests with real CDI XML samples
#[test]
fn test_parse_tower_lcc_structure() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // Validate identification
    let ident = cdi.identification.expect("Should have identification");
    assert_eq!(ident.manufacturer, Some("RR-CirKits".to_string()));
    assert_eq!(ident.model, Some("Tower-LCC".to_string()));
    assert_eq!(ident.hardware_version, Some("rev-D".to_string()));
    assert_eq!(ident.software_version, Some("rev-C6".to_string()));
    
    // Tower-LCC has 6 segments
    assert_eq!(cdi.segments.len(), 6, "Tower-LCC should have 6 segments");
}

#[test]
fn test_parse_tower_lcc_segments() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // Segment 0: NODE ID (space 251)
    assert_eq!(cdi.segments[0].space, 251);
    assert_eq!(cdi.segments[0].name, Some("NODE ID".to_string()));
    
    // Segment 1: Node Power Monitor (space 253, origin 7744)
    assert_eq!(cdi.segments[1].space, 253);
    assert_eq!(cdi.segments[1].origin, 7744);
    assert_eq!(cdi.segments[1].name, Some("Node Power Monitor".to_string()));
    
    // Segment 2: Port I/O (space 253, origin 128)
    assert_eq!(cdi.segments[2].space, 253);
    assert_eq!(cdi.segments[2].origin, 128);
    assert_eq!(cdi.segments[2].name, Some("Port I/O".to_string()));
    
    // Segment 3: Conditionals (space 253, origin 2528)
    assert_eq!(cdi.segments[3].space, 253);
    assert_eq!(cdi.segments[3].origin, 2528);
    assert_eq!(cdi.segments[3].name, Some("Conditionals".to_string()));
    
    // Segment 4: Track Receiver (space 253, origin 7104)
    assert_eq!(cdi.segments[4].space, 253);
    assert_eq!(cdi.segments[4].origin, 7104);
    assert_eq!(cdi.segments[4].name, Some("Track Receiver".to_string()));
    
    // Segment 5: Track Transmitter (space 253, origin 7424)
    assert_eq!(cdi.segments[5].space, 253);
    assert_eq!(cdi.segments[5].origin, 7424);
    assert_eq!(cdi.segments[5].name, Some("Track Transmitter".to_string()));
}

#[test]
fn test_parse_tower_lcc_replication() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // Port I/O segment (index 2) has replicated group with 16 lines
    let port_io_segment = &cdi.segments[2];
    
    let DataElement::Group(line_group) = &port_io_segment.elements[0] else {
        panic!("First element should be a replicated group");
    };
    
    assert_eq!(line_group.replication, 16, "Port I/O has 16 replicated lines");
    assert_eq!(line_group.name, Some("Line".to_string()));
    assert_eq!(line_group.repname, vec!["Line".to_string()]);
}

#[test]
fn test_parse_tower_lcc_nested_groups() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // Port I/O segment has nested groups (Line > Delay with replication=2)
    let port_io_segment = &cdi.segments[2];
    
    let DataElement::Group(line_group) = &port_io_segment.elements[0] else {
        panic!("Expected Line group");
    };
    
    // Find nested Delay group
    let delay_group = line_group.elements.iter()
        .find_map(|e| {
            if let DataElement::Group(g) = e {
                if g.name.as_ref().map(|n| n.as_str()) == Some("Delay") {
                    return Some(g);
                }
            }
            None
        })
        .expect("Should have Delay nested group");
    
    assert_eq!(delay_group.replication, 2, "Delay group has 2 intervals");
    assert_eq!(delay_group.repname, vec!["Interval".to_string()]);
}

#[test]
fn test_parse_tower_lcc_eventid_elements() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // Node Power Monitor segment has EventID elements
    let power_monitor = &cdi.segments[1];
    
    let eventid_count = power_monitor.elements.iter()
        .filter(|e| matches!(e, DataElement::EventId(_)))
        .count();
    
    assert_eq!(eventid_count, 2, "Power Monitor has 2 EventID elements");
    
    // Check first EventID
    let DataElement::EventId(evt) = &power_monitor.elements[1] else {
        panic!("Expected EventId element at index 1");
    };
    
    assert_eq!(evt.name, Some("Power OK".to_string()));
    assert_eq!(evt.description, Some("EventID".to_string()));
}

#[test]
fn test_parse_tower_lcc_int_with_map() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // Port I/O Line group has int elements with map values
    let port_io_segment = &cdi.segments[2];
    
    let DataElement::Group(line_group) = &port_io_segment.elements[0] else {
        panic!("Expected Line group");
    };
    
    // Find "Output Function" int element with map
    let output_func = line_group.elements.iter()
        .find_map(|e| {
            if let DataElement::Int(i) = e {
                if i.name.as_ref().map(|n| n.as_str()) == Some("Output Function") {
                    return Some(i);
                }
            }
            None
        })
        .expect("Should have Output Function element");
    
    let map = output_func.map.as_ref().expect("Output Function should have map");
    
    // Tower-LCC Output Function has 18 map entries
    assert!(map.entries.len() >= 10, "Output Function should have multiple map entries");
    
    // Check specific map entry
    let no_func = map.entries.iter()
        .find(|e| e.value == 0)
        .expect("Should have value 0");
    
    assert_eq!(no_func.label, "No Function");
}

#[test]
fn test_parse_tower_lcc_string_elements() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // NODE ID segment has string elements
    let node_id_segment = &cdi.segments[0];
    
    let DataElement::Group(name_group) = &node_id_segment.elements[0] else {
        panic!("Expected group in NODE ID segment");
    };
    
    let DataElement::String(node_name) = &name_group.elements[0] else {
        panic!("Expected String element");
    };
    
    assert_eq!(node_name.name, Some("Node Name".to_string()));
    assert_eq!(node_name.size, 63, "Node Name should be 63 bytes");
}

#[test]
fn test_parse_tower_lcc_max_depth() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // Calculate max nesting depth
    let max_depth = lcc_rs::cdi::hierarchy::calculate_max_depth(&cdi);
    
    // Tower-LCC has nested groups: Segment > Line (Group) > Delay (Group) > Elements
    // Depth: 1 (segment) + 1 (Line) + 1 (Delay) + 1 (elements) = 4
    assert!(max_depth >= 4, "Tower-LCC should have depth of at least 4");
}

#[test]
fn test_parse_tower_lcc_group_expansion() {
    let cdi = parse_cdi(TOWER_LCC_XML).expect("Failed to parse Tower-LCC.xml");
    
    // Test expanding the 16-line Port I/O group
    let port_io_segment = &cdi.segments[2];
    
    let DataElement::Group(line_group) = &port_io_segment.elements[0] else {
        panic!("Expected Line group");
    };
    
    let base_addr = port_io_segment.origin;
    let expanded = line_group.expand_replications(base_addr);
    
    assert_eq!(expanded.len(), 16, "Should expand to 16 instances");
    
    // Check naming
    assert_eq!(expanded[0].name, "Line 1");
    assert_eq!(expanded[15].name, "Line 16");
    
    // Check addresses are calculated correctly
    assert_eq!(expanded[0].address, base_addr);
    assert!(expanded[15].address > expanded[0].address, "Last instance should have higher address");
}

// T043k: Malformed XML tests
#[test]
fn test_malformed_xml_incomplete() {
    let xml = r#"<cdi><segment space="253""#;
    
    let result = parse_cdi(xml);
    assert!(result.is_err(), "Incomplete XML should fail");
}

#[test]
fn test_malformed_xml_invalid_root() {
    let xml = r#"<not_cdi><segment space="253"></segment></not_cdi>"#;
    
    let result = parse_cdi(xml);
    assert!(result.is_err(), "Invalid root element should fail");
    
    let err = result.unwrap_err();
    assert!(err.contains("Root element must be 'cdi'"), "Error should mention root element");
}

#[test]
fn test_malformed_xml_missing_required_attribute() {
    let xml = r#"<cdi><segment><name>No Space</name></segment></cdi>"#;
    
    let result = parse_cdi(xml);
    assert!(result.is_err(), "Missing 'space' attribute should fail");
    
    let err = result.unwrap_err();
    assert!(err.contains("Segment missing 'space' attribute"));
}

#[test]
fn test_malformed_xml_invalid_int_size() {
    let xml = r#"
        <cdi>
            <segment space="253">
                <int size="3">
                    <name>Invalid Size</name>
                </int>
            </segment>
        </cdi>
    "#;
    
    let result = parse_cdi(xml);
    assert!(result.is_err(), "Invalid int size should fail");
    
    let err = result.unwrap_err();
    assert!(err.contains("Invalid int size"), "Error should mention invalid size");
}

#[test]
fn test_malformed_xml_invalid_attribute_type() {
    let xml = r#"<cdi><segment space="not_a_number"></segment></cdi>"#;
    
    let result = parse_cdi(xml);
    assert!(result.is_err(), "Non-numeric space should fail");
}

#[test]
fn test_graceful_degradation_unknown_elements() {
    // CDI parser should ignore unknown elements instead of failing
    let xml = r#"
        <cdi>
            <segment space="253">
                <unknown_element>
                    <data>This should be ignored</data>
                </unknown_element>
                <int size="1">
                    <name>Valid Element</name>
                </int>
            </segment>
        </cdi>
    "#;
    
    let cdi = parse_cdi(xml).expect("Unknown elements should be ignored gracefully");
    
    // Unknown element should be filtered, only int element should remain
    assert_eq!(cdi.segments[0].elements.len(), 1);
    
    let DataElement::Int(int_elem) = &cdi.segments[0].elements[0] else {
        panic!("Expected Int element");
    };
    
    assert_eq!(int_elem.name, Some("Valid Element".to_string()));
}

#[test]
fn test_graceful_degradation_missing_optional_fields() {
    // Elements should parse even when optional fields are missing
    let xml = r#"
        <cdi>
            <segment space="253">
                <int size="1">
                    <!-- No name, description, or constraints -->
                </int>
                <eventid>
                    <!-- No name or description -->
                </eventid>
            </segment>
        </cdi>
    "#;
    
    let cdi = parse_cdi(xml).expect("Missing optional fields should be allowed");
    
    assert_eq!(cdi.segments[0].elements.len(), 2);
    
    let DataElement::Int(int_elem) = &cdi.segments[0].elements[0] else {
        panic!("Expected Int element");
    };
    
    assert_eq!(int_elem.name, None);
    assert_eq!(int_elem.description, None);
}

#[test]
fn test_edge_case_empty_cdi() {
    let xml = r#"<cdi></cdi>"#;
    
    let cdi = parse_cdi(xml).expect("Empty CDI should parse");
    
    assert!(cdi.identification.is_none());
    assert!(cdi.acdi.is_none());
    assert_eq!(cdi.segments.len(), 0);
}

#[test]
fn test_edge_case_large_replication() {
    // Test with large replication count (100)
    let xml = r#"
        <cdi>
            <segment space="253">
                <group replication="100">
                    <name>Channel</name>
                    <repname>Ch</repname>
                    <int size="1">
                        <name>Value</name>
                    </int>
                </group>
            </segment>
        </cdi>
    "#;
    
    let cdi = parse_cdi(xml).expect("Large replication should parse");
    
    let DataElement::Group(group) = &cdi.segments[0].elements[0] else {
        panic!("Expected Group element");
    };
    
    assert_eq!(group.replication, 100);
    
    // Test expansion
    let expanded = group.expand_replications(0);
    assert_eq!(expanded.len(), 100);
    assert_eq!(expanded[0].name, "Ch 1");
    assert_eq!(expanded[99].name, "Ch 100");
}

#[test]
fn test_edge_case_deep_nesting() {
    // Test deeply nested groups (8 levels)
    let xml = r#"
        <cdi>
            <segment space="253">
                <group>
                    <name>Level 1</name>
                    <group>
                        <name>Level 2</name>
                        <group>
                            <name>Level 3</name>
                            <group>
                                <name>Level 4</name>
                                <group>
                                    <name>Level 5</name>
                                    <group>
                                        <name>Level 6</name>
                                        <group>
                                            <name>Level 7</name>
                                            <group>
                                                <name>Level 8</name>
                                                <int size="1"><name>Deep</name></int>
                                            </group>
                                        </group>
                                    </group>
                                </group>
                            </group>
                        </group>
                    </group>
                </group>
            </segment>
        </cdi>
    "#;
    
    let cdi = parse_cdi(xml).expect("Deep nesting should parse");
    
    // Calculate depth
    let depth = lcc_rs::cdi::hierarchy::calculate_max_depth(&cdi);
    
    // Depth: 1 (segment) + 8 (groups) + 1 (int element) = 10
    assert_eq!(depth, 10, "Should handle 8 levels of nested groups");
}

#[test]
fn test_spacer_groups_correct_field_addresses() {
    // Regression test for the UWT-100 address bug: spacer groups (<group offset='N'/>)
    // were previously dropped by the Footnote-4 filter, causing all subsequent fields
    // to be calculated at wrong addresses.
    //
    // Layout (segment origin=512, space=253):
    //   int(1)   Show Welcome Tutorial  -> address 512
    //   int(1)   Regulatory Region      -> address 513
    //   group offset=6 (spacer)         -> advances cursor to 520
    //   group replication=2 (Profiles)  -> first instance at 520
    //     string(32) SSID               -> 520
    //     int(2)     Port               -> 552
    //     group offset=4 (inner spacer) -> advances to 558 (end of instance)
    //   second Profiles instance        -> 558
    //     string(32) SSID               -> 558
    //     int(2)     Port               -> 590
    let xml = r#"
        <cdi>
            <segment space='253' origin='512'>
                <int size='1'><name>Show Welcome Tutorial</name></int>
                <int size='1'><name>Regulatory Region</name></int>
                <group offset='6'/>
                <group replication='2'>
                    <name>Profiles</name>
                    <repname>Profile</repname>
                    <string size='32'><name>SSID</name></string>
                    <int size='2'><name>Port</name></int>
                    <group offset='4'/>
                </group>
            </segment>
        </cdi>
    "#;

    let cdi = parse_cdi(xml).expect("Should parse");
    let seg = &cdi.segments[0];
    assert_eq!(seg.origin, 512);

    // The segment elements, in order, should be:
    //   0: Int "Show Welcome Tutorial"
    //   1: Int "Regulatory Region"
    //   2: Group (spacer, offset=6)
    //   3: Group (Profiles, replication=2)
    assert_eq!(seg.elements.len(), 4, "Spacer group must be preserved as element 2");

    let DataElement::Group(spacer) = &seg.elements[2] else {
        panic!("Expected spacer group at index 2");
    };
    assert_eq!(spacer.offset, 6);
    assert!(spacer.elements.is_empty());

    let DataElement::Group(profiles) = &seg.elements[3] else {
        panic!("Expected Profiles group at index 3");
    };
    assert_eq!(profiles.replication, 2);

    // calculate_size() of one Profiles instance: 32 (SSID) + 2 (Port) + 4 (inner spacer) = 38
    let stride = profiles.calculate_size();
    assert_eq!(stride, 38, "Profile stride must include inner spacer offset");

    // Verify addresses via expand_replications.
    // The Profiles group starts at: 512 (origin) + 1+1 (two ints) + 6 (outer spacer offset) = 520.
    let profile_base = 512 + 1 + 1 + 6;  // = 520
    let expanded = profiles.expand_replications(profile_base);
    assert_eq!(expanded.len(), 2);

    assert_eq!(expanded[0].address, 520, "Profile 1 base address");
    assert_eq!(expanded[1].address, 520 + 38, "Profile 2 base address = 558");
}

#[test]
fn test_edge_case_negative_offset() {
    // Tower-LCC.xml contains negative offsets
    let xml = r#"
        <cdi>
            <segment space="253" origin="1000">
                <eventid offset="-50">
                    <name>Negative Offset Event</name>
                </eventid>
            </segment>
        </cdi>
    "#;
    
    let cdi = parse_cdi(xml).expect("Negative offset should parse");
    
    let DataElement::EventId(eventid) = &cdi.segments[0].elements[0] else {
        panic!("Expected EventId element");
    };
    
    assert_eq!(eventid.offset, -50);
}
