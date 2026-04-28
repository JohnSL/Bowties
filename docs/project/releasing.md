# Release Process

This document describes how to cut a new Bowties release. The process has two separate operations:

1. **Prepare the release locally**: confirm the version bump, review user-facing docs, run validation, and push the release-preparation commit.
2. **Publish the release**: create and push the release tag from the current synced version, then prepare end-user release notes markdown for the GitHub draft release.

GitHub Actions handles the packaged builds after the tag is pushed. The local preparation work and the later publication handoff are separate steps on purpose.

If you are using Copilot inside this repository, the shared skills `/release-prepare` and `/release-publish` follow this same workflow. `/release-prepare` reads the current synced version from the repo, asks for the target version, and then walks the local preparation steps. `/release-publish` reads the current synced version from the repo, creates and pushes the release tag, and then prepares end-user release notes markdown.

## Version number locations

The Bowties release version appears in four files and must stay in sync:

| File | Key |
|------|-----|
| `app/src-tauri/tauri.conf.json` | `"version"` |
| `app/package.json` | `"version"` |
| `app/src-tauri/Cargo.toml` | `version` under `[package]` |
| `lcc-rs/Cargo.toml` | `version` under `[package]` |

Before you start a release, confirm that all four files already agree on the current version. If they do not, stop and reconcile them before choosing the next version.

Bowties follows [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`.

## Part 1: Prepare the release locally

### 1. Choose the next version

Start from the current synced version in the four files above and choose the next release version, for example `0.2.0`.

### 2. Review user-facing documentation

Do this before changing the version number. A release should not ship with stale user-facing instructions.

Review these surfaces:

- `README.md` for the top-level overview, supported hardware table, and links
- `docs/user/installing.md` for installer names, platform support, install paths, and security-warning guidance
- `docs/user/using.md` for workflow steps, UI labels, and supported adapter wording
- `docs/images/` for screenshots that may no longer match the current UI
- `docs/project/releasing.md` for release workflow or artifact expectations that changed during the work

Things to check:

- installer filenames and platform availability still match what the workflow produces
- supported hardware and connection methods still match the application
- screenshots still reflect the current UI and current wording
- any release-visible behavior changes are documented for end users

If the release includes user-visible changes and the docs are stale, update them in the same release-preparation commit.

### 3. Update all version files

Edit each version file and change the version string to the new value:

```jsonc
// app/src-tauri/tauri.conf.json
"version": "0.2.0",
```

```jsonc
// app/package.json
"version": "0.2.0",
```

```toml
# app/src-tauri/Cargo.toml
[package]
version = "0.2.0"
```

```toml
# lcc-rs/Cargo.toml
[package]
version = "0.2.0"
```

### 4. Run validation

Run the standard release validation commands before committing:

```powershell
cd lcc-rs
cargo test
```

```powershell
cd app/src-tauri
cargo test
```

```powershell
cd app
npm test
```

If the release touches the offline layout, sync, discovery, or config-read workflow, also run:

```powershell
cd app
npm run test:refactor-gate
```

### 5. Commit the release preparation

Include the version files and any release-related doc updates in the same commit:

```powershell
git add app/src-tauri/tauri.conf.json app/package.json app/src-tauri/Cargo.toml lcc-rs/Cargo.toml README.md docs/user/installing.md docs/user/using.md docs/project/releasing.md docs/images
git commit -m "chore: bump version to 0.2.0"
```

Only stage the files that actually changed. The command above is the full set of common release surfaces, not a requirement to modify every one of them.

### 6. Push the commit

```powershell
git push
```

Stop here. Tagging is part of the publish step so it always uses the current synced version from the already-prepared files.

## Part 2: Publish the release

### 1. Confirm the current synced version

Before creating the tag, confirm again that these four files still agree on the same version:

- `app/src-tauri/tauri.conf.json`
- `app/package.json`
- `app/src-tauri/Cargo.toml`
- `lcc-rs/Cargo.toml`

The publish step should derive the tag name from those files, not from memory.

### 2. Create and push the tag

The tag name **must** start with `v` to trigger the release workflow. If the current synced version is `0.2.0`, the tag is `v0.2.0`:

```powershell
git tag v0.2.0
git push origin v0.2.0
```

### 3. Monitor the build

Go to the **[Actions](https://github.com/JohnSL/Bowties/actions)** tab in GitHub. The **Release** workflow will:

- build the Windows NSIS installer
- build the Linux `.deb` and AppImage packages for x86-64
- build the Linux `.deb` package for ARM64
- create a **draft** GitHub Release with all artifacts attached

### 4. Prepare end-user release notes markdown

Write release notes for end users. They should explain what changed from the user's point of view and should not require knowledge of the implementation.

Use this structure:

```md
## Summary

One or two sentences about what this release gives users.

## What's new

- User-visible change
- User-visible change

## Install and compatibility notes

- Platform, installer, or upgrade note when relevant

## Documentation

- User-facing doc updates or guidance changes when relevant

## Notes

- Optional user-relevant limitation or caution
```

Rules:

- focus on user-visible behavior, not implementation details
- do not mention file names, functions, or internal refactors
- omit empty sections if they add no value

### 5. Paste the markdown into the draft release and publish it manually

1. Navigate to **Releases** in GitHub.
2. Open the draft release created by the workflow.
3. Review the attached artifacts and confirm they match the expected release version.
4. Paste the prepared release notes markdown into the release notes editor.
5. Mark the release as the latest public release if GitHub has not already done so.
6. Click **Publish release**.

## Release notes checklist

Release notes should summarize the user-visible changes in the release, not just the implementation details.

Include, when relevant:

- new user-visible features
- changed workflows or UI terminology
- supported hardware or install-path changes
- documentation updates users may need to know about
- any known limitations that matter for adoption

## What the workflow builds

| Platform | Artifact |
|----------|----------|
| Windows x86-64 | `Bowties_x.y.z_x64-setup.exe` (NSIS installer) |
| Linux x86-64 | `bowties_x.y.z_amd64.deb`, `Bowties_x.y.z_amd64.AppImage` |
| Linux ARM64 | `bowties_x.y.z_arm64.deb` |

macOS builds are not currently included in the workflow.

## Hotfix releases

For a patch release such as `0.2.1`, follow the same process on the relevant branch. If the fix is on `main`, increment the patch version, repeat the documentation review, rerun validation, push the release-preparation commit, and then tag from `main` during the publish step.
