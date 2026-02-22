//! Native application menu setup.
//!
//! Builds the OS-native menu bar and exposes handles to the items that need
//! dynamic enable/disable updates driven by the frontend via the
//! `update_menu_state` Tauri command.

use tauri::{AppHandle, Wry};
use tauri::menu::{MenuBuilder, SubmenuBuilder, MenuItem, PredefinedMenuItem};

/// Handles to menu items whose enabled state must change at runtime.
///
/// `MenuItem<Wry>` is internally Arc-backed and is `Clone + Send + Sync`,
/// so the struct can be stored as managed Tauri state.
pub struct MenuHandles {
    pub disconnect:      MenuItem<Wry>,
    pub refresh_nodes:   MenuItem<Wry>,
    pub traffic_monitor: MenuItem<Wry>,
    pub view_cdi:        MenuItem<Wry>,
    pub redownload_cdi:  MenuItem<Wry>,
}

/// Build the native application menu.
///
/// Returns the assembled `Menu` (to be set on the app with `app.set_menu()`)
/// and a `MenuHandles` value (to be registered with `app.manage()`) for
/// subsequent enable/disable calls.
pub fn build_app_menu(app: &AppHandle<Wry>) -> tauri::Result<(tauri::menu::Menu<Wry>, MenuHandles)> {

    // ── File ──────────────────────────────────────────────────────────────
    let connect_item    = MenuItem::with_id(app, "menu-connect",    "Connect…",   true,  None::<&str>)?;
    let disconnect_item = MenuItem::with_id(app, "menu-disconnect", "Disconnect", false, None::<&str>)?;

    let file_submenu = SubmenuBuilder::new(app, "File")
        .item(&connect_item)
        .item(&disconnect_item)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;

    // ── View ──────────────────────────────────────────────────────────────
    let refresh_item = MenuItem::with_id(app, "menu-refresh", "Refresh Nodes",        false, None::<&str>)?;
    let traffic_item = MenuItem::with_id(app, "menu-traffic", "Open Traffic Monitor", false, None::<&str>)?;

    let view_submenu = SubmenuBuilder::new(app, "View")
        .item(&refresh_item)
        .separator()
        .item(&traffic_item)
        .build()?;

    // ── Tools ─────────────────────────────────────────────────────────────
    let view_cdi_item       = MenuItem::with_id(app, "menu-view-cdi",       "View CDI XML for Selected Node",    false, None::<&str>)?;
    let redownload_cdi_item = MenuItem::with_id(app, "menu-redownload-cdi", "Re-download CDI for Selected Node", false, None::<&str>)?;
    let disc_opts_item      = MenuItem::with_id(app, "menu-discovery-opts", "Discovery Options…",                true,  None::<&str>)?;

    let tools_submenu = SubmenuBuilder::new(app, "Tools")
        .item(&view_cdi_item)
        .item(&redownload_cdi_item)
        .separator()
        .item(&disc_opts_item)
        .build()?;

    // ── Help ──────────────────────────────────────────────────────────────
    let help_submenu = SubmenuBuilder::new(app, "Help")
        .item(&PredefinedMenuItem::about(app, None::<&str>, None::<tauri::menu::AboutMetadata<'_>>)?)
        .build()?;

    // ── Assemble ──────────────────────────────────────────────────────────
    let menu = MenuBuilder::new(app)
        .item(&file_submenu)
        .item(&view_submenu)
        .item(&tools_submenu)
        .item(&help_submenu)
        .build()?;

    let handles = MenuHandles {
        disconnect:      disconnect_item,
        refresh_nodes:   refresh_item,
        traffic_monitor: traffic_item,
        view_cdi:        view_cdi_item,
        redownload_cdi:  redownload_cdi_item,
    };

    Ok((menu, handles))
}
