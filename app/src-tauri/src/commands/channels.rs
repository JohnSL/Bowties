//! Tauri commands for information channel operations.

use bowties_core::layout::channels::{ChannelsDocument, InformationChannel};
use crate::state::AppState;
use std::path::Path;

/// List all information channels for the currently active layout.
///
/// Reads `channels.yaml` from the active layout directory via the
/// intent-shaped `read_channels` API (ADR-0005). Returns an empty vec
/// when no layout is open or the file is missing (pre-015 layouts).
#[tauri::command]
pub async fn list_channels(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<InformationChannel>, String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Ok(Vec::new());
    };
    if context.root_path.is_empty() {
        return Ok(Vec::new());
    }
    let layout_dir = Path::new(&context.root_path);
    let doc = bowties_core::layout::read_channels(layout_dir)?;
    Ok(doc.channels)
}

/// Append new channels to the active layout's channel inventory.
///
/// Reads the current `channels.yaml`, appends the provided channels,
/// and writes the result through the journaled write path (ADR-0006).
/// Returns the complete updated channel list.
#[tauri::command]
pub async fn create_channels(
    state: tauri::State<'_, AppState>,
    channels: Vec<InformationChannel>,
) -> Result<Vec<InformationChannel>, String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Err("No layout is open".to_string());
    };
    if context.root_path.is_empty() {
        return Err("No layout is open".to_string());
    }
    let layout_dir = Path::new(&context.root_path);

    let mut doc = bowties_core::layout::read_channels(layout_dir)?;
    doc.channels.extend(channels);
    if doc.schema_version.is_empty() {
        doc.schema_version = ChannelsDocument::SCHEMA_VERSION.to_string();
    }
    bowties_core::layout::update_channels(layout_dir, &doc)?;
    Ok(doc.channels)
}

/// Rename a single channel by ID.
///
/// Reads `channels.yaml`, finds the channel with the given ID, updates
/// its name, and writes back via journal. Returns an error if the
/// channel is not found or the layout is not open.
#[tauri::command]
pub async fn rename_channel(
    state: tauri::State<'_, AppState>,
    id: String,
    new_name: String,
) -> Result<(), String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Err("No layout is open".to_string());
    };
    if context.root_path.is_empty() {
        return Err("No layout is open".to_string());
    }
    let layout_dir = Path::new(&context.root_path);

    let mut doc = bowties_core::layout::read_channels(layout_dir)?;
    let channel = doc
        .channels
        .iter_mut()
        .find(|ch| ch.id == id)
        .ok_or_else(|| format!("Channel not found: {}", id))?;
    channel.name = new_name;
    bowties_core::layout::update_channels(layout_dir, &doc)?;
    Ok(())
}

/// Delete channels by their IDs from the active layout's channel inventory.
///
/// Reads `channels.yaml`, removes channels whose IDs match the provided list,
/// and writes the result through the journaled write path (ADR-0006).
#[tauri::command]
pub async fn delete_channels(
    state: tauri::State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), String> {
    let active = state.active_layout.read().await;
    let Some(context) = active.as_ref() else {
        return Err("No layout is open".to_string());
    };
    if context.root_path.is_empty() {
        return Err("No layout is open".to_string());
    }
    let layout_dir = Path::new(&context.root_path);

    let mut doc = bowties_core::layout::read_channels(layout_dir)?;
    doc.channels.retain(|ch| !ids.contains(&ch.id));
    bowties_core::layout::update_channels(layout_dir, &doc)?;
    Ok(())
}
