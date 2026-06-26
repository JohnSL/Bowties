//! Profile type definitions
//!
//! Defines all Rust structs that are deserialised from a `.profile.yaml` file
//! and the `RelevanceAnnotation` that is attached to `GroupNode`s after annotation.

use serde::{Deserialize, Serialize};

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

    /// First-class Configuration Modes (v2 schema, FR-001). Each mode owns a
    /// selector + named variants whose overlays are composed in declaration
    /// order, last-write-wins per target (FR-006).
    #[serde(default)]
    pub configuration_modes: Vec<ConfigurationMode>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Modes (v2)
// ─────────────────────────────────────────────────────────────────────────────

/// A first-class Configuration Mode: a named selector that switches between
/// declared variants, each contributing an overlay of event roles, relevance
/// rules, and structural constraints.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationMode {
    /// Unique mode id within the profile (e.g. `"turnoutboss-side"`).
    pub id: String,
    /// User-facing label shown in the variant picker.
    pub label: String,
    /// How the active variant is determined.
    pub selector: Selector,
    /// Declared variants. At least one entry is required by the schema.
    pub variants: Vec<Variant>,
}

/// How a Configuration Mode picks its active variant.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Selector {
    /// Variant chosen by the value of a CDI enum/int field at `field_path`.
    EnumField {
        /// CDI path of the controlling field in `'/' + '#N'` notation.
        field_path: String,
    },
    /// Variant chosen by which board occupies a structural slot.
    StructuralSlot {
        /// Slot identifier (e.g. `"connector-a"`).
        slot_id: String,
        /// User-facing slot label (e.g. `"Connector A"`).
        ///
        /// Optional for backward compatibility with TurnoutBoss-style slots
        /// whose label is the owning `ConfigurationMode.label`.
        #[serde(default)]
        slot_label: Option<String>,
        /// Stable ordering hint for slot picker UI. Defaults to 0; the
        /// owning `ConfigurationMode`'s declaration order is the
        /// authoritative fallback when omitted.
        #[serde(default)]
        slot_order: u32,
        /// CDI paths whose shape this slot drives.
        affected_paths: Vec<String>,
        /// When true, the reserved variant id `"__none__"` is a valid selection
        /// representing an empty slot.
        #[serde(default)]
        allow_none_installed: bool,
        /// Optional fallback behaviour for affected paths when no board is
        /// installed (FR-022 in the connector-slots spec).
        #[serde(default)]
        base_behavior_when_empty: Option<EmptyConnectorBehavior>,
    },
    /// Variant auto-detected from the live CDI's shape (firmware-revision
    /// sensing). Each entry in `variants_signature` maps a `variant_id` to a
    /// `ConnectorCdiSignature` the live CDI must satisfy for that variant to
    /// be considered active. Exactly one signature must match; zero or
    /// multiple matches surface as a typed `AnnotationReport` warning.
    CdiSignature {
        variants_signature: Vec<CdiSignatureVariantMatch>,
    },
}

/// One entry in a `Selector::CdiSignature` match list — pairs a variant id
/// with the CDI signature that selects it.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CdiSignatureVariantMatch {
    pub variant_id: String,
    pub signature: ConnectorCdiSignature,
}

/// One declared variant within a Configuration Mode.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    /// Variant id. For `EnumField`: the integer enum value stringified (or the
    /// `<property>` value). For `StructuralSlot`: the daughterboard id. The
    /// literal `"__none__"` is reserved for `allowNoneInstalled` slots.
    pub id: String,
    /// User-facing label (e.g. `"Left"`, `"BOD4-CP"`).
    pub label: String,
    /// Payload applied when this variant is active.
    pub overlay: Overlay,
}

