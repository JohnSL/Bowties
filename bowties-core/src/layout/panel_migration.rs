//! Load-time conversion only: no compilation, event allocation or board edits.

use std::collections::{BTreeMap, HashSet};
use serde::{Deserialize, Serialize};
use super::channels::{ChannelBinding, ChannelRole, ChannelsDocument};
use super::facilities::{canonical_node_key, FacilitiesDocument, Facility, LitWhen, PanelEntry, PanelRecord};
use super::node_snapshot::NodeSnapshot;
use super::types::LayoutFile;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PanelMigration {
    pub indicators: usize,
    pub panels: usize,
    pub warnings: Vec<String>,
}

pub(crate) struct RejectedIndicator {
    facility_id: Option<String>,
    warning: String,
}

/// Only identifiable retired records are recoverable; invalid supported data
/// still fails the read instead of silently losing a current facility.
pub(crate) fn decode_facilities(
    mut value: serde_yaml_ng::Value,
) -> Result<(FacilitiesDocument, Vec<RejectedIndicator>), String> {
    let mut rejected = Vec::new();
    if let Some(records) = value.get("facilities").and_then(serde_yaml_ng::Value::as_sequence) {
        let mut accepted = Vec::new();
        for record in records {
            if let Err(error) = serde_yaml_ng::from_value::<Facility>(record.clone()) {
                if record.get("templateId").and_then(serde_yaml_ng::Value::as_str) != Some("block-indicator") {
                    return Err(error.to_string());
                }
                let facility_id = record.get("facilityId").and_then(serde_yaml_ng::Value::as_str).map(String::from);
                let name = record.get("name").and_then(serde_yaml_ng::Value::as_str)
                    .or(facility_id.as_deref()).unwrap_or("<unnamed>");
                rejected.push(RejectedIndicator {
                    warning: format!("Block Indicator '{name}': dropped malformed record: {error}"),
                    facility_id,
                });
            } else {
                accepted.push(record.clone());
            }
        }
        value["facilities"] = serde_yaml_ng::Value::Sequence(accepted);
    }
    serde_yaml_ng::from_value(value).map(|document| (document, rejected)).map_err(|error| error.to_string())
}

pub(crate) fn migrate_loaded(
    facilities: &mut FacilitiesDocument,
    channels: &mut ChannelsDocument,
    bowties: &mut LayoutFile,
    snapshots: &[NodeSnapshot],
    rejected: Vec<RejectedIndicator>,
) -> Option<PanelMigration> {
    if rejected.is_empty() && !facilities.facilities.iter().any(|f| f.template_id == "block-indicator") {
        return None;
    }
    let mut outcome = migrate(facilities, channels, bowties, snapshots);
    if !rejected.is_empty() {
        facilities.schema_version = FacilitiesDocument::SCHEMA_VERSION.into();
        let mut retired = HashSet::new();
        for record in rejected {
            if let Some(id) = record.facility_id {
                if !facilities.facilities.iter().any(|f| f.facility_id == id) {
                    retired.insert(id);
                }
            }
            outcome.warnings.push(record.warning);
        }
        clear_backlinks(bowties, &retired);
    }
    Some(outcome)
}

