//! Spec 021: offline Save + reopen roundtrip for a Panel facility, and the
//! manifest version policy that came with it, through the real
//! layout-directory write and read paths.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bowties_core::layout::channels::{
    apply_channel_deltas, ChannelBinding, ChannelOwnership, ChannelRole, ChannelsDocument,
    InformationChannel,
};
use bowties_core::layout::facilities::{
    apply_facility_deltas, FacilitiesDocument, Facility, LitWhen, PanelEntry, PanelRecord,
};
use bowties_core::layout::io::{read_layout_capture, write_layout_capture, LayoutDirectoryWriteData};
use bowties_core::layout::manifest::LayoutManifest;
use bowties_core::layout::types::{LayoutEditDelta, LayoutFile};

const NODE: &str = "050101010700";

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bowties_panel_it_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn lamp_channel() -> InformationChannel {
    InformationChannel {
        id: "ch-lamp-1".into(),
        name: "Lamp 1".into(),
        role: ChannelRole::LampIndicator,
        style: "single-led-direct-lamp".into(),
        ownership: ChannelOwnership::UserOwned,
        binding: ChannelBinding::LampRow { node_key: NODE.into(), row_ordinal: 3 },
    }
}

fn panel_record() -> PanelRecord {
    PanelRecord {
        lit_when: LitWhen::Clear,
        entries: vec![
            PanelEntry {
                entry_id: "e1".into(),
                block_id: "blk-1".into(),
                lamp_channel_id: Some("ch-lamp-1".into()),
                lit_when: Some(LitWhen::Occupied),
            },
            PanelEntry {
                entry_id: "e2".into(),
                block_id: "blk-2".into(),
                lamp_channel_id: None,
                lit_when: None,
            },
        ],
    }
}

fn panel_facility() -> Facility {
    Facility {
        facility_id: "panel-1".into(),
        template_id: "panel".into(),
        name: "Main Panel".into(),
        slot_bindings: BTreeMap::from([("lamps".to_string(), vec!["ch-lamp-1".to_string()])]),
        block_signals: None,
        panel: Some(panel_record()),
    }
}

fn data(facilities: FacilitiesDocument, channels: ChannelsDocument) -> LayoutDirectoryWriteData {
    LayoutDirectoryWriteData {
        manifest: LayoutManifest::new(
            "panel-it".into(),
            "2026-11-25T00:00:00Z".into(),
            "2026-11-25T00:00:00Z".into(),
        ),
        node_snapshots: Vec::new(),
        bowties: LayoutFile::default(),
        pending_changes: Vec::new(),
        cdi_files: Vec::new(),
        channels,
        facilities,
    }
}

#[test]
fn offline_save_and_reopen_preserves_panel_facility() {
    let deltas = vec![
        LayoutEditDelta::CreateChannel { channel: lamp_channel() },
        LayoutEditDelta::AddFacility {
            facility: Facility {
                panel: None,
                slot_bindings: BTreeMap::new(),
                ..panel_facility()
            },
        },
        LayoutEditDelta::AttachChannelToSlot {
            facility_id: "panel-1".into(),
            slot_label: "lamps".into(),
            channel_id: "ch-lamp-1".into(),
        },
        LayoutEditDelta::SetPanelRecord {
            facility_id: "panel-1".into(),
            record: panel_record(),
        },
    ];
    let mut facilities = FacilitiesDocument::default();
    let mut channels = ChannelsDocument::default();
    apply_channel_deltas(&mut channels, &deltas).unwrap();
    apply_facility_deltas(&mut facilities, &deltas).unwrap();

    let dir = temp_dir("roundtrip").join("layout");
    write_layout_capture(&dir, &data(facilities, channels)).unwrap();

    let manifest_yaml = std::fs::read_to_string(dir.join("manifest.yaml")).unwrap();
    assert!(manifest_yaml.contains("schemaVersion: 6"), "{manifest_yaml}");
    let facilities_yaml = std::fs::read_to_string(dir.join("facilities.yaml")).unwrap();
    assert!(facilities_yaml.contains("panel:"), "{facilities_yaml}");

    let read = read_layout_capture(&dir).unwrap();
    assert_eq!(read.manifest.schema_version, 6);
    assert_eq!(read.facilities.schema_version, "3.0");
    assert_eq!(read.facilities.facilities, vec![panel_facility()]);
    assert_eq!(read.channels.channels, vec![lamp_channel()]);
    assert!(read.load_warnings.is_empty(), "{:?}", read.load_warnings);
}

fn write_layout_with_manifest_version(dir: &PathBuf, version: u32) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join("manifest.yaml"),
        format!(
            "schemaVersion: {version}\nlayoutId: old\ncapturedAt: t\nlastSavedAt: t\nactiveMode: offline\nmatchThresholds:\n  likelySame: 80\n  uncertainMin: 40\n"
        ),
    )
    .unwrap();
    std::fs::write(
        dir.join("facilities.yaml"),
        "schemaVersion: \"2.0\"\nfacilities:\n  - facilityId: f1\n    templateId: block-indicator\n    name: Old\n    slotBindings: {}\n",
    )
    .unwrap();
}

#[test]
fn manifest_versions_4_5_and_6_open_and_older_facilities_files_stay_readable() {
    for version in [4u32, 5, 6] {
        let dir = temp_dir(&format!("v{version}")).join("layout");
        write_layout_with_manifest_version(&dir, version);
        let read = read_layout_capture(&dir).unwrap();
        assert_eq!(read.manifest.schema_version, version);
        assert_eq!(read.facilities.schema_version, "2.0");
        assert_eq!(read.facilities.facilities[0].name, "Old");
        assert_eq!(read.facilities.facilities[0].panel, None);
    }
}

#[test]
fn manifest_version_7_is_refused_with_newer_bowties_message() {
    let dir = temp_dir("v7").join("layout");
    write_layout_with_manifest_version(&dir, 7);
    let err = read_layout_capture(&dir).unwrap_err();
    assert!(err.contains("newer version of Bowties"), "{err}");
}