/// Per-variant overlay payload: event-role, relevance, and structural-constraint
/// contributions that apply only when the owning variant is active.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overlay {
    #[serde(default)]
    pub event_roles: Vec<EventRoleDecl>,
    #[serde(default)]
    pub relevance_rules: Vec<RelevanceRule>,
    /// Structural constraint rules (same shape as v1 `ConnectorConstraintRule`).
    /// Replaces v1 `connectorConstraintVariants` + `carrierOverrides`.
    #[serde(default)]
    pub structural_constraints: Vec<ConnectorConstraintRule>,
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
/// `affected_target` section is considered irrelevant.
///
/// V2: the field was renamed from `affectedGroupPath` to `affectedTarget` and
/// may now resolve to a group, a single replication instance, or a leaf.
/// `#[serde(alias = "affectedGroupPath")]` accepts the legacy v1 spelling
/// through the transition slices; the alias is dropped in S5.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelevanceRule {
    /// Unique identifier within this profile (e.g., `"R001"`).
    pub id: String,

    /// CDI path of the target rendered irrelevant when the condition fires.
    /// May be a group, a single replication instance, or a leaf.
    #[serde(alias = "affectedGroupPath")]
    pub affected_target: String,

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
pub struct ConnectorConstraintRule {
    pub target_path: String,
    pub constraint_type: ConnectorConstraintType,
    #[serde(default)]
    pub line_ordinals: Vec<u32>,
    /// 1-based replication indices within the matched group. When empty,
    /// the rule applies to all replications.
    #[serde(default)]
    pub replication_ordinals: Vec<u32>,
    #[serde(default)]
    pub allowed_values: Vec<ProfileScalarValue>,
    #[serde(default)]
    pub allowed_value_labels: Vec<String>,
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
    fn v2_configuration_modes_roundtrip() {
        let profile = StructureProfile {
            schema_version: "2.0".to_string(),
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
            configuration_modes: vec![
                ConfigurationMode {
                    id: "firmware-revision".to_string(),
                    label: "Firmware revision".to_string(),
                    selector: Selector::CdiSignature {
                        variants_signature: vec![CdiSignatureVariantMatch {
                            variant_id: "tower-lcc-legacy".to_string(),
                            signature: ConnectorCdiSignature {
                                required_paths: vec!["Port I/O/Line/Output Function".to_string()],
                                enum_entry_counts: vec![ConnectorCdiEnumCount {
                                    path: "Port I/O/Line/Output Function".to_string(),
                                    count: 17,
                                }],
                            },
                        }],
                    },
                    variants: vec![Variant {
                        id: "tower-lcc-legacy".to_string(),
                        label: "Legacy firmware".to_string(),
                        overlay: Overlay::default(),
                    }],
                },
                ConfigurationMode {
                    id: "serial-a".to_string(),
                    label: "Serial A".to_string(),
                    selector: Selector::StructuralSlot {
                        slot_id: "serial-a".to_string(),
                        slot_label: Some("Serial A".to_string()),
                        slot_order: 0,
                        affected_paths: vec!["Port I/O/Line/Event#1".to_string()],
                        allow_none_installed: true,
                        base_behavior_when_empty: Some(EmptyConnectorBehavior {
                            effect: EmptyConnectorEffect::HideDependent,
                            allowed_values: vec![],
                        }),
                    },
                    variants: vec![Variant {
                        id: "db-8in".to_string(),
                        label: "8-Input Board".to_string(),
                        overlay: Overlay::default(),
                    }],
                },
            ],
        };

        let yaml = serde_yaml_ng::to_string(&profile).expect("profile should serialize");
        let parsed: StructureProfile =
            serde_yaml_ng::from_str(&yaml).expect("profile should deserialize");

        assert_eq!(parsed.configuration_modes.len(), 2);

        match &parsed.configuration_modes[0].selector {
            Selector::CdiSignature { variants_signature } => {
                assert_eq!(variants_signature.len(), 1);
                assert_eq!(
                    variants_signature[0].signature.enum_entry_counts[0].count,
                    17
                );
            }
            other => panic!("expected CdiSignature selector, got {:?}", other),
        }

        match &parsed.configuration_modes[1].selector {
            Selector::StructuralSlot {
                slot_id,
                slot_order,
                affected_paths,
                allow_none_installed,
                base_behavior_when_empty,
                ..
            } => {
                assert_eq!(slot_id, "serial-a");
                assert_eq!(*slot_order, 0);
                assert_eq!(affected_paths.len(), 1);
                assert!(*allow_none_installed);
                assert_eq!(
                    base_behavior_when_empty.as_ref().map(|behavior| behavior.effect),
                    Some(EmptyConnectorEffect::HideDependent)
                );
            }
            other => panic!("expected StructuralSlot selector, got {:?}", other),
        }

        assert_eq!(parsed.configuration_modes[1].variants[0].id, "db-8in");
    }
}