/// Convert a normalized aggregate. Reapplying to converted data is a no-op.
pub fn migrate(
    facilities: &mut FacilitiesDocument,
    channels: &mut ChannelsDocument,
    bowties: &mut LayoutFile,
    snapshots: &[NodeSnapshot],
) -> PanelMigration {
    let mut accepted_ids: HashSet<_> = facilities.facilities.iter()
        .filter(|f| f.template_id != "block-indicator").map(|f| f.facility_id.clone()).collect();
    let retired: HashSet<_> = facilities.facilities.iter()
        .filter(|f| f.template_id == "block-indicator" && !accepted_ids.contains(&f.facility_id)).map(|f| f.facility_id.clone()).collect();
    if !facilities.facilities.iter().any(|f| f.template_id == "block-indicator") { return PanelMigration::default(); }
    let mut outcome = PanelMigration::default();
    let mut groups: Vec<(Option<String>, Facility)> = Vec::new();
    let mut retained = Vec::new();
    for old in std::mem::take(&mut facilities.facilities) {
        if old.template_id != "block-indicator" { retained.push(old); continue; }
        let inputs = old.slot_bindings.get("input").map(Vec::as_slice).unwrap_or(&[]);
        let outputs = old.slot_bindings.get("output").map(Vec::as_slice).unwrap_or(&[]);
        let block = inputs.first().and_then(|id| channels.channels.iter().find(|c| c.id == *id));
        let lamp = outputs.first().and_then(|id| channels.channels.iter().find(|c| c.id == *id));
        let board = lamp.and_then(|c| match &c.binding {
            ChannelBinding::LampRow { node_key, row_ordinal } if *row_ordinal > 0 => crate::node_key::NodeKey::parse(node_key).ok().map(|key| key.to_string()),
            _ => None,
        });
        let malformed = old.facility_id.trim().is_empty() || accepted_ids.contains(&old.facility_id)
            || inputs.len() != 1 || outputs.len() > 1
            || block.is_none_or(|c| c.role != ChannelRole::BlockOccupancy)
            || (!outputs.is_empty() && (lamp.is_none_or(|c| c.role != ChannelRole::LampIndicator) || board.is_none()))
            || old.panel.is_some() || old.block_signals.is_some()
            || old.slot_bindings.iter().any(|(key, ids)| key != "input" && key != "output" && !ids.is_empty());
        if malformed {
            outcome.warnings.push(format!("Block Indicator '{}': dropped malformed record (requires a unique non-empty facility ID, one block and at most one Direct Lamp row).", old.name));
            continue;
        }
        let block = block.unwrap().id.clone();
        let lamp_id = lamp.map(|c| c.id.clone());
        let index = groups.iter().position(|(key, _)| key == &board).unwrap_or_else(|| {
            let name = board.as_ref().map(|key| {
                let label = snapshots.iter().find(|s| canonical_node_key(&s.node_key) == *key)
                    .map(|s| if !s.snip.user_name.trim().is_empty() { s.snip.user_name.as_str() } else { s.snip.model_name.as_str() })
                    .filter(|name| !name.trim().is_empty()).unwrap_or(key);
                format!("{label} panel")
            }).unwrap_or_else(|| "Indicators without a lamp".into());
            groups.push((board.clone(), Facility {
                facility_id: old.facility_id.clone(), template_id: "panel".into(), name,
                slot_bindings: BTreeMap::from([("lamps".into(), Vec::new())]),
                block_signals: None, panel: Some(PanelRecord { lit_when: LitWhen::Occupied, entries: Vec::new() }),
            }));
            groups.len() - 1
        });
        let panel = &mut groups[index].1;
        let entries = &mut panel.panel.as_mut().unwrap().entries;
        if entries.iter().any(|e| e.block_id == block || (lamp_id.is_some() && e.lamp_channel_id == lamp_id)) {
            outcome.warnings.push(format!("Block Indicator '{}': dropped duplicate block or lamp in '{}'; first accepted entry kept.", old.name, panel.name));
            continue;
        }
        if let Some(id) = &lamp_id {
            channels.channels.iter_mut().find(|c| c.id == *id).unwrap().name = old.name;
            panel.slot_bindings.get_mut("lamps").unwrap().push(id.clone());
        }
        accepted_ids.insert(old.facility_id.clone());
        entries.push(PanelEntry { entry_id: old.facility_id, block_id: block, lamp_channel_id: lamp_id, lit_when: None });
        outcome.indicators += 1;
    }
    outcome.panels = groups.len();
    retained.extend(groups.into_iter().map(|(_, panel)| panel));
    facilities.facilities = retained;
    facilities.schema_version = FacilitiesDocument::SCHEMA_VERSION.into();
    clear_backlinks(bowties, &retired);
    outcome
}

fn clear_backlinks(bowties: &mut LayoutFile, retired: &HashSet<String>) {
    for metadata in bowties.bowties.values_mut() {
        if metadata.created_by_facility.as_ref().is_some_and(|id| retired.contains(id)) {
            metadata.created_by_facility = None;
        }
    }
}
