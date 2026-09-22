# ti4-advisor

`ti4-advisor` is a loopback-only HTTP service that evaluates one legal engine
choice against a supplied, viewer-redacted game snapshot. It loads an MLP
checkpoint once at startup and exposes `POST /evaluate`.

## Start the service

Run this command from the repository root:

```bash
cargo run -p ti4-advisor -- \
  --checkpoint examples/reviewer/checkpoint-473312 \
  --port 8081
```

Both arguments are optional. The defaults are
`examples/reviewer/checkpoint-473312` and port `8081`.

The service listens only on `127.0.0.1`; it is not reachable from other
machines. Stop it with `Ctrl-C`.

## Native runtime

The advisor links PyTorch/libtorch through `tch` and must be built and run with
a matching CPU libtorch tree.

On Linux or WSL, install the pinned Linux runtime once:

```bash
scripts/install_libtorch_cpu_linux.sh
export LIBTORCH="$PWD/out/libtorch-2.9.1-cpu-linux"
export LIBTORCH_BYPASS_VERSION_CHECK=1
export LD_LIBRARY_PATH="$LIBTORCH/lib:${LD_LIBRARY_PATH:-}"
```

The Linux runtime is checked against
`plans/artifacts/libtorch-2.9.1-cpu-linux.manifest.json` during the build.

On Windows, restore the complete pinned CPU tree at
`out/libtorch-2.9.1-cpu` and make its `lib` directory available on `PATH`:

```powershell
$env:LIBTORCH = "$PWD\out\libtorch-2.9.1-cpu"
$env:LIBTORCH_BYPASS_VERSION_CHECK = "1"
$env:PATH = "$env:LIBTORCH\lib;$env:PATH"
```

## Evaluate a choice

Send JSON with these fields to `POST http://127.0.0.1:8081/evaluate`:

| Field | Required | Meaning |
| --- | --- | --- |
| `state` | Yes | A viewer-redacted `GameState` snapshot. |
| `galaxy_layout` | Yes | The versioned layout used to reconstruct the galaxy. |
| `player` | Yes | The player ID that owns the choice. |
| `choice` | Yes | The pending engine `Choice`, including its legal options. |
| `temperature` | No | Positive finite sampling temperature; defaults to `0.25`. |

The request body is limited to 1 MiB. Unknown fields, invalid layouts,
inconsistent source sets, choices owned by another player, and invalid
temperatures return HTTP `400` with an `error` field.

A successful response contains the selected policy head, raw critic `value`,

```json
{
  "head": "tactical",
  "value": 0.42,
  "options": [
    {
      "option_id": "activate:19",
      "probability": 0.73,
      "logit": 1.18
    }
  ]
}
```

`value` is a raw critic output, not a win probability. Build requests from the
server's current snapshot and pending-choice messages rather than hand-writing
state or option IDs.
