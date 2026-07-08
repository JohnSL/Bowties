//! Logic adapter — compiles behavior templates into CDI field writes.
//!
//! This module owns the compilation of `Compiled` behavior templates into
//! concrete CDI configuration values (conditional line settings on Tower LCC
//! nodes). S1 provides a stub compiler that returns structurally valid
//! `CompiledLogicPlan` values without real conditional-line expansion; S2
//! replaces the stub with real compilation logic.
//!
//! Function-level module seam (YAGNI: no trait/dynamic dispatch until a
//! second compilation target arrives — per D2:A).

use serde::{Deserialize, Serialize};

// ── Allocation types ──────────────────────────────────────────────────────

/// A contiguous range of conditional lines allocated on a target node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConditionalLineRange {
    /// 0-based start index of the first conditional line in this range.
    pub start: u32,
    /// Number of contiguous conditional lines in this range.
    pub count: u32,
}

/// A logic allocation record for one facility on one target node.
///
/// Persisted as part of the facility layer so that save + reopen
/// preserves the allocation. The compiler is the single authority
/// on allocation rules (D3:A — per-facility field, no new store).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicAllocation {
    /// Facility that owns this allocation.
    pub facility_id: String,
    /// Node key of the logic target (Tower LCC node).
    pub target_node_key: String,
    /// Conditional line range(s) allocated on the target node.
    pub conditional_lines: ConditionalLineRange,
}

// ── Compiled plan ─────────────────────────────────────────────────────────

/// One CDI field write produced by the compiler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompiledFieldWrite {
    /// CDI leaf path on the target node (e.g. "cdi/segment/group/int").
    pub leaf_path: String,
    /// Memory space of the leaf.
    pub space: u8,
    /// Byte address of the leaf within the memory space.
    pub address: u64,
    /// Value to write (as raw bytes, up to 8 bytes for numeric fields).
    pub value: Vec<u8>,
}

/// The output of the logic compiler for one facility.
///
/// Contains the allocation record and the CDI field writes that
/// configure the allocated conditional lines on the target node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompiledLogicPlan {
    /// The allocation record (persisted with the facility).
    pub allocation: LogicAllocation,
    /// CDI field writes to stage as drafts on the target node.
    pub field_writes: Vec<CompiledFieldWrite>,
}

// ── Capacity ──────────────────────────────────────────────────────────────

/// Capacity information for a logic target node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogicCapacity {
    /// Total conditional lines available on the node.
    pub total_lines: u32,
    /// Conditional lines currently allocated.
    pub used_lines: u32,
}

impl LogicCapacity {
    pub fn available(&self) -> u32 {
        self.total_lines.saturating_sub(self.used_lines)
    }
}

// ── Errors ────────────────────────────────────────────────────────────────

/// Errors the logic compiler may return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompileError {
    /// The target node does not have enough conditional lines to
    /// satisfy the template's requirements.
    InsufficientCapacity {
        required: u32,
        available: u32,
    },
    /// The referenced template is not a `Compiled` template.
    NotCompiled {
        template_id: String,
    },
    /// The referenced template was not found in the registry.
    UnknownTemplate {
        template_id: String,
    },
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientCapacity { required, available } => {
                write!(
                    f,
                    "insufficient capacity: need {} conditional lines but only {} available",
                    required, available
                )
            }
            Self::NotCompiled { template_id } => {
                write!(f, "template '{}' is not a compiled template", template_id)
            }
            Self::UnknownTemplate { template_id } => {
                write!(f, "unknown template '{}'", template_id)
            }
        }
    }
}

impl std::error::Error for CompileError {}

// ── Stub compiler (S1) ───────────────────────────────────────────────────

/// Maximum conditional lines per Tower LCC node.
pub const MAX_CONDITIONAL_LINES: u32 = 32;

/// Compile a facility's behavior template into a `CompiledLogicPlan`.
///
/// S1 stub: validates the template is `Compiled`, checks capacity, and
/// returns a structurally valid plan with a realistic allocation record
/// and placeholder field writes. The real compiler (S2) replaces the
/// field-write generation with actual conditional-line expansion.
pub fn compile_facility(
    template_id: &str,
    facility_id: &str,
    target_node_key: &str,
    existing_allocations: &[LogicAllocation],
) -> Result<CompiledLogicPlan, CompileError> {
    let template = crate::behavior_templates::find_template(template_id)
        .ok_or_else(|| CompileError::UnknownTemplate {
            template_id: template_id.to_string(),
        })?;

    if template.compilation_target != crate::behavior_templates::CompilationTarget::Compiled {
        return Err(CompileError::NotCompiled {
            template_id: template_id.to_string(),
        });
    }

    let required = template.rules.len() as u32;
    let used = used_lines_on_node(target_node_key, existing_allocations);
    let available = MAX_CONDITIONAL_LINES.saturating_sub(used);

    if required > available {
        return Err(CompileError::InsufficientCapacity { required, available });
    }

    // Allocate the next contiguous block of lines after existing usage.
    let start = used;
    let allocation = LogicAllocation {
        facility_id: facility_id.to_string(),
        target_node_key: target_node_key.to_string(),
        conditional_lines: ConditionalLineRange {
            start,
            count: required,
        },
    };

    // Stub: produce one placeholder field write per rule so downstream
    // consumers (draft staging, save/discard) see a realistic shape.
    let field_writes = template
        .rules
        .iter()
        .enumerate()
        .map(|(i, rule)| CompiledFieldWrite {
            leaf_path: format!(
                "cdi/segment/conditionalLines/line{}/label",
                start + i as u32
            ),
            space: 253,
            address: 1000 + (start + i as u32) as u64 * 64,
            value: rule.label.as_bytes().to_vec(),
        })
        .collect();

    Ok(CompiledLogicPlan {
        allocation,
        field_writes,
    })
}

