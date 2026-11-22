//! Spec 020 / E1a: offline Save + reopen roundtrip for an Automatic Block
//! Signals facility, through the real layout-directory write and read paths.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bowties_core::layout::channels::{
    apply_channel_deltas, ChannelBinding, ChannelOwnership, ChannelRole, ChannelsDocument,
    InformationChannel,
};
use bowties_core::layout::facilities::{
    apply_facility_deltas, BlockSignalsRecord, ClaimedSlot, ClaimedSlotKind, FacilitiesDocument,
    Facility, SignalRecord,
};
use bowties_core::layout::io::{read_layout_capture, write_layout_capture, LayoutDirectoryWriteData};
use bowties_core::layout::manifest::LayoutManifest;
use bowties_core::layout::offline_changes::{OfflineChange, OfflineChangeKind, OfflineChangeStatus};
use bowties_core::layout::types::{LayoutEditDelta, LayoutFile};
use bowties_core::signaling::{
    EntranceEnd, ExitEnd, FacilitySignalSettings, Movements, SelectedBlock, TravelDirection,
};

const NODE: &str = "050101010700";

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bowties_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn signal_channel() -> InformationChannel {
    InformationChannel {
        id: "ch-sig-1".into(),
        name: "Signal 1".into(),
        role: ChannelRole::SignalIndication,
        style: "signal-lcc-rg".into(),
        ownership: ChannelOwnership::UserOwned,
        binding: ChannelBinding::SignalLccRgOutputs {
            node_key: NODE.into(),
            green_output: 1,
            red_output: 2,
        },
    }
}

fn abs_facility() -> Facility {
    Facility {
        facility_id: "abs-1".into(),
        template_id: "automatic-block-signals".into(),
        name: "Main ABS".into(),
        slot_bindings: BTreeMap::from([("signals".to_string(), vec!["ch-sig-1".to_string()])]),
        block_signals: Some(BlockSignalsRecord {
            direction: TravelDirection::Westward,
            movements: Movements::OneDirection,
            entrance_end: EntranceEnd::NoPlant,
            exit_end: ExitEnd::NothingBeyond,
            blocks: vec![SelectedBlock {
                block_id: "blk-1".into(),
                name: "Main 1".into(),
                joined_to_previous: false,
                turnouts: vec![],
            }],
            settings: FacilitySignalSettings::default(),
            signals: vec![SignalRecord {
                signal_id: "sig-0".into(),
                name: "Signal 1".into(),
                protected_block: "blk-1".into(),
                channel_id: "ch-sig-1".into(),
            }],
            claimed_slots: vec![ClaimedSlot {
                node_key: NODE.into(),
                kind: ClaimedSlotKind::Mast,
                index: 3,
            }],
        }),
    }
}

fn config_change(id: &str, offset: &str, value: &str) -> OfflineChange {
    OfflineChange {
        change_id: id.into(),
        kind: OfflineChangeKind::Config,
        node_key: Some(NODE.into()),
        space: Some(253),
        offset: Some(offset.into()),
        baseline_value: "0".into(),
        planned_value: value.into(),
        status: OfflineChangeStatus::Pending,
        error: None,
        updated_at: "2026-11-22T00:00:00Z".into(),
    }
}

#[test]
fn offline_save_and_reopen_preserves_block_signals_facility() {
    let deltas = vec![
        LayoutEditDelta::CreateChannel { channel: signal_channel() },
        LayoutEditDelta::AddFacility { facility: abs_facility() },
        LayoutEditDelta::AttachChannelToSlot {
            facility_id: "abs-1".into(),
            slot_label: "signals".into(),
            channel_id: "ch-sig-1".into(),
        },
    ];
    let mut facilities = FacilitiesDocument::default();
    let mut channels = ChannelsDocument::default();
    apply_channel_deltas(&mut channels, &deltas).unwrap();
    apply_facility_deltas(&mut facilities, &deltas).unwrap();

    let data = LayoutDirectoryWriteData {
        manifest: LayoutManifest::new(
            "abs-it".into(),
            "2026-11-22T00:00:00Z".into(),
            "2026-11-22T00:00:00Z".into(),
        ),
        node_snapshots: Vec::new(),
        bowties: LayoutFile::default(),
        offline_changes: vec![
            config_change("chg-1", "0x00000010", "5"),
            config_change("chg-2", "0x00000020", "7"),
        ],
        cdi_files: Vec::new(),
        channels,
        facilities,
    };
    let dir = temp_dir("roundtrip").join("layout");
    write_layout_capture(&dir, &data).unwrap();

    let manifest_yaml = std::fs::read_to_string(dir.join("manifest.yaml")).unwrap();
    assert!(manifest_yaml.contains("schemaVersion: 5"), "{manifest_yaml}");
    let facilities_yaml = std::fs::read_to_string(dir.join("facilities.yaml")).unwrap();
    assert!(facilities_yaml.contains("blockSignals:"), "{facilities_yaml}");

    let read = read_layout_capture(&dir).unwrap();
    assert_eq!(read.manifest.schema_version, 5);
    assert_eq!(read.facilities.schema_version, "2.0");
    assert_eq!(read.facilities.facilities, vec![abs_facility()]);
    let record = read.facilities.facilities[0].block_signals.as_ref().unwrap();
    assert_eq!(record.signals[0].channel_id, "ch-sig-1");
    assert_eq!(record.claimed_slots[0].index, 3);
    assert_eq!(read.facilities.facilities[0].slot_bindings["signals"], vec!["ch-sig-1"]);
    assert_eq!(read.channels.channels, vec![signal_channel()]);
    assert!(read.load_warnings.is_empty(), "{:?}", read.load_warnings);
    assert_eq!(read.offline_changes.len(), 2);
    assert!(read.offline_changes.iter().all(|c| c.node_key.as_deref() == Some(NODE)));
    assert_eq!(read.offline_changes[1].offset.as_deref(), Some("0x00000020"));
    assert_eq!(read.offline_changes[1].planned_value, "7");
}

