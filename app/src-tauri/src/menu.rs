//! Native application menu setup.
//!
//! Builds the OS-native menu bar and exposes handles to the items that need
//! dynamic enable/disable updates driven by the frontend via the
//! `update_menu_state` Tauri command.
//!
//! ## macOS specifics
//!
//! On macOS the first submenu in the menu bar becomes the application menu
//! (its title is replaced with the app name by AppKit). We build a proper
//! App submenu (About / Services / Hide / Quit) there. The Edit submenu
//! (built cross-platform) uses `PredefinedMenuItem`s so keyboard shortcuts
//! like Cmd+C / Cmd+V / Cmd+X / Cmd+A / Cmd+Z reach focused text inputs
//! through the AppKit responder chain — without an Edit submenu built from
//! predefined items, WKWebView never receives those keystrokes and text
//! fields cannot copy/paste (GitHub issue #19). Windows and Linux WebViews
//! handle those shortcuts natively, so the Edit menu there is just
//! discoverability chrome.

use tauri::{AppHandle, Wry};
use tauri::menu::{MenuBuilder, SubmenuBuilder, MenuItem, PredefinedMenuItem};

/// Handles to menu items whose enabled state must change at runtime.
///
/// `MenuItem<Wry>` is internally Arc-backed and is `Clone + Send + Sync`,
/// so the struct can be stored as managed Tauri state.
pub struct MenuHandles {
    pub disconnect:       MenuItem<Wry>,
    pub refresh_nodes:    MenuItem<Wry>,
    pub traffic_monitor:  MenuItem<Wry>,
    pub view_cdi:         MenuItem<Wry>,
    pub redownload_cdi:   MenuItem<Wry>,
    pub open_layout:      MenuItem<Wry>,
    pub close_layout:     MenuItem<Wry>,
    pub save_layout:      MenuItem<Wry>,
    pub save_layout_as:   MenuItem<Wry>,
    pub sync_to_bus:      MenuItem<Wry>,
    /// Spec 014 / S8 — "Add Placeholder Board…". Enabled only when an
    /// offline layout is the active context.
    pub add_placeholder_board: MenuItem<Wry>,
    /// Spec 014 / S8.5 / T11 — "Delete Placeholder Board…". Enabled only
    /// when the currently selected node is an in-memory placeholder board.
    pub delete_placeholder_board: MenuItem<Wry>,
    pub diagnostics:      MenuItem<Wry>,
}

