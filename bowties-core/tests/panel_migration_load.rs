use std::collections::{BTreeMap, HashMap};
use bowties_core::layout::{self, LayoutDirectoryWriteData};
use bowties_core::layout::channels::{ChannelBinding, ChannelOwnership, ChannelRole, ChannelsDocument, InformationChannel};
use bowties_core::layout::facilities::{FacilitiesDocument, Facility, LitWhen};
use bowties_core::layout::manifest::LayoutManifest;
use bowties_core::layout::state::LayoutState;
use bowties_core::layout::types::{BowtieMetadata, LayoutFile};

fn block(id: &str) -> InformationChannel {
    InformationChannel {
        id: id.into(), name: id.into(), role: ChannelRole::BlockOccupancy,
        style: "detector".into(), ownership: ChannelOwnership::HardwareOwned,
        binding: ChannelBinding::ConnectorInput { node_key: "050101010700".into(), connector: "H1".into(), input: 1 },
    }
}

fn lamp(id: &str, node: &str, row: u32) -> InformationChannel {
    InformationChannel {
        id: id.into(), name: "old channel name".into(), role: ChannelRole::LampIndicator,
        style: "single-led-direct-lamp".into(), ownership: ChannelOwnership::UserOwned,
        binding: ChannelBinding::LampRow { node_key: node.into(), row_ordinal: row },
    }
}

fn indicator(id: &str, block: &str, lamp: Option<&str>) -> Facility {
    Facility {
        facility_id: id.into(), template_id: "block-indicator".into(), name: format!("Old {id}"),
        slot_bindings: BTreeMap::from([
            ("input".into(), vec![block.into()]),
            ("output".into(), lamp.into_iter().map(String::from).collect()),
        ]),
        block_signals: None, panel: None,
    }
}

fn board(key: &str, name: &str) -> layout::node_snapshot::NodeSnapshot {
    serde_yaml_ng::from_str(&format!(
        "nodeKey: \"{key}\"\nnodeId: \"{key}\"\ncapturedAt: t\ncaptureStatus: complete\nsnip:\n  userName: {name}\n  userDescription: ''\n  manufacturerName: RR-Cirkits\n  modelName: Signal LCC\ncdiRef:\n  cacheKey: signal\n  version: v1\n  fingerprint: saved\nconfig:\n  Direct Lamp Control:\n    Lamp 9:\n      Lamp On:\n        value: '0102030405060708'\n        space: 253\n        offset: '0x00000010'\n      Lamp Off:\n        value: '0102030405060709'\n        space: 253\n        offset: '0x00000018'\n"
    )).unwrap()
}

