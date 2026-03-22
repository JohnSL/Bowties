# Release Process

This document describes how to cut a new release of Bowties. GitHub Actions handles the actual build and publishing; your job is to update version numbers and push a tag.

## Version number locations

The version number appears in four files and must be kept in sync:

| File | Key |
|------|-----|
| `app/src-tauri/tauri.conf.json` | `"version"` |
| `app/package.json` | `"version"` |
| `app/src-tauri/Cargo.toml` | `version` under `[package]` |
| `lcc-rs/Cargo.toml` | `version` under `[package]` |

Bowties follows [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`.

## Step-by-step

### 1. Decide the new version

Choose the next version number, e.g. `0.2.0`.

### 2. Update all version files

Edit each file listed above and change the version string to the new value:

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

### 3. Commit the version bump

```powershell
git add app/src-tauri/tauri.conf.json app/package.json app/src-tauri/Cargo.toml lcc-rs/Cargo.toml
git commit -m "chore: bump version to 0.2.0"
```

### 4. Push the commit

```powershell
git push
```

### 5. Create and push the tag

The tag name **must** start with `v` to trigger the release workflow:

```powershell
git tag v0.2.0
git push origin v0.2.0
```

### 6. Monitor the build

Go to the **[Actions](https://github.com/JohnSL/Bowties/actions)** tab in GitHub. The **Release** workflow will:

- Build the Windows NSIS installer
- Build the Linux `.deb` and AppImage packages
- Create a **draft** GitHub Release with all artifacts attached

### 7. Publish the draft release

1. Navigate to **Releases** in GitHub.
2. Open the draft release created by the workflow.
3. Review and edit the release notes as needed.
4. Click **Publish release**.

## What the workflow builds

| Platform | Artifact |
|----------|----------|
| Windows | `Bowties_x.y.z_x64-setup.exe` (NSIS installer) |
| Linux | `bowties_x.y.z_amd64.deb`, `Bowties_x.y.z_amd64.AppImage` |

macOS builds are not currently included in the workflow.

## Hotfix releases

For a patch release (e.g. `0.2.1`), follow the same steps on the relevant branch. If the fix is on `main`, simply increment the patch number and tag from `main`.
