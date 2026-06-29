//! Bundled-profile smoke harness (spec 018 S1.1).
//!
//! Iterates every profile and shared-daughterboard YAML under
//! `app/src-tauri/profiles/` and verifies that each one parses as its
//! matching schema type with a supported `schemaVersion`.
//!
//! Intentionally content-free beyond schema-version conformance: future
//! profile renames or content edits (adding daughterboards, tweaking event
//! roles, etc.) MUST NOT cause this harness to fail. Test assertions that
//! depend on specific profile content belong with capability fixtures under
//! `bowties-core/tests/fixtures/structure-profiles/` (owned by the test) or
//! with bundled tests that legitimately validate the shipping data
//! end-to-end (see the audit table on slice S1.1).
//!
//! Catches:
//! - a shipping profile that no longer parses under the current schema
//! - a shipping profile whose `schemaVersion` declares a value Bowties no
//!   longer supports
//! - relocation of the profiles directory (the harness fails loudly if it
//!   cannot find the directory)

use std::fs;
use std::path::{Path, PathBuf};

use bowties_core::profile::{SharedDaughterboardLibrary, StructureProfile};

/// Schema version every shipping `.profile.yaml` MUST currently declare.
const SUPPORTED_STRUCTURE_PROFILE_SCHEMA: &str = "2.0";

/// Schema version every shipping `.shared-daughterboards.yaml` MUST
/// currently declare.
const SUPPORTED_SHARED_DAUGHTERBOARDS_SCHEMA: &str = "1.0";

fn bundled_profiles_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("app")
        .join("src-tauri")
        .join("profiles")
}

fn read_yaml_entries(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir).unwrap_or_else(|err| {
        panic!(
            "bundled profiles directory must exist at {} (error: {err}). \
             If the profiles directory has moved, update bundled_profiles_dir() in this harness.",
            dir.display()
        )
    });

    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("yaml"))
        })
        .collect();
    paths.sort();
    paths
}

fn is_structure_profile(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".profile.yaml"))
}

fn is_shared_daughterboards(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".shared-daughterboards.yaml"))
}

#[test]
fn every_bundled_structure_profile_parses_with_supported_schema() {
    let dir = bundled_profiles_dir();
    let profile_paths: Vec<PathBuf> = read_yaml_entries(&dir)
        .into_iter()
        .filter(|path| is_structure_profile(path))
        .collect();

    assert!(
        !profile_paths.is_empty(),
        "expected at least one .profile.yaml under {} — \
         did the profiles directory move or get emptied?",
        dir.display()
    );

    for path in profile_paths {
        let yaml = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let profile: StructureProfile = serde_yaml_ng::from_str(&yaml).unwrap_or_else(|err| {
            panic!(
                "shipping profile {} must parse as a v{SUPPORTED_STRUCTURE_PROFILE_SCHEMA} \
                 StructureProfile (error: {err})",
                path.display()
            )
        });
        assert_eq!(
            profile.schema_version, SUPPORTED_STRUCTURE_PROFILE_SCHEMA,
            "shipping profile {} declares schemaVersion {:?} but only \
             {SUPPORTED_STRUCTURE_PROFILE_SCHEMA:?} is currently supported",
            path.display(),
            profile.schema_version,
        );
    }
}

#[test]
fn every_bundled_shared_daughterboards_library_parses_with_supported_schema() {
    let dir = bundled_profiles_dir();
    let library_paths: Vec<PathBuf> = read_yaml_entries(&dir)
        .into_iter()
        .filter(|path| is_shared_daughterboards(path))
        .collect();

    assert!(
        !library_paths.is_empty(),
        "expected at least one .shared-daughterboards.yaml under {} — \
         did the profiles directory move or get emptied?",
        dir.display()
    );

    for path in library_paths {
        let yaml = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let library: SharedDaughterboardLibrary =
            serde_yaml_ng::from_str(&yaml).unwrap_or_else(|err| {
                panic!(
                    "shipping shared-daughterboard library {} must parse as a \
                     v{SUPPORTED_SHARED_DAUGHTERBOARDS_SCHEMA} SharedDaughterboardLibrary \
                     (error: {err})",
                    path.display()
                )
            });
        assert_eq!(
            library.schema_version, SUPPORTED_SHARED_DAUGHTERBOARDS_SCHEMA,
            "shipping shared-daughterboard library {} declares schemaVersion {:?} but only \
             {SUPPORTED_SHARED_DAUGHTERBOARDS_SCHEMA:?} is currently supported",
            path.display(),
            library.schema_version,
        );
    }
}

/// Spec 018 / S3 (ADR-0013): the shipping shared-daughterboard library must
/// expose a non-empty `styles:` catalog containing `bod-block-detector-input`.
/// The BOD-* daughterboards rely on this style for their constraint contract
/// since the inline `validityRules` were retired.
#[test]
fn bundled_shared_daughterboards_library_declares_bod_block_detector_input_style() {
    let dir = bundled_profiles_dir();
    let library_paths: Vec<PathBuf> = read_yaml_entries(&dir)
        .into_iter()
        .filter(|path| is_shared_daughterboards(path))
        .collect();

    assert!(
        !library_paths.is_empty(),
        "expected at least one .shared-daughterboards.yaml under {}",
        dir.display()
    );

    for path in library_paths {
        let yaml = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        let library: SharedDaughterboardLibrary = serde_yaml_ng::from_str(&yaml)
            .unwrap_or_else(|err| panic!("library {} must parse: {err}", path.display()));

        assert!(
            !library.styles.is_empty(),
            "shipping library {} must declare a non-empty `styles:` catalog \
             (Spec 018 / S3 — BOD-* constraint rules now live on styles)",
            path.display()
        );
        assert!(
            library
                .styles
                .iter()
                .any(|style| style.style_id == "bod-block-detector-input"),
            "shipping library {} must declare a `bod-block-detector-input` style",
            path.display()
        );
    }
}
