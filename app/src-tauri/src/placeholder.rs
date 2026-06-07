//! Placeholder factory — owns creation of synthesized placeholder nodes.
//!
//! To "Add Placeholder" as bus discovery is to "Node Appeared": the factory
//! synthesizes a fully-valid in-memory state holder and inserts it into the
//! same registry that live nodes use.  No other module knows the conventions
//! (UUID key minting, bundled CDI resolution, all-zero EventId synthesis).
//!
//! Pure domain helpers (CDI loading, EventId-zero collection, config-value
//! merging, leaf-default population) live in `bowties_core::placeholder`.
//! This module owns the Tauri-specific orchestration: resolving resource dirs,
//! querying profiles, building trees with profile overlays, and assembling
//! the `SynthesizedNodeProxy`.
//!
//! Spec 014 / S8.10.

use lcc_rs::cdi::Cdi;
use lcc_rs::types::{CdiData, SNIPData};

use crate::node_proxy::SynthesizedNodeProxy;
use crate::node_tree::NodeConfigTree;
use crate::state::AppState;

// Re-export pure helpers so existing `crate::placeholder::*` call sites work.
pub use bowties_core::placeholder::{
    collect_eventid_zeros,
    load_bundled_cdi_with_xml,
    merge_config_values_into_tree,
    populate_leaf_defaults_in_tree,
};

/// Synthesize a placeholder node from a bundled profile stem.
///
/// Mints a `placeholder:<uuid>` key, loads the bundled CDI, walks the CDI
/// for EventId leaves (pre-populating `[0u8; 8]`), builds the config tree
/// with profile metadata applied, and returns the complete
/// `SynthesizedNodeProxy` ready for registry insertion.
pub async fn synthesize(
    profile_stem: &str,
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<(String, SynthesizedNodeProxy), String> {
    let node_key = format!("placeholder:{}", uuid::Uuid::new_v4());

    // ── Load bundled CDI (raw XML + parsed) ──────────────────────────────
    let dirs = crate::commands::cdi::bundled_cdi_search_dirs(app_handle);
    let (xml, cdi) = load_bundled_cdi_with_xml(&dirs, profile_stem)?;

    // ── Walk CDI for EventId leaves → all-zero bytes ─────────────────────
    let config_values = collect_eventid_zeros(&cdi);

    // ── Resolve manufacturer / model from profile listing ────────────────
    let profiles = crate::profile::loader::list_bundled_profiles(app_handle);
    let summary = profiles
        .iter()
        .find(|p| p.stem == profile_stem)
        .ok_or_else(|| {
            format!("UnknownProfile: '{profile_stem}' not in bundled profile listing")
        })?;

    let snip = SNIPData {
        manufacturer: summary.manufacturer.clone(),
        model: summary.model.clone(),
        hardware_version: String::new(),
        software_version: String::new(),
        user_name: String::new(),
        user_description: String::new(),
    };

    // ── Build config tree + profile overlay ───────────────────────────────
    let mut tree = build_tree_with_profile(&node_key, &cdi, app_handle, state).await;
    merge_config_values_into_tree(&mut tree, &config_values);
    populate_leaf_defaults_in_tree(&mut tree);

    let cdi_data = CdiData {
        xml_content: xml,
        retrieved_at: chrono::Utc::now(),
    };

    let proxy = SynthesizedNodeProxy {
        node_key: node_key.clone(),
        profile_stem: profile_stem.to_string(),
        snip: Some(snip),
        cdi_data: Some(cdi_data),
        cdi_parsed: Some(cdi),
        config_values,
        config_tree: Some(tree),
        producer_identified_events: Vec::new(),
    };

    Ok((node_key, proxy))
}

/// Reconstitute a `SynthesizedNodeProxy` from a saved placeholder's known
/// key and profile stem.
///
/// Same pipeline as `synthesize` but skips UUID minting — the caller already
/// knows the `node_key` (e.g. from a persisted `NodeSnapshot`).  Used by
/// `get_node_tree` to lazily populate the registry for saved placeholders
/// that weren't factory-minted this session.
pub async fn reconstitute(
    node_key: &str,
    profile_stem: &str,
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<SynthesizedNodeProxy, String> {
    let dirs = crate::commands::cdi::bundled_cdi_search_dirs(app_handle);
    let (xml, cdi) = load_bundled_cdi_with_xml(&dirs, profile_stem)?;
    let config_values = collect_eventid_zeros(&cdi);

    let profiles = crate::profile::loader::list_bundled_profiles(app_handle);
    let summary = profiles
        .iter()
        .find(|p| p.stem == profile_stem)
        .ok_or_else(|| {
            format!("UnknownProfile: '{profile_stem}' not in bundled profile listing")
        })?;

    let snip = SNIPData {
        manufacturer: summary.manufacturer.clone(),
        model: summary.model.clone(),
        hardware_version: String::new(),
        software_version: String::new(),
        user_name: String::new(),
        user_description: String::new(),
    };

    let mut tree = build_tree_with_profile(node_key, &cdi, app_handle, state).await;
    merge_config_values_into_tree(&mut tree, &config_values);
    populate_leaf_defaults_in_tree(&mut tree);

    let cdi_data = CdiData {
        xml_content: xml,
        retrieved_at: chrono::Utc::now(),
    };

    Ok(SynthesizedNodeProxy {
        node_key: node_key.to_string(),
        profile_stem: profile_stem.to_string(),
        snip: Some(snip),
        cdi_data: Some(cdi_data),
        cdi_parsed: Some(cdi),
        config_values,
        config_tree: Some(tree),
        producer_identified_events: Vec::new(),
    })
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Build the config tree from a parsed CDI and apply the structure profile
/// overlay (event roles, relevance, connector profile, mode selections).
async fn build_tree_with_profile(
    node_key: &str,
    cdi: &Cdi,
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> NodeConfigTree {
    let mut tree = crate::node_tree::build_node_config_tree(node_key, cdi);

    if let Some(identity) = &cdi.identification {
        let manufacturer = identity.manufacturer.as_deref().unwrap_or("");
        let model = identity.model.as_deref().unwrap_or("");
        if !manufacturer.is_empty() || !model.is_empty() {
            if let Some(profile) = crate::profile::load_profile(
                manufacturer,
                model,
                cdi,
                app_handle,
                &state.profiles,
            )
            .await
            {
                let shared_daughterboards =
                    crate::profile::load_shared_daughterboards(app_handle).await;
                let selections =
                    crate::commands::cdi::active_node_mode_selections(state, node_key)
                        .await;
                crate::commands::cdi::apply_profile_metadata_to_tree(
                    &mut tree,
                    node_key,
                    &profile,
                    shared_daughterboards.as_ref(),
                    cdi,
                    &selections,
                );
            }
        }
    }

    tree
}

// Tests for the pure helper functions (collect_eventid_zeros,
// merge_config_values_into_tree, populate_leaf_defaults_in_tree) now live in
// bowties_core::placeholder::tests.
