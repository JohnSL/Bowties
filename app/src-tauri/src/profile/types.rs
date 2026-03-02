//! Profile type definitions
//!
//! Defines all Rust structs that are deserialised from a `.profile.yaml` file
//! and the `RelevanceAnnotation` that is attached to `GroupNode`s after annotation.

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// StructureProfile — root deserialization target
// ─────────────────────────────────────────────────────────────────────────────

/// Root of a `.profile.yaml` file.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureProfile {
    /// Schema version string. Currently must be "1.0".
    pub schema_version: String,

    /// Node type identification (manufacturer + model).
    pub node_type: ProfileNodeType,

    /// Optional firmware version range. Advisory only — does not gate profile application.
    #[serde(default)]
    pub firmware_version_range: Option<FirmwareVersionRange>,

    /// Event role declarations for CDI groups containing eventid leaves.
    #[serde(default)]
    pub event_roles: Vec<EventRoleDecl>,

    /// Conditional relevance rules.
    #[serde(default)]
    pub relevance_rules: Vec<RelevanceRule>,
}

/// Manufacturer + model identification block within a profile.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileNodeType {
    pub manufacturer: String,
    pub model: String,
}

/// Optional firmware version range (advisory only).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FirmwareVersionRange {
    pub min: Option<String>,
    pub max: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Event role declarations
// ─────────────────────────────────────────────────────────────────────────────

/// Declares the event role for all eventid leaves within a named CDI group.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRoleDecl {
    /// Name-based CDI path using '/' separators and '#N' ordinal suffix for
    /// same-named siblings (1-based).  E.g., `"Port I/O/Line/Event#1"`.
    pub group_path: String,

    /// Declared role for all eventid leaves in this group.
    pub role: ProfileEventRole,

    /// Optional display-name override for the matched group(s).
    ///
    /// When set, every `GroupNode` whose path resolves to this declaration gets
    /// its `display_name` set to this string instead of the CDI `<name>` text.
    /// Useful when the firmware's group name is ambiguous (e.g. two groups both
    /// named "Event" where one is Consumer and one is Producer).
    #[serde(default)]
    pub label: Option<String>,
}

/// Profile-declared event role (serialised as "Producer" / "Consumer").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ProfileEventRole {
    Producer,
    Consumer,
}

impl From<ProfileEventRole> for lcc_rs::cdi::EventRole {
    fn from(r: ProfileEventRole) -> Self {
        match r {
            ProfileEventRole::Producer => lcc_rs::cdi::EventRole::Producer,
            ProfileEventRole::Consumer => lcc_rs::cdi::EventRole::Consumer,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Relevance rules
// ─────────────────────────────────────────────────────────────────────────────

/// Conditional relevance rule.
///
/// When the `all_of` conditions are satisfied (V1: only single-condition rules
/// are evaluated; multi-condition rules are skipped with a log warning), the
/// `affected_group_path` section is considered irrelevant.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceRule {
    /// Unique identifier within this profile (e.g., `"R001"`).
    pub id: String,

    /// CDI group path of the section rendered irrelevant when the condition fires.
    pub affected_group_path: String,

    /// Conditions that must ALL be true (V1: only single-entry lists are evaluated).
    pub all_of: Vec<RelevanceCondition>,

    /// User-facing explanation text shown verbatim in the UI banner.
    pub explanation: String,
}

/// One condition within a relevance rule's `allOf` list.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceCondition {
    /// CDI name of the controlling field, sibling within the same replicated
    /// group instance as the affected group.  E.g., `"Output Function"`.
    pub field: String,

    /// Integer enum values of the controlling field that render the section irrelevant.
    pub irrelevant_when: Vec<i64>,
}

// ─────────────────────────────────────────────────────────────────────────────
// RelevanceAnnotation — tree extension (attached to GroupNode)
// ─────────────────────────────────────────────────────────────────────────────

/// Relevance rule annotation attached to a `GroupNode`.
///
/// Present only when a profile declares a relevance rule for this group.
/// Carries all information the frontend needs to evaluate and display relevance
/// state reactively without additional tree traversal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceAnnotation {
    /// Unique rule identifier from the profile (e.g., `"R001"`).
    pub rule_id: String,

    /// Index-based path of the controlling leaf within the same tree.
    pub controlling_field_path: Vec<String>,

    /// Memory address of the controlling field leaf.
    /// Combined with `controlling_field_space`, forms the `pendingEditsStore` key.
    pub controlling_field_address: u32,

    /// Memory space of the controlling field.
    pub controlling_field_space: u8,

    /// Integer enum values of the controlling field that make this section irrelevant.
    pub irrelevant_when: Vec<i64>,

    /// User-facing explanation rendered verbatim in the UI banner.
    pub explanation: String,
}
