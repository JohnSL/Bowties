//! CDI navigation and hierarchy helpers
//!
//! This module provides functions for navigating CDI structures,
//! expanding replicated groups, and calculating hierarchy metadata.

use super::*;

/// Expanded group instance from replication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpandedGroup {
    /// Instance index (0-based)
    pub index: u32,
    
    /// Computed name for this instance
    pub name: String,
    
    /// Computed memory address for this instance
    pub address: i32,
    
    /// Elements in this instance (cloned from template)
    pub elements: Vec<DataElement>,
}

impl Group {
    /// Expand replicated group into individual instances
    ///
    /// Generates N instances with computed names and addresses based on replication count.
    ///
    /// # Arguments
    /// * `base_address` - Starting memory address for the replicated group
    ///
    /// # Returns
    /// Vector of expanded group instances
    pub fn expand_replications(&self, base_address: i32) -> Vec<ExpandedGroup> {
        let size_per_instance = self.calculate_size();
        
        (0..self.replication)
            .map(|i| ExpandedGroup {
                index: i,
                name: self.compute_repname(i),
                address: base_address + (i as i32 * size_per_instance),
                elements: self.elements.clone(),
            })
            .collect()
    }
    
    /// Compute instance name from repname template
    ///
    /// Handles numbering per CDI spec:
    /// - If repname contains single string: append instance number (1-based)
    /// - If repname is empty: use group name + instance number
    ///
    /// # Arguments
    /// * `instance_index` - 0-based instance index
    ///
    /// # Returns
    /// Computed instance name
    pub fn compute_repname(&self, instance_index: u32) -> String {
        let instance_num = instance_index + 1; // 1-based numbering
        
        if !self.repname.is_empty() {
            // Use first repname entry as template
            format!("{} {}", self.repname[0], instance_num)
        } else if let Some(ref name) = self.name {
            // Use group name as template
            format!("{} {}", name, instance_num)
        } else {
            // Fallback to generic numbering
            format!("Instance {}", instance_num)
        }
    }
    
    /// Calculate total size of this group in bytes
    ///
    /// Recursively calculates size of all child elements
    fn calculate_size(&self) -> i32 {
        self.elements.iter().map(calculate_element_size).sum()
    }
}

/// Calculate size of a data element in bytes
fn calculate_element_size(element: &DataElement) -> i32 {
    match element {
        DataElement::Group(g) => {
            let size_per_instance = g.calculate_size();
            size_per_instance * g.replication as i32
        }
        DataElement::Int(i) => i.size as i32,
        DataElement::String(s) => s.size as i32,
        DataElement::EventId(_) => 8, // Event IDs are always 8 bytes
        DataElement::Float(_) => 4, // Assume 32-bit floats
        DataElement::Action(_) => 1, // Actions are typically 1 byte
        DataElement::Blob(b) => b.size as i32,
    }
}

/// Calculate maximum nesting depth in CDI structure
///
/// Traverses the entire hierarchy to find the deepest level.
///
/// # Arguments
/// * `cdi` - The CDI structure to analyze
///
/// # Returns
/// Maximum depth (1 = segments only, 2+ = nested groups/elements)
pub fn calculate_max_depth(cdi: &Cdi) -> usize {
    let mut max_depth = 1; // Minimum depth (segments)
    
    for segment in &cdi.segments {
        let segment_depth = calculate_elements_depth(&segment.elements, 2);
        max_depth = max_depth.max(segment_depth);
    }
    
    max_depth
}

/// Calculate depth of a list of elements (helper for calculate_max_depth)
fn calculate_elements_depth(elements: &[DataElement], current_depth: usize) -> usize {
    let mut max_depth = current_depth;
    
    for element in elements {
        if let DataElement::Group(group) = element {
            let group_depth = calculate_elements_depth(&group.elements, current_depth + 1);
            max_depth = max_depth.max(group_depth);
        }
    }
    
    max_depth
}