/// Build the native application menu.
///
/// Returns the assembled `Menu` (to be set on the app with `app.set_menu()`)
/// and a `MenuHandles` value (to be registered with `app.manage()`) for
/// subsequent enable/disable calls.
pub fn build_app_menu(app: &AppHandle<Wry>) -> tauri::Result<(tauri::menu::Menu<Wry>, MenuHandles)> {

    // ── File ──────────────────────────────────────────────────────────────
    let disconnect_item = MenuItem::with_id(app, "menu-disconnect", "Disconnect", false, None::<&str>)?;

    // Exit lives in the File menu on Windows/Linux; on macOS it moves to the
    // App submenu as "Quit Bowties" (built below).
    #[cfg(not(target_os = "macos"))]
    let exit_item = MenuItem::with_id(app, "menu-exit", "Exit", true, None::<&str>)?;

    // Keep shortcut UX in sync with frontend capture logic.
    // When adding/changing these accelerators, update:
    //   app/src/lib/keyboard/menuShortcuts.ts
    let open_layout_item    = MenuItem::with_id(app, "menu-open-layout",    "Open Layout\u{2026}",    false, Some("CmdOrCtrl+O"))?;
    let close_layout_item   = MenuItem::with_id(app, "menu-close-layout",   "Close Layout",          false, Some("CmdOrCtrl+W"))?;
    let save_layout_item    = MenuItem::with_id(app, "menu-save-layout",    "Save Layout",           false, Some("CmdOrCtrl+S"))?;
    let save_layout_as_item = MenuItem::with_id(app, "menu-save-layout-as", "Save Layout As\u{2026}", false, Some("CmdOrCtrl+Shift+S"))?;
    let sync_to_bus_item    = MenuItem::with_id(app, "menu-sync-to-bus",    "Sync to Bus",           false, None::<&str>)?;
    let add_placeholder_item    = MenuItem::with_id(app, "menu-add-placeholder-board",    "Add Placeholder Board\u{2026}",    false, None::<&str>)?;
    let delete_placeholder_item = MenuItem::with_id(app, "menu-delete-placeholder-board", "Delete Placeholder Board\u{2026}", false, None::<&str>)?;

    let file_submenu = {
        let builder = SubmenuBuilder::new(app, "File")
            .item(&disconnect_item)
            .separator()
            .item(&open_layout_item)
            .item(&close_layout_item)
            .separator()
            .item(&save_layout_item)
            .item(&save_layout_as_item)
            .item(&sync_to_bus_item)
            .separator()
            .item(&add_placeholder_item)
            .item(&delete_placeholder_item);
        #[cfg(not(target_os = "macos"))]
        let builder = builder.separator().item(&exit_item);
        builder.build()?
    };

    // ── Edit ──────────────────────────────────────────────────────────────
    // On macOS these predefined items must exist in the native menu so
    // Cmd+C / Cmd+V / Cmd+X / Cmd+A / Cmd+Z / Cmd+Shift+Z are routed to
    // focused text inputs through the AppKit responder chain (GitHub
    // issue #19). On Windows/Linux the WebView already handles these
    // shortcuts natively, but a conventional Edit menu is still useful
    // for discoverability, so we include it on all platforms.
    //
    // Undo/Redo are macOS-only: on Windows (WebView2) and Linux
    // (WebKitGTK) the predefined items send platform commands that the
    // WebView does not respond to, so they would appear as non-functional
    // menu entries. The native Ctrl+Z / Ctrl+Y shortcuts inside text
    // fields still work; they just don't get a menu-bar entry there.
    let edit_submenu = {
        #[cfg(target_os = "macos")]
        let undo_p = PredefinedMenuItem::undo(app, None)?;
        #[cfg(target_os = "macos")]
        let redo_p = PredefinedMenuItem::redo(app, None)?;
        let cut_p        = PredefinedMenuItem::cut(app, None)?;
        let copy_p       = PredefinedMenuItem::copy(app, None)?;
        let paste_p      = PredefinedMenuItem::paste(app, None)?;
        let select_all_p = PredefinedMenuItem::select_all(app, None)?;
        let builder = SubmenuBuilder::new(app, "Edit");
        #[cfg(target_os = "macos")]
        let builder = builder.item(&undo_p).item(&redo_p).separator();
        builder
            .item(&cut_p)
            .item(&copy_p)
            .item(&paste_p)
            .item(&select_all_p)
            .build()?
    };

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
    let diagnostics_item    = MenuItem::with_id(app, "menu-diagnostics",    "Copy Diagnostic Report",            false, None::<&str>)?;

    let tools_submenu = SubmenuBuilder::new(app, "Tools")
        .item(&view_cdi_item)
        .item(&redownload_cdi_item)
        .separator()
        .item(&diagnostics_item)
        .build()?;

    // ── Help ──────────────────────────────────────────────────────────────
    // "About Bowties" lives in the Help menu on Windows/Linux and in the
    // macOS App submenu on macOS. Dropping the Help submenu entirely on
    // macOS is fine while About is its only entry; if new Help items are
    // added later, restore a macOS Help submenu without an About entry.
    #[cfg(not(target_os = "macos"))]
    let help_submenu = {
        let about_item = MenuItem::with_id(app, "menu-about", "About Bowties", true, None::<&str>)?;
        SubmenuBuilder::new(app, "Help").item(&about_item).build()?
    };

    // ── App submenu (macOS only) ─────────────────────────────────────────
    // The first submenu in a macOS menu bar becomes the application menu
    // (its title is replaced with the app name by AppKit). About and Quit
    // are intentionally custom `MenuItem`s (not `PredefinedMenuItem`s) so
    // they emit the existing `menu-about` and `menu-exit` events and the
    // frontend's Bowties-branded About dialog and unsaved-changes exit
    // prompt still run. Services / Hide / Hide Others / Show All *must* be
    // predefined items to invoke the native AppKit behaviours.
    #[cfg(target_os = "macos")]
    let app_submenu = {
        let about_item    = MenuItem::with_id(app, "menu-about", "About Bowties", true, None::<&str>)?;
        let services_p    = PredefinedMenuItem::services(app, None)?;
        let hide_p        = PredefinedMenuItem::hide(app, None)?;
        let hide_others_p = PredefinedMenuItem::hide_others(app, None)?;
        let show_all_p    = PredefinedMenuItem::show_all(app, None)?;
        let quit_item     = MenuItem::with_id(app, "menu-exit", "Quit Bowties", true, Some("Cmd+Q"))?;
        SubmenuBuilder::new(app, "Bowties")
            .item(&about_item)
            .separator()
            .item(&services_p)
            .separator()
            .item(&hide_p)
            .item(&hide_others_p)
            .item(&show_all_p)
            .separator()
            .item(&quit_item)
            .build()?
    };

    // ── Assemble ──────────────────────────────────────────────────────────
    let builder = MenuBuilder::new(app);
    #[cfg(target_os = "macos")]
    let builder = builder.item(&app_submenu);
    let builder = builder
        .item(&file_submenu)
        .item(&edit_submenu)
        .item(&view_submenu)
        .item(&tools_submenu);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.item(&help_submenu);
    let menu = builder.build()?;

    let handles = MenuHandles {
        disconnect:      disconnect_item,
        refresh_nodes:   refresh_item,
        traffic_monitor: traffic_item,
        view_cdi:        view_cdi_item,
        redownload_cdi:  redownload_cdi_item,
        open_layout:     open_layout_item,
        close_layout:    close_layout_item,
        save_layout:     save_layout_item,
        save_layout_as:  save_layout_as_item,
        sync_to_bus:     sync_to_bus_item,
        add_placeholder_board: add_placeholder_item,
        delete_placeholder_board: delete_placeholder_item,
        diagnostics:     diagnostics_item,
    };

    Ok((menu, handles))
}