#[test]
fn canonical_legacy_read_groups_in_source_order_preserves_names_bindings_and_events_without_writes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("migration-load-fixture");
    if root.exists() { std::fs::remove_dir_all(&root).unwrap(); }
    let mut bowties = LayoutFile::default();
    bowties.bowties.insert("0102030405060708".into(), BowtieMetadata {
        name: Some("Main occupied".into()), tags: vec!["keep".into()], created_by_facility: Some("a".into()),
    });
    let mut manifest = LayoutManifest::new("legacy".into(), "t".into(), "t".into());
    manifest.schema_version = 5;
    let data = LayoutDirectoryWriteData {
        manifest, bowties, node_snapshots: vec![board("050101010701", "Eagle Creek Fascia"), board("050101010702", "West Signals")], pending_changes: vec![], cdi_files: vec![],
        channels: ChannelsDocument::new(vec![block("b1"), block("b2"), block("b3"),
            lamp("l1", "050101010701", 9), lamp("l2", "050101010702", 13), lamp("l3", "050101010701", 3)]),
        facilities: FacilitiesDocument::new(vec![
            indicator("a", "b1", Some("l1")), indicator("b", "b1", Some("l2")),
            indicator("c", "b2", Some("l3")), indicator("d", "b3", None),
        ]),
    };
    layout::save_capture(&root, &data).unwrap();
    let before: Vec<_> = ["manifest.yaml", "channels.yaml", "facilities.yaml", "bowties.yaml", "offline-changes.yaml"]
        .into_iter().map(|f| (f, std::fs::read(root.join(f)).unwrap())).collect();
    let loaded = layout::read_capture(&root).unwrap();
    assert!(loaded.facilities.facilities.iter().all(|f| f.template_id == "panel"), "legacy runtime facilities must be converted");
    assert_eq!(loaded.facilities.facilities.len(), 3);
    let panels = &loaded.facilities.facilities;
    assert_eq!(panels[0].name, "Eagle Creek Fascia panel");
    assert_eq!(panels[1].name, "West Signals panel");
    assert_eq!(loaded.panel_migration.as_ref().unwrap().indicators, 4);
    assert_eq!(loaded.panel_migration.as_ref().unwrap().panels, 3);
    for (actual, original) in loaded.node_snapshots.iter().zip(&data.node_snapshots) {
        assert_eq!(serde_yaml_ng::to_string(&actual.config).unwrap(), serde_yaml_ng::to_string(&original.config).unwrap());
    }
    assert_eq!(panels[0].panel.as_ref().unwrap().entries.iter().map(|e| e.block_id.as_str()).collect::<Vec<_>>(), ["b1", "b2"]);
    assert_eq!(panels[1].panel.as_ref().unwrap().entries[0].block_id, "b1");
    assert_eq!(panels[2].name, "Indicators without a lamp");
    assert_eq!(panels[0].panel.as_ref().unwrap().lit_when, LitWhen::Occupied);
    for (actual, original) in loaded.channels.channels.iter().zip(&data.channels.channels) {
        assert_eq!(actual.id, original.id);
        assert_eq!(actual.binding, original.binding);
        assert_eq!(actual.style, original.style);
        assert_eq!(actual.ownership, original.ownership);
    }

    assert_eq!(loaded.channels.channels[3].name, "Old a");
    let meta = &loaded.bowties.bowties["0102030405060708"];
    assert_eq!(meta.name.as_deref(), Some("Main occupied"));
    assert_eq!(meta.tags, ["keep"]);
    assert_eq!(meta.created_by_facility, None);
    assert!(loaded.pending_changes.is_empty());
    let state = LayoutState::from_loaded(root.clone(), loaded, HashMap::new(), HashMap::new());
    assert!(state.effective_facilities().facilities.iter().all(|f| f.template_id == "panel"));
    assert_eq!(layout::read_facilities(&root).unwrap(), *state.effective_facilities());
    for (file, bytes) in before { assert_eq!(std::fs::read(root.join(file)).unwrap(), bytes); }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_and_same_panel_collisions_warn_first_accepted_wins_and_conversion_is_idempotent() {
        let mut channels = ChannelsDocument::new(vec![block("b1"), block("b2"),
            lamp("l1", "050101010701", 9), lamp("l2", "05.01.01.01.07.01", 3),
            lamp("bad-lamp", "not-a-node", 1)]);
        let mut many = indicator("many", "b1", None);
        many.slot_bindings.get_mut("input").unwrap().push("b2".into());
        let mut facilities = FacilitiesDocument::new(vec![
            indicator("first", "b1", Some("l1")),
            indicator("same-block", "b1", Some("l2")),
            indicator("same-lamp", "b2", Some("l1")),
            indicator("bad", "b2", Some("bad-lamp")), many,
            indicator("empty", "missing", None),
            indicator("without", "b2", None),
        ]);
        let mut bowties = LayoutFile::default();
        for (event, owner) in [("retired", Some("bad")), ("manual", None), ("abs", Some("supported-abs"))] {
            bowties.bowties.insert(event.into(), BowtieMetadata {
                name: Some(event.into()), tags: vec![], created_by_facility: owner.map(String::from),
            });
        }

        let result = layout::panel_migration::migrate(&mut facilities, &mut channels, &mut bowties, &[]);
        assert_eq!(result.indicators, 2, "only first accepted and lamp-less records convert");
        assert_eq!(result.panels, 2);
        assert_eq!(result.warnings.len(), 5);
        assert!(result.warnings.iter().any(|w| w.contains("Old bad") && w.contains("malformed")));
        assert!(result.warnings.iter().any(|w| w.contains("Old same-block") && w.contains("first accepted")));
        assert_eq!(channels.channels[2].name, "Old first");
        assert_eq!(channels.channels[3].name, "old channel name");
        assert_eq!(bowties.bowties["retired"].created_by_facility, None);
        assert_eq!(bowties.bowties["abs"].created_by_facility.as_deref(), Some("supported-abs"));
        let once = serde_yaml_ng::to_string(&(&facilities, &channels, &bowties)).unwrap();
        assert_eq!(layout::panel_migration::migrate(&mut facilities, &mut channels, &mut bowties, &[]), Default::default());
        assert_eq!(serde_yaml_ng::to_string(&(&facilities, &channels, &bowties)).unwrap(), once);
    }

