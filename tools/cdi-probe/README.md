# cdi-probe

CLI diagnostic tool for LCC node discovery and CDI download timing.

Exercises the same `lcc-rs` code paths as the Bowties Tauri app (serial
transport → connection → `PeerSessionRegistry` → `PeerSessionHandle::download_cdi`)
so hardware timing bugs can be reproduced and swept without the frontend
in the loop. Useful for tuning `post_ack_delay_ms`, chunk timeouts, and
verifying back-to-back reliability against a real peer (e.g. SPROG USB-LCC).

## Build

```powershell
cd tools/cdi-probe
cargo build --release
```

Release build is recommended when the tool is used for timing measurement so
the numbers aren't distorted by debug overhead.

## Usage

### Discover nodes on the bus

```powershell
cargo run --release -- --port COM8 discover
```

Output:

```
Discovered 7 node(s) in 512ms:
NodeID               Alias
------               -----
02.01.2C.02.17.00    0x62D
02.01.57.00.02.D9    0x3AE
...
```

### Download CDI once (default 5 iterations)

```powershell
cargo run --release -- --port COM8 cdi --node 02.01.57.00.02.D9
```

Output:

```
  #  status    total_ms   chunks     bytes     min    mean     p95     max
  1  ok           31664      232     14826      33     136     140     152
  2  ok           31683      232     14826      33     136     140     149
  ...

── Summary ─────────────────────────────────────────
  iterations              : 5
  successes               : 5 (100.0%)
  failures                : 0
  post_ack_delay_ms       : 100
  timeout_ms (per chunk)  : 5000
  total DR retries        : 0
  total_duration_ms       : min=31664 median=31683 mean=31742 p95=31860 max=31860
```

### Sweep `post_ack_delay_ms` values

Save as `sweep-pacing.ps1` next to the binary:

```powershell
param(
    [string]$Port = "COM8",
    [string]$Node = "02.01.57.00.02.D9",
    [int]$Iterations = 10,
    [int[]]$Delays = @(0, 25, 50, 75, 100)
)

foreach ($d in $Delays) {
    Write-Host "`n=== post_ack_delay_ms=$d ===" -ForegroundColor Cyan
    cargo run --quiet --release -- `
        --port $Port cdi `
        --node $Node `
        --iterations $Iterations `
        --post-ack-delay-ms $d
}
```

Run it:

```powershell
.\sweep-pacing.ps1
```

Any failed iteration exits with code 1, so you'll notice regressions.

### JSON output for scripted analysis

```powershell
cargo run --release -- --port COM8 cdi `
    --node 02.01.57.00.02.D9 `
    --iterations 20 `
    --post-ack-delay-ms 50 `
    --json > run.jsonl
```

Each line is either an iteration record (`{"iteration": N, "status": "ok", ...}`
or `{"iteration": N, "status": "err", ...}`) or the trailing summary
record. Pipe through `jq` / `ConvertFrom-Json` for analysis.

## CLI reference

Global options:

| Flag | Default | Notes |
|---|---|---|
| `--port` | `COM8` | Serial device |
| `--baud` | `57600` | GridConnect standard |
| `--flow` | `none` | `none`\|`hardware`\|`software` |
| `--encoding` | `standard` | `standard`\|`merg` |
| `--our-node-id` | `05.01.01.01.A2.FE` | Distinct from Bowties app (`A2.FF`) so both can be on the bus |
| `--session-settle-ms` | `100` | Time to let `PeerSessionRegistry` spawn-watcher catch up after discovery |

`cdi` subcommand options:

| Flag | Default | Notes |
|---|---|---|
| `--node` | *required* | Target NodeID (dotted or contiguous hex) |
| `--iterations` | `5` | Back-to-back downloads |
| `--post-ack-delay-ms` | `100` | The pacing knob we're tuning |
| `--timeout-ms` | `5000` | Per-chunk read timeout |
| `--max-retries` | `3` | Resend-OK DR retry cap |
| `--discover-timeout-ms` | `500` | Discovery window before starting downloads |
| `--json` | `false` | Emit JSONL to stdout instead of the human table |

## Why this exists

Testing pacing changes through the Tauri UI is slow: change a constant,
`cargo build`, launch the app, click Connect, wait for discovery, click a
node, click Download CDI, look at logs, repeat. Each cycle is ~60 s and
each result is n=1. This tool lets you sweep 5 delay values × 20 iterations
in one script and get statistical confidence about the minimum safe pacing.

## Notes

- Uses a different `our_node_id` from the Bowties app by default (`A2.FE` vs
  the app's `A2.FF`) so you can run both simultaneously on the same bus
  during comparative testing.
- Cleanly shuts down the `PeerSessionRegistry` on exit so the COM port is
  released — safe to run in rapid succession.
- Not currently exercised by CI. Hardware-in-the-loop only.
