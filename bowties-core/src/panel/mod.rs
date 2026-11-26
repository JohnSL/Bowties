//! Panel facility domain logic (Spec 021).
//!
//! Owns the slot -> channel -> board-row step that makes a panel's lamps
//! occupy Direct Lamp Control rows. Pin ownership claims
//! (`signaling::slots`) and output attribution (`signaling::outputs`) both
//! read it, so there is one producer of "which rows does this panel hold".

use crate::layout::channels::{ChannelBinding, ChannelsDocument};
use crate::layout::facilities::Facility;

/// Template slot label that holds a panel's lamp-indicator channels.
pub const LAMPS_SLOT: &str = "lamps";

/// The `(node_key, row_ordinal)` of every Direct Lamp Control row a panel
/// facility's lamp channels are bound to. Non-panel facilities, unknown
/// channel ids and channels with other bindings contribute nothing.
pub fn panel_lamp_rows(facility: &Facility, channels: &ChannelsDocument) -> Vec<(String, u32)> {
    if facility.template_id != "panel" {
        return Vec::new();
    }
    facility
        .slot_bindings
        .get(LAMPS_SLOT)
        .into_iter()
        .flatten()
        .filter_map(|id| channels.channels.iter().find(|c| &c.id == id))
        .filter_map(|channel| match &channel.binding {
            ChannelBinding::LampRow { node_key, row_ordinal } => Some((node_key.clone(), *row_ordinal)),
            _ => None,
        })
        .collect()
}

/// The saved row each panel entry's lamp channel is bound to, for the panel
/// facility `facility`. Entries without a lamp channel, or whose channel is
/// missing or not bound to a lamp row, contribute nothing.
pub fn panel_saved_rows(facility: &Facility, channels: &ChannelsDocument) -> Vec<compiler::PanelRowAssignment> {
    let Some(record) = facility.panel.as_ref().filter(|_| facility.template_id == "panel") else {
        return Vec::new();
    };
    record
        .entries
        .iter()
        .filter_map(|entry| {
            let id = entry.lamp_channel_id.as_ref()?;
            match &channels.channels.iter().find(|c| &c.id == id)?.binding {
                ChannelBinding::LampRow { node_key, row_ordinal } => Some(compiler::PanelRowAssignment {
                    entry_id: entry.entry_id.clone(),
                    node_key: node_key.clone(),
                    row: *row_ordinal,
                }),
                _ => None,
            }
        })
        .collect()
}

pub mod compiler;
