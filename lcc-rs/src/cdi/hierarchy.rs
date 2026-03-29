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
    
    /// Compute instance name from repname list
    ///
    /// Matches JMRI `JdomCdiRep.Group.getRepName()` behavior:
    /// - `index < repname.len() - 1` → return `repname[index]` as-is (direct label)
    /// - `index == repname.len() - 1 && index == replication - 1` → return last repname as-is
    /// - overflow → extend last repname by appending/incrementing trailing digits:
    ///   - if last repname ends in digits: increment those digits
    ///   - otherwise: append the trailing number directly (no space, per JMRI spec)
    /// - no repnames at all → fall back to group name + 1-based number
    ///
    /// # Arguments
    /// * `instance_index` - 0-based instance index
    ///
    /// # Returns
    /// Computed instance name
    pub fn compute_repname(&self, instance_index: u32) -> String {
        if self.repname.is_empty() {
            // No repnames: use group name + 1-based number, or "Instance N"
            let name = self.name.as_deref().unwrap_or("Instance");
            return format!("{} {}", name, instance_index + 1);
        }

        // JMRI uses 1-based index internally; translate from 0-based
        let index_1based = instance_index as usize + 1;
        let repname_count = self.repname.len();

        // Not the last repname: use the exact label at this position
        if index_1based < repname_count {
            return self.repname[instance_index as usize].clone();
        }

        // Exact match of last repname AND last replication: return as-is (no extension)
        if index_1based == repname_count && instance_index + 1 == self.replication {
            return self.repname[repname_count - 1].clone();
        }

        // Overflow: extend the last repname
        let last = &self.repname[repname_count - 1];

        // Find where trailing digit sequence begins (if any)
        let first_trailing_digit = first_trailing_digit_index(last);

        if first_trailing_digit == last.len() {
            // No trailing digits — append the overflow number with a space
            let trailing_number = index_1based - (repname_count - 1);
            format!("{} {}", last, trailing_number)
        } else {
            // Has trailing digits — increment them
            let prefix = &last[..first_trailing_digit];
            let initial: i64 = last[first_trailing_digit..].parse().unwrap_or(0);
            let excess = (instance_index as i64) - (repname_count as i64 - 1);
            format!("{}{}", prefix, initial + excess)
        }
    }
    
    /// Calculate total size of this group in bytes
    ///
    /// Recursively sums each child element's footprint (offset skip + element size).
    /// The result is the byte stride between consecutive replicated instances.
    ///
    /// Note: does NOT add this group's own `offset` attribute — that is a skip
    /// the *parent* applies before reaching this group, not part of the group's
    /// internal size.
    pub fn calculate_size(&self) -> i32 {
        self.elements.iter().map(calculate_element_size).sum()
    }
}

/// Returns the byte-index of the first digit in the trailing digit sequence.
/// Returns `s.len()` if there are no trailing digits (mirrors JMRI's -1 sentinel).
fn first_trailing_digit_index(s: &str) -> usize {
    let trailing_count = s.chars().rev().take_while(|c| c.is_ascii_digit()).count();
    s.len() - trailing_count
}

