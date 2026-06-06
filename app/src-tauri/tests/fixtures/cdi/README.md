# CDI Fixtures

Real captured CDI XML from physical LCC nodes. Used as regression guards so
profile signatures (e.g. `firmware-revision` in
`profiles/RR-CirKits_Tower-LCC.profile.yaml`) stay aligned with the CDIs that
real hardware actually emits.

Synthetic CDIs built inline in unit tests can drift from real-hardware shape
without anyone noticing until a user connects. These fixtures catch that.

## Fixtures

- `tower-lcc-legacy.xml` — RR-CirKits Tower-LCC running legacy firmware.
  Output Function has 17 enum entries, Input Function has 9.

## How to add a new fixture

Bowties writes node CDIs into the active layout's companion directory on save.
Copy the relevant `<node-id>.cdi.xml` into this folder and add a Rust test in
`src/profile/mod.rs` that loads it via `include_str!` and asserts the expected
profile-build outcome.