#[test]
fn malformed_identity_is_dropped_without_colliding_with_existing_panels_or_clearing_their_backlinks() {
    let existing = Facility {
        facility_id: "supported".into(), template_id: "panel".into(), name: "Existing".into(),
        slot_bindings: BTreeMap::new(), block_signals: None, panel: None,
    };
    let mut facilities = FacilitiesDocument::new(vec![
        existing.clone(), indicator("", "b1", Some("l1")),
        indicator("supported", "b1", Some("l1")),
        indicator("repeat", "b1", Some("l1")), indicator("repeat", "b2", Some("l2")),
    ]);
    let mut channels = ChannelsDocument::new(vec![block("b1"), block("b2"),
        lamp("l1", "050101010701", 1), lamp("l2", "050101010702", 1)]);
    let mut bowties = LayoutFile::default();
    bowties.bowties.insert("event".into(), BowtieMetadata {
        name: Some("Keep".into()), tags: vec![], created_by_facility: Some("supported".into()),
    });
    let outcome = layout::panel_migration::migrate(&mut facilities, &mut channels, &mut bowties, &[]);
    assert_eq!(outcome.indicators, 1);
    assert_eq!(outcome.warnings.len(), 3);
    assert_eq!(facilities.facilities.len(), 2);
    assert_eq!(facilities.facilities[0], existing);
    assert_eq!(bowties.bowties["event"].created_by_facility.as_deref(), Some("supported"));
}

#[test]
fn malformed_legacy_wire_record_warns_and_drops_without_rejecting_valid_records() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target").join("malformed-migration-fixture");
    if root.exists() { std::fs::remove_dir_all(&root).unwrap(); }
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("manifest.yaml"), serde_yaml_ng::to_string(&LayoutManifest::new("old".into(), "t".into(), "t".into())).unwrap()).unwrap();
    std::fs::write(root.join("channels.yaml"), serde_yaml_ng::to_string(&ChannelsDocument::new(vec![block("b1")])).unwrap()).unwrap();
    std::fs::write(root.join("facilities.yaml"),
        "schemaVersion: '2.0'\nfacilities:\n  - facilityId: broken\n    templateId: block-indicator\n    name: Broken old lamp\n    slotBindings: invalid\n  - facilityId: good\n    templateId: block-indicator\n    name: Good\n    slotBindings:\n      input: [b1]\n      output: []\n").unwrap();
    let mut bowties = LayoutFile::default();
    bowties.bowties.insert("event".into(), BowtieMetadata {
        name: Some("Old name".into()), tags: vec![], created_by_facility: Some("broken".into()),
    });
    std::fs::write(root.join("bowties.yaml"), serde_yaml_ng::to_string(&bowties).unwrap()).unwrap();
    let before = std::fs::read(root.join("facilities.yaml")).unwrap();
    let loaded = layout::read_capture(&root).expect("valid records still load");
    assert_eq!(loaded.facilities.facilities.len(), 1);
    assert_eq!(loaded.facilities.facilities[0].name, "Indicators without a lamp");
    let outcome = loaded.panel_migration.unwrap();
    assert_eq!(outcome.indicators, 1);
    assert_eq!(outcome.warnings.len(), 1);
    assert!(outcome.warnings[0].contains("Broken old lamp"));
    assert!(outcome.warnings[0].contains("malformed"));
    assert_eq!(loaded.bowties.bowties["event"].name.as_deref(), Some("Old name"));
    assert_eq!(loaded.bowties.bowties["event"].created_by_facility, None);
    assert_eq!(std::fs::read(root.join("facilities.yaml")).unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}