/// Navigate to a specific element using a path array
///
/// Follows the path from root through segments → groups → elements to find the target.
///
/// # Arguments
/// * `cdi` - The CDI structure to navigate
/// * `path` - Array of element IDs representing the navigation path
///
/// # Returns
/// * `Ok(&DataElement)` - Reference to the element at the path
/// * `Err(String)` - Error if path is invalid
pub fn navigate_to_path<'a>(cdi: &'a Cdi, path: &[String]) -> Result<NavigationResult<'a>, String> {
    if path.is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    
    // First element should be a segment ID
    let segment_id = &path[0];
    
    // Find segment (segment IDs are typically "seg-{name}" or index-based)
    let segment = find_segment_by_id(cdi, segment_id)
        .ok_or_else(|| format!("Segment not found: {}", segment_id))?;
    
    if path.len() == 1 {
        return Ok(NavigationResult::Segment(segment));
    }
    
    // Navigate through elements
    navigate_elements(&segment.elements, &path[1..])
}

/// Result of navigation - can be a segment or an element
#[derive(Debug)]
pub enum NavigationResult<'a> {
    Segment(&'a Segment),
    Element(&'a DataElement),
}

/// Find segment by ID helper
fn find_segment_by_id<'a>(cdi: &'a Cdi, segment_id: &str) -> Option<&'a Segment> {
    // Parse index-based segment ID: seg:N
    if let Some(index_str) = segment_id.strip_prefix("seg:") {
        if let Ok(index) = index_str.parse::<usize>() {
            return cdi.segments.get(index);
        }
    }
    
    None
}

/// Navigate through elements recursively
fn navigate_elements<'a>(elements: &'a [DataElement], path: &[String]) -> Result<NavigationResult<'a>, String> {
    if path.is_empty() {
        return Err("Element path cannot be empty".to_string());
    }
    
    let element_id = &path[0];
    
    // Find the element
    let element = find_element_by_id(elements, element_id)
        .ok_or_else(|| format!("Element not found: {}", element_id))?;
    
    if path.len() == 1 {
        return Ok(NavigationResult::Element(element));
    }
    
    // If there's more path, the current element must be a group
    match element {
        DataElement::Group(group) => {
            navigate_elements(&group.elements, &path[1..])
        }
        _ => Err(format!("Cannot navigate into non-group element: {}", element_id))
    }
}