fn write_v4_layout(dir: &PathBuf) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("manifest.yaml"),
        "schemaVersion: 4\nlayoutId: old\ncapturedAt: t\nlastSavedAt: t\nactiveMode: offline\nmatchThresholds:\n  likelySame: 80\n  uncertainMin: 40\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("facilities.yaml"),
        "schemaVersion: \"1.0\"\nfacilities:\n  - facilityId: f1\n    templateId: block-indicator\n    name: Old\n    slotBindings: {}\n",
    )
    .unwrap();
}

#[test]
fn v4_layout_opens_and_facilities_without_block_signals_load_unchanged() {
    let dir = temp_dir("v4").join("layout");
    write_v4_layout(&dir);
    let read = read_layout_capture(&dir).unwrap();
    assert_eq!(read.manifest.schema_version, 4);
    assert_eq!(read.facilities.facilities.len(), 1);
    assert_eq!(read.facilities.facilities[0].name, "Old");
    assert_eq!(read.facilities.facilities[0].block_signals, None);
}

#[test]
fn schema_version_6_is_refused_with_newer_bowties_message_on_open() {
    let dir = temp_dir("v6").join("layout");
    write_v4_layout(&dir);
    let m = std::fs::read_to_string(dir.join("manifest.yaml"))
        .unwrap()
        .replace("schemaVersion: 4", "schemaVersion: 6");
    std::fs::write(dir.join("manifest.yaml"), m).unwrap();
    let err = read_layout_capture(&dir).unwrap_err();
    assert!(err.contains("newer version of Bowties"), "{err}");
}

fn data_with_facilities(facilities: FacilitiesDocument) -> LayoutDirectoryWriteData {
    LayoutDirectoryWriteData {
        manifest: LayoutManifest::new(
            "abs-it".into(),
            "2026-11-22T00:00:00Z".into(),
            "2026-11-22T00:00:00Z".into(),
        ),
        node_snapshots: Vec::new(),
        bowties: LayoutFile::default(),
        offline_changes: Vec::new(),
        cdi_files: Vec::new(),
        channels: ChannelsDocument::default(),
        facilities,
    }
}

fn v1_facilities_document() -> FacilitiesDocument {
    serde_yaml_ng::from_str("schemaVersion: \"1.0\"
facilities: []
").unwrap()
}

#[test]
fn applying_a_facility_to_a_v1_document_saves_it_as_current_schema_version() {
    let mut facilities = v1_facilities_document();
    apply_facility_deltas(
        &mut facilities,
        &[LayoutEditDelta::AddFacility { facility: abs_facility() }],
    )
    .unwrap();
    let dir = temp_dir("v1_apply").join("layout");
    write_layout_capture(&dir, &data_with_facilities(facilities)).unwrap();
    let yaml = std::fs::read_to_string(dir.join("facilities.yaml")).unwrap();
    assert!(yaml.contains("schemaVersion: '2.0'") || yaml.contains("schemaVersion: \"2.0\""), "{yaml}");
    assert_eq!(read_layout_capture(&dir).unwrap().facilities.schema_version, "2.0");
}

#[test]
fn saving_an_untouched_v1_facilities_document_stamps_current_schema_version() {
    let dir = temp_dir("v1_untouched").join("layout");
    write_layout_capture(&dir, &data_with_facilities(v1_facilities_document())).unwrap();
    assert_eq!(read_layout_capture(&dir).unwrap().facilities.schema_version, "2.0");
}
