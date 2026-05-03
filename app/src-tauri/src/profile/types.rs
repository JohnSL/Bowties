//! Profile type definitions
//!
//! Defines all Rust structs that are deserialised from a `.profile.yaml` file
//! and the `RelevanceAnnotation` that is attached to `GroupNode`s after annotation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type DaughterboardId = String;

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

    /// Connector-slot declarations for modular carrier boards.
    #[serde(default)]
    pub connector_slots: Vec<ConnectorSlotDefinition>,

    /// Optional connector CDI variants used to select daughterboard
    /// constraint variants for the connected node.
    #[serde(default)]
    pub connector_constraint_variants: Vec<ConnectorConstraintVariant>,

    /// Reusable daughterboard definitions referenced by this carrier profile.
    #[serde(default)]
    pub daughterboard_references: Vec<DaughterboardId>,

    /// Optional carrier-specific daughterboard refinements.
    #[serde(default)]
    pub carrier_overrides: Vec<CarrierOverrideRule>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCdiSignature {
    #[serde(default)]
    pub required_paths: Vec<String>,
    #[serde(default)]
    pub enum_entry_counts: Vec<ConnectorCdiEnumCount>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConstraintVariant {
    pub variant_id: String,
    pub cdi_signature: ConnectorCdiSignature,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorCdiEnumCount {
    pub path: String,
    pub count: usize,
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

// ─────────────────────────────────────────────────────────────────────────────
// Connector daughterboard metadata
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorSlotDefinition {
    pub slot_id: String,
    pub label: String,
    pub order: u32,
    pub allow_none_installed: bool,
    pub supported_daughterboard_ids: Vec<DaughterboardId>,
    pub affected_paths: Vec<String>,
    #[serde(default)]
    pub base_behavior_when_empty: Option<EmptyConnectorBehavior>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmptyConnectorBehavior {
    pub effect: EmptyConnectorEffect,
    #[serde(default)]
    pub allowed_values: Vec<ProfileScalarValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EmptyConnectorEffect {
    HideDependent,
    DisableDependent,
    AllowSubset,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CarrierOverrideRule {
    pub carrier_key: String,
    #[serde(default)]
    pub slot_id: Option<String>,
    pub daughterboard_id: DaughterboardId,
    #[serde(default)]
    pub replace_shared_validity_rules: bool,
    #[serde(default)]
    pub override_validity_rules: Vec<ConnectorConstraintRule>,
    #[serde(default)]
    pub override_repair_rules: Vec<RepairRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorConstraintRule {
    pub target_path: String,
    pub constraint_type: ConnectorConstraintType,
    #[serde(default)]
    pub line_ordinals: Vec<u32>,
    #[serde(default)]
    pub allowed_values: Vec<ProfileScalarValue>,
    #[serde(default)]
    pub denied_values: Vec<ProfileScalarValue>,
    #[serde(default)]
    pub explanation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectorConstraintType {
    AllowValues,
    DenyValues,
    ShowSection,
    HideSection,
    ReadOnly,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairRule {
    pub target_path: String,
    pub replacement_strategy: RepairStrategy,
    #[serde(default)]
    pub replacement_value: Option<serde_json::Value>,
    #[serde(default)]
    pub priority: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RepairStrategy {
    SetExplicit,
    ResetDefault,
    ClearEmpty,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ProfileScalarValue {
    String(String),
    Integer(i64),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedDaughterboardLibrary {
    pub schema_version: String,
    pub manufacturer: String,
    #[serde(default)]
    pub daughterboards: Vec<DaughterboardDefinition>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaughterboardDefinition {
    pub daughterboard_id: DaughterboardId,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default)]
    pub validity_rules: Vec<ConnectorConstraintRule>,
    #[serde(default)]
    pub repair_rules: Vec<RepairRule>,
    #[serde(default)]
    pub defaults_when_selected: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    pub constraint_variants: Vec<DaughterboardConstraintVariant>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<DaughterboardMetadata>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaughterboardConstraintVariant {
    pub variant_id: String,
    #[serde(default)]
    pub replace_base_validity_rules: bool,
    #[serde(default)]
    pub validity_rules: Vec<ConnectorConstraintRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaughterboardMetadata {
    #[serde(default)]
    pub manual_citations: Vec<String>,
    #[serde(default)]
    pub manufacturer_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connector_profile_types_roundtrip() {
        let profile = StructureProfile {
            schema_version: "1.0".to_string(),
            node_type: ProfileNodeType {
                manufacturer: "RR-CirKits".to_string(),
                model: "Tower-LCC".to_string(),
            },
            firmware_version_range: Some(FirmwareVersionRange {
                min: Some("1.0.0".to_string()),
                max: None,
            }),
            event_roles: vec![],
            relevance_rules: vec![],
            connector_slots: vec![ConnectorSlotDefinition {
                slot_id: "serial-a".to_string(),
                label: "Serial A".to_string(),
                order: 0,
                allow_none_installed: true,
                supported_daughterboard_ids: vec!["db-8in".to_string()],
                affected_paths: vec!["Port I/O/Line/Event#1".to_string()],
                base_behavior_when_empty: Some(EmptyConnectorBehavior {
                    effect: EmptyConnectorEffect::HideDependent,
                    allowed_values: vec![],
                }),
            }],
            connector_constraint_variants: vec![ConnectorConstraintVariant {
                variant_id: "tower-lcc-legacy".to_string(),
                cdi_signature: ConnectorCdiSignature {
                    required_paths: vec!["Port I/O/Line/Output Function".to_string()],
                    enum_entry_counts: vec![ConnectorCdiEnumCount {
                        path: "Port I/O/Line/Output Function".to_string(),
                        count: 17,
                    }],
                },
            }],
            daughterboard_references: vec!["db-8in".to_string()],
            carrier_overrides: vec![CarrierOverrideRule {
                carrier_key: "rr-cirkits::tower-lcc".to_string(),
                slot_id: Some("serial-a".to_string()),
                daughterboard_id: "db-8in".to_string(),
                replace_shared_validity_rules: true,
                override_validity_rules: vec![ConnectorConstraintRule {
                    target_path: "Port I/O/Line/Event#1".to_string(),
                    constraint_type: ConnectorConstraintType::HideSection,
                    line_ordinals: vec![1, 2, 3, 4],
                    allowed_values: vec![],
                    denied_values: vec![],
                    explanation: Some("Hidden when card selected".to_string()),
                }],
                override_repair_rules: vec![RepairRule {
                    target_path: "Port I/O/Line/Event#2".to_string(),
                    replacement_strategy: RepairStrategy::SetExplicit,
                    replacement_value: Some(serde_json::Value::String("occupancy".to_string())),
                    priority: Some(1),
                }],
            }],
        };

        let yaml = serde_yaml_ng::to_string(&profile).expect("profile should serialize");
        let parsed: StructureProfile =
            serde_yaml_ng::from_str(&yaml).expect("profile should deserialize");

        assert_eq!(parsed.connector_slots.len(), 1);
        assert_eq!(
            parsed
                .connector_constraint_variants[0]
                .cdi_signature
                .enum_entry_counts[0]
                .count,
            17
        );
        assert_eq!(parsed.daughterboard_references, vec!["db-8in"]);
        assert_eq!(parsed.carrier_overrides.len(), 1);
        assert!(parsed.carrier_overrides[0].replace_shared_validity_rules);
        assert_eq!(parsed.carrier_overrides[0].override_repair_rules.len(), 1);
        assert_eq!(parsed.carrier_overrides[0].override_validity_rules[0].line_ordinals, vec![1, 2, 3, 4]);
        assert_eq!(parsed.connector_slots[0].base_behavior_when_empty.as_ref().map(|behavior| behavior.effect), Some(EmptyConnectorEffect::HideDependent));
    }
}