/// Find element by index-based ID helper
/// 
/// Parses index-based element IDs in format:
/// - "elem:N" for non-replicated elements (N is 0-based index)
/// - "elem:N#I" for replicated group instances (N is 0-based index, I is 1-based instance)
/// 
/// This eliminates ambiguity with CDI element names that contain '#' characters.
fn find_element_by_id<'a>(elements: &'a [DataElement], element_id: &str) -> Option<&'a DataElement> {
    // Parse index-based element ID: elem:N or elem:N#I
    if let Some(index_part) = element_id.strip_prefix("elem:") {
        // Check if this is a replicated instance (elem:N#I)
        if let Some(hash_pos) = index_part.find('#') {
            // Parse element index and instance number
            let index_str = &index_part[..hash_pos];
            let instance_str = &index_part[hash_pos + 1..];
            
            if let (Ok(element_index), Ok(_instance_num)) = (
                index_str.parse::<usize>(),
                instance_str.parse::<usize>()
            ) {
                // For replicated groups, we just need the base element at the index
                // The replication expansion happens in the frontend/pathId generation
                // The element in the array is the template group definition
                return elements.get(element_index);
            }
        } else {
            // Non-replicated element: elem:N
            if let Ok(index) = index_part.parse::<usize>() {
                return elements.get(index);
            }
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_replications_single() {
        // T043c: Test replication=1 (no actual replication)
        let group = Group {
            name: Some("Input".to_string()),
            description: None,
            offset: 0,
            replication: 1,
            repname: vec!["Line".to_string()],
            elements: vec![],
            hints: None,
        };
        
        let expanded = group.expand_replications(100);
        assert_eq!(expanded.len(), 1, "Replication=1 should produce exactly 1 instance");
        assert_eq!(expanded[0].name, "Line 1");
        assert_eq!(expanded[0].index, 0);
        assert_eq!(expanded[0].address, 100);
    }

    #[test]
    fn test_expand_replications_sixteen() {
        // T043c: Test replication=16 (common in Tower-LCC I/O lines)
        let group = Group {
            name: Some("Port".to_string()),
            description: None,
            offset: 0,
            replication: 16,
            repname: vec!["Line".to_string()],
            elements: vec![
                DataElement::Int(IntElement {
                    name: Some("Value".to_string()),
                    description: None,
                    size: 4,
                    offset: 0,
                    min: None,
                    max: None,
                    default: None,
                    map: None,
                }),
            ],
            hints: None,
        };
        
        let expanded = group.expand_replications(1000);
        assert_eq!(expanded.len(), 16, "Replication=16 should produce 16 instances");
        
        // Check first instance
        assert_eq!(expanded[0].name, "Line 1");
        assert_eq!(expanded[0].index, 0);
        assert_eq!(expanded[0].address, 1000);
        
        // Check middle instance
        assert_eq!(expanded[7].name, "Line 8");
        assert_eq!(expanded[7].index, 7);
        assert_eq!(expanded[7].address, 1000 + 7 * 4); // size=4 per instance
        
        // Check last instance
        assert_eq!(expanded[15].name, "Line 16");
        assert_eq!(expanded[15].index, 15);
        assert_eq!(expanded[15].address, 1000 + 15 * 4);
    }

    #[test]
    fn test_expand_replications_hundred() {
        // T043c: Test replication=100 (stress test for large replication counts)
        let group = Group {
            name: Some("Channel".to_string()),
            description: None,
            offset: 0,
            replication: 100,
            repname: vec!["Ch".to_string()],
            elements: vec![
                DataElement::Int(IntElement {
                    name: Some("Value".to_string()),
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
        
        let expanded = group.expand_replications(0);
        assert_eq!(expanded.len(), 100, "Replication=100 should produce 100 instances");
        
        // Spot check instances
        assert_eq!(expanded[0].name, "Ch 1");
        assert_eq!(expanded[0].address, 0);
        
        assert_eq!(expanded[49].name, "Ch 50");
        assert_eq!(expanded[49].address, 49); // size=1, so address = index
        
        assert_eq!(expanded[99].name, "Ch 100");
        assert_eq!(expanded[99].address, 99);
    }

    #[test]
    fn test_compute_repname_with_template() {
        // T043d: Test numbering with repname template
        let group = Group {
            name: Some("Input".to_string()),
            description: None,
            offset: 0,
            replication: 16,
            repname: vec!["Channel".to_string()],
            elements: vec![],
            hints: None,
        };
        
        assert_eq!(group.compute_repname(0), "Channel 1", "First instance should be 1-based");
        assert_eq!(group.compute_repname(5), "Channel 6");
        assert_eq!(group.compute_repname(15), "Channel 16", "Last instance of 16");
    }

    #[test]
    fn test_compute_repname_no_template() {
        // T043d: Test numbering without repname template (fallback to group name)
        let group = Group {
            name: Some("Input".to_string()),
            description: None,
            offset: 0,
            replication: 8,
            repname: vec![],
            elements: vec![],
            hints: None,
        };
        
        assert_eq!(group.compute_repname(0), "Input 1", "Should use group name with number");
        assert_eq!(group.compute_repname(7), "Input 8");
    }

    #[test]
    fn test_compute_repname_no_template_no_name() {
        // T043d: Test fallback when both repname and name are missing
        let group = Group {
            name: None,
            description: Some("Some description".to_string()),
            offset: 0,
            replication: 5,
            repname: vec![],
            elements: vec![],
            hints: None,
        };
        
        assert_eq!(group.compute_repname(0), "Instance 1", "Should use generic fallback");
        assert_eq!(group.compute_repname(4), "Instance 5");
    }

    #[test]
    fn test_calculate_max_depth() {
        let cdi = Cdi {
            identification: None,
            acdi: None,
            segments: vec![
                Segment {
                    name: Some("Test".to_string()),
                    description: None,
                    space: 253,
                    origin: 0,
                    elements: vec![
                        DataElement::Group(Group {
                            name: Some("Outer".to_string()),
                            description: None,
                            offset: 0,
                            replication: 1,
                            repname: vec![],
                            elements: vec![
                                DataElement::Group(Group {
                                    name: Some("Inner".to_string()),
                                    description: None,
                                    offset: 0,
                                    replication: 1,
                                    repname: vec![],
                                    elements: vec![],
                                    hints: None,
                                }),
                            ],
                            hints: None,
                        }),
                    ],
                },
            ],
        };
        
        assert_eq!(calculate_max_depth(&cdi), 4); // segment(1) -> outer group(2) -> inner group(3) -> elements(4)
    }

    #[test]
    fn test_navigate_to_element_with_hash_in_name() {
        // Test navigating to elements with '#' in their CDI names inside replicated groups
        // This was previously failing because name-based parsing couldn't distinguish
        // between replication suffix (Logic#12) and CDI names with # (Variable #1)
        let cdi = Cdi {
            identification: None,
            acdi: None,
            segments: vec![
                Segment {
                    name: Some("Conditionals".to_string()),
                    description: None,
                    space: 253,
                    origin: 0,
                    elements: vec![
                        DataElement::Group(Group {
                            name: Some("Logic".to_string()),
                            description: None,
                            offset: 0,
                            replication: 32,
                            repname: vec!["Logic".to_string()],
                            elements: vec![
                                DataElement::String(crate::cdi::StringElement {
                                    name: Some("Description".to_string()),
                                    description: None,
                                    size: 20,
                                    offset: 0,
                                }),
                                DataElement::Group(Group {
                                    name: Some("Function".to_string()),
                                    description: None,
                                    offset: 20,
                                    replication: 1,
                                    repname: vec![],
                                    elements: vec![],
                                    hints: None,
                                }),
                                DataElement::Group(Group {
                                    name: Some("Variable #1".to_string()),  // Name contains '#'!
                                    description: None,
                                    offset: 24,
                                    replication: 1,
                                    repname: vec![],
                                    elements: vec![
                                        DataElement::Int(crate::cdi::IntElement {
                                            name: Some("Trigger".to_string()),
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
                                }),
                                DataElement::Group(Group {
                                    name: Some("Variable #2".to_string()),  // Name contains '#'!
                                    description: None,
                                    offset: 28,
                                    replication: 1,
                                    repname: vec![],
                                    elements: vec![],
                                    hints: None,
                                }),
                            ],
                            hints: None,
                        }),
                    ],
                },
            ],
        };

        // Test 1: Navigate to segment by index
        let path = vec!["seg:0".to_string()];
        let result = navigate_to_path(&cdi, &path);
        assert!(result.is_ok(), "Should find segment at index 0");
        
        // Test 2: Navigate to replicated group instance #12
        // Path format: seg:0 (segment) -> elem:0#12 (Logic group, instance 12)
        let path = vec!["seg:0".to_string(), "elem:0#12".to_string()];
        let result = navigate_to_path(&cdi, &path);
        assert!(result.is_ok(), "Should find replicated Logic group instance #12");
        
        // Test 3: Navigate to "Variable #1" group inside replicated Logic instance #12
        // Path format: seg:0 -> elem:0#12 (Logic #12) -> elem:2 (Variable #1 is at index 2 in elements array)
        let path = vec![
            "seg:0".to_string(),
            "elem:0#12".to_string(),
            "elem:2".to_string(),  // Variable #1 is at index 2 (after Description and Function)
        ];
        let result = navigate_to_path(&cdi, &path);
        assert!(result.is_ok(), "Should find 'Variable #1' group at index 2");
        
        // Verify we got the right element
        match result.unwrap() {
            NavigationResult::Element(DataElement::Group(g)) => {
                assert_eq!(g.name.as_ref().unwrap(), "Variable #1", 
                    "Should navigate to the 'Variable #1' group");
            }
            _ => panic!("Expected to navigate to a Group element"),
        }
        
        // Test 4: Navigate to "Variable #2" group
        let path = vec![
            "seg:0".to_string(),
            "elem:0#5".to_string(),   // Different instance (#5)
            "elem:3".to_string(),      // Variable #2 is at index 3
        ];
        let result = navigate_to_path(&cdi, &path);
        assert!(result.is_ok(), "Should find 'Variable #2' group at index 3");
        
        match result.unwrap() {
            NavigationResult::Element(DataElement::Group(g)) => {
                assert_eq!(g.name.as_ref().unwrap(), "Variable #2", 
                    "Should navigate to the 'Variable #2' group");
            }
            _ => panic!("Expected to navigate to a Group element"),
        }
    }
}