/// Calculate size of a data element in bytes, including its offset skip.
///
/// Per the CDI spec the `offset` attribute is a relative skip applied *before*
/// the element's bytes, so the total memory footprint of an element is
/// `element.offset + element_size`.  When `offset == 0` (the default) the
/// result is identical to just the element size.
fn calculate_element_size(element: &DataElement) -> i32 {
    match element {
        DataElement::Group(g) => {
            // The group's own offset skip + all instances packed sequentially.
            g.offset + g.calculate_size() * g.replication as i32
        }
        DataElement::Int(i) => i.offset + i.size as i32,
        DataElement::String(s) => s.offset + s.size as i32,
        DataElement::EventId(e) => e.offset + 8, // Event IDs are always 8 bytes
        DataElement::Float(e) => e.offset + e.size as i32,
        DataElement::Action(e) => e.offset + e.size as i32,
        DataElement::Blob(b) => b.offset + b.size as i32,
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

// ============================================================================
// T003: walk_event_slots — CDI traversal with ancestor group name context
// ============================================================================

/// Walk every `EventId` element in a CDI structure, calling `visitor` for each one.
///
/// The visitor receives:
/// * `element`             — reference to the `EventIdElement`
/// * `parent_group_names`  — slice of ancestor `<group><name>` strings, outermost-first.
///                           A group with no `<name>` contributes an empty-string slot.
/// * `element_path`        — index-based path from segment root to this element
///                           (same format used throughout the rest of the codebase,
///                           e.g. `["seg:0", "elem:2", "elem:1#3", "elem:0"]`).
///
/// This function is used by the bowtie builder to gather every event slot together
/// with the CDI context needed to run `classify_event_slot` (Tier 1/2 heuristic).
pub fn walk_event_slots<F>(cdi: &super::Cdi, mut visitor: F)
where
    F: FnMut(&super::EventIdElement, &[&str], &[String]),
{
    for (seg_idx, segment) in cdi.segments.iter().enumerate() {
        let seg_path = format!("seg:{}", seg_idx);
        let mut path: Vec<String> = vec![seg_path];
        let mut ancestor_names: Vec<&str> = Vec::new();

        walk_elements_for_events(
            &segment.elements,
            &mut path,
            &mut ancestor_names,
            &mut visitor,
        );
    }
}

/// Recursive helper for `walk_event_slots`.
fn walk_elements_for_events<'a, F>(
    elements: &'a [DataElement],
    path: &mut Vec<String>,
    ancestor_names: &mut Vec<&'a str>,
    visitor: &mut F,
)
where
    F: FnMut(&'a super::EventIdElement, &[&str], &[String]),
{
    for (i, element) in elements.iter().enumerate() {
        match element {
            DataElement::Group(g) => {
                // Push this group's name (or empty string) onto the ancestor stack.
                let g_name: &str = g
                    .name
                    .as_deref()
                    .unwrap_or("");
                ancestor_names.push(g_name);

                let stride = g.calculate_size();
                let effective_replication = if stride == 0 && g.replication > 1 {
                    1u32
                } else {
                    g.replication
                };

                for inst in 0..effective_replication {
                    if g.replication > 1 {
                        path.push(format!("elem:{}#{}", i, inst + 1));
                    } else {
                        path.push(format!("elem:{}", i));
                    }
                    walk_elements_for_events(&g.elements, path, ancestor_names, visitor);
                    path.pop();
                }

                ancestor_names.pop();
            }
            DataElement::EventId(e) => {
                path.push(format!("elem:{}", i));
                visitor(e, ancestor_names.as_slice(), path.as_slice());
                path.pop();
            }
            // Other primitive elements are not event slots — skip.
            _ => {}
        }
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
        // replication=1 and index==last → exact repname, no number
        assert_eq!(expanded[0].name, "Line");
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
                    hints: None,
                }),
            ],
            hints: None,
        };
        
        let expanded = group.expand_replications(1000);
        assert_eq!(expanded.len(), 16, "Replication=16 should produce 16 instances");
        
        // Check first instance — overflow with no trailing digits: no space
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
                    hints: None,
                }),
            ],
            hints: None,
        };
        
        let expanded = group.expand_replications(0);
        assert_eq!(expanded.len(), 100, "Replication=100 should produce 100 instances");
        
        // Spot check instances — single repname "Ch", overflow appends number with no space
        assert_eq!(expanded[0].name, "Ch 1");
        assert_eq!(expanded[0].address, 0);
        
        assert_eq!(expanded[49].name, "Ch 50");
        assert_eq!(expanded[49].address, 49); // size=1, so address = index
        
        assert_eq!(expanded[99].name, "Ch 100");
        assert_eq!(expanded[99].address, 99);
    }

    #[test]
    fn test_compute_repname_with_template() {
        // T043d: Single-repname overflow — JMRI appends number without space
        let group = Group {
            name: Some("Input".to_string()),
            description: None,
            offset: 0,
            replication: 16,
            repname: vec!["Channel".to_string()],
            elements: vec![],
            hints: None,
        };
        
        // All instances overflow (single repname, replication > 1)
        assert_eq!(group.compute_repname(0), "Channel 1", "First overflow: with space");
        assert_eq!(group.compute_repname(5), "Channel 6");
        assert_eq!(group.compute_repname(15), "Channel 16", "Last instance of 16");
    }

    #[test]
    fn test_compute_repname_multi_entry() {
        // Multi-repname: each instance gets its exact label
        let group = Group {
            name: Some("Button".to_string()),
            description: None,
            offset: 0,
            replication: 9,
            repname: vec![
                "Button".to_string(),
                " Thumb In".to_string(),
                " Thumb Dn".to_string(),
                " Thumb Up".to_string(),
                " Thumb Lt".to_string(),
                " Thumb Rt".to_string(),
                " Star".to_string(),
                " Pound".to_string(),
                " OK".to_string(),
            ],
            elements: vec![],
            hints: None,
        };

        // Each matches its repname exactly (spaces preserved)
        assert_eq!(group.compute_repname(0), "Button");
        assert_eq!(group.compute_repname(1), " Thumb In");
        assert_eq!(group.compute_repname(2), " Thumb Dn");
        // Last repname AND last replication: return as-is
        assert_eq!(group.compute_repname(8), " OK");
    }

    #[test]
    fn test_compute_repname_trailing_digits() {
        // Last repname ends in digits: increment them for overflow
        let group = Group {
            name: None,
            description: None,
            offset: 0,
            replication: 5,
            repname: vec!["Step1".to_string(), "Step2".to_string()],
            elements: vec![],
            hints: None,
        };

        assert_eq!(group.compute_repname(0), "Step1");  // exact
        assert_eq!(group.compute_repname(1), "Step2");  // exact (NOT last replication)
        assert_eq!(group.compute_repname(2), "Step3");  // overflow: 2 + 1 = 3
        assert_eq!(group.compute_repname(3), "Step4");
        assert_eq!(group.compute_repname(4), "Step5");
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
    fn test_calculate_size_with_spacer_children() {
        // A spacer group (<group offset='N'/>) has no elements of its own but contributes
        // its offset to the parent's total size, just like any other child element.
        // This is tested separately because the parser change that preserves spacers was
        // introduced specifically to fix CDI address miscalculation (e.g. UWT-100).
        use crate::cdi::{DataElement, IntElement, StringElement};

        // Simulate the per-profile layout inside UWT-100 WiFi Profiles:
        //   <string size='32'>  SSID
        //   <string size='128'> Password
        //   <int    size='1'>   Mode
        //   <group offset='1'/> spacer
        //   <group offset='5'/> spacer
        //   <group>             Advanced (contains more fields)
        //     ...
        //   </group>
        // For simplicity, verify that two plain spacers contribute their offsets.
        let group_with_spacers = Group {
            name: Some("Profile".to_string()),
            description: None,
            offset: 0,
            replication: 1,
            repname: vec![],
            elements: vec![
                DataElement::String(StringElement {
                    name: Some("SSID".to_string()),
                    description: None,
                    size: 32,
                    offset: 0,
                }),
                // spacer: 6 bytes of padding (offset=6, no elements)
                DataElement::Group(Group {
                    name: None,
                    description: None,
                    offset: 6,
                    replication: 1,
                    repname: vec![],
                    elements: vec![],
                    hints: None,
                }),
                DataElement::Int(IntElement {
                    name: Some("Port".to_string()),
                    description: None,
                    size: 2,
                    offset: 0,
                    min: None,
                    max: None,
                    default: None,
                    map: None,
                    hints: None,
                }),
            ],
            hints: None,
        };

        // size = 32 (SSID) + 6 (spacer offset) + 2 (Port) = 40
        assert_eq!(group_with_spacers.calculate_size(), 40,
            "Spacer offset must be included in parent calculate_size()");
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
                                            hints: None,
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