/// Count how many conditional lines are already allocated on a given node.
fn used_lines_on_node(node_key: &str, allocations: &[LogicAllocation]) -> u32 {
    allocations
        .iter()
        .filter(|a| a.target_node_key == node_key)
        .map(|a| a.conditional_lines.count)
        .sum()
}

/// Query the capacity of a logic target node given existing allocations.
pub fn get_capacity(
    target_node_key: &str,
    existing_allocations: &[LogicAllocation],
) -> LogicCapacity {
    let used = used_lines_on_node(target_node_key, existing_allocations);
    LogicCapacity {
        total_lines: MAX_CONDITIONAL_LINES,
        used_lines: used,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_compiler_returns_valid_plan_for_abs_template() {
        let plan = compile_facility(
            "abs-3-aspect-signal",
            "facility-1",
            "050201020300",
            &[],
        )
        .unwrap();

        assert_eq!(plan.allocation.facility_id, "facility-1");
        assert_eq!(plan.allocation.target_node_key, "050201020300");
        assert_eq!(plan.allocation.conditional_lines.start, 0);
        assert_eq!(plan.allocation.conditional_lines.count, 3);
        assert_eq!(plan.field_writes.len(), 3);
    }

    #[test]
    fn stub_compiler_allocates_after_existing_usage() {
        let existing = vec![LogicAllocation {
            facility_id: "other".to_string(),
            target_node_key: "050201020300".to_string(),
            conditional_lines: ConditionalLineRange { start: 0, count: 5 },
        }];

        let plan = compile_facility(
            "abs-3-aspect-signal",
            "facility-2",
            "050201020300",
            &existing,
        )
        .unwrap();

        assert_eq!(plan.allocation.conditional_lines.start, 5);
        assert_eq!(plan.allocation.conditional_lines.count, 3);
    }

    #[test]
    fn stub_compiler_rejects_when_capacity_exceeded() {
        let existing = vec![LogicAllocation {
            facility_id: "other".to_string(),
            target_node_key: "050201020300".to_string(),
            conditional_lines: ConditionalLineRange { start: 0, count: 31 },
        }];

        let err = compile_facility(
            "abs-3-aspect-signal",
            "facility-3",
            "050201020300",
            &existing,
        )
        .unwrap_err();

        assert_eq!(
            err,
            CompileError::InsufficientCapacity {
                required: 3,
                available: 1
            }
        );
    }

    #[test]
    fn stub_compiler_rejects_composed_template() {
        let err = compile_facility(
            "block-indicator",
            "facility-1",
            "050201020300",
            &[],
        )
        .unwrap_err();

        assert_eq!(
            err,
            CompileError::NotCompiled {
                template_id: "block-indicator".to_string()
            }
        );
    }

    #[test]
    fn stub_compiler_rejects_unknown_template() {
        let err = compile_facility(
            "nonexistent",
            "facility-1",
            "050201020300",
            &[],
        )
        .unwrap_err();

        assert_eq!(
            err,
            CompileError::UnknownTemplate {
                template_id: "nonexistent".to_string()
            }
        );
    }

    #[test]
    fn capacity_query_reflects_existing_allocations() {
        let allocs = vec![
            LogicAllocation {
                facility_id: "a".to_string(),
                target_node_key: "NODE1".to_string(),
                conditional_lines: ConditionalLineRange { start: 0, count: 10 },
            },
            LogicAllocation {
                facility_id: "b".to_string(),
                target_node_key: "NODE1".to_string(),
                conditional_lines: ConditionalLineRange { start: 10, count: 5 },
            },
        ];

        let cap = get_capacity("NODE1", &allocs);
        assert_eq!(cap.total_lines, 32);
        assert_eq!(cap.used_lines, 15);
        assert_eq!(cap.available(), 17);
    }

    #[test]
    fn capacity_query_ignores_other_nodes() {
        let allocs = vec![LogicAllocation {
            facility_id: "a".to_string(),
            target_node_key: "OTHER".to_string(),
            conditional_lines: ConditionalLineRange { start: 0, count: 20 },
        }];

        let cap = get_capacity("NODE1", &allocs);
        assert_eq!(cap.used_lines, 0);
        assert_eq!(cap.available(), 32);
    }

    #[test]
    fn allocation_round_trips_json() {
        let alloc = LogicAllocation {
            facility_id: "f1".to_string(),
            target_node_key: "050201020300".to_string(),
            conditional_lines: ConditionalLineRange { start: 5, count: 3 },
        };
        let json = serde_json::to_value(&alloc).unwrap();
        assert_eq!(json["facilityId"], "f1");
        assert_eq!(json["targetNodeKey"], "050201020300");
        assert_eq!(json["conditionalLines"]["start"], 5);
        assert_eq!(json["conditionalLines"]["count"], 3);

        let parsed: LogicAllocation = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, alloc);
    }

    #[test]
    fn compiled_plan_round_trips_json() {
        let plan = CompiledLogicPlan {
            allocation: LogicAllocation {
                facility_id: "f1".to_string(),
                target_node_key: "NODE1".to_string(),
                conditional_lines: ConditionalLineRange { start: 0, count: 1 },
            },
            field_writes: vec![CompiledFieldWrite {
                leaf_path: "cdi/segment/line0/label".to_string(),
                space: 253,
                address: 1000,
                value: vec![83, 116, 111, 112], // "Stop"
            }],
        };
        let json = serde_json::to_string(&plan).unwrap();
        let parsed: CompiledLogicPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, plan);
    }
}
