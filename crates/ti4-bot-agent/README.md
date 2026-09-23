# ti4-bot-agent

`ti4-bot-agent` is a headless WebSocket client that receives an actor's
pending choice from `ti4-server`, asks an external `ti4-advisor` service to
evaluate it, and submits one of the legal options.

## Run unit tests

From the repository root:

```bash
cargo test -p ti4-bot-agent
```

## Run the real end-to-end test

The opt-in E2E test starts a local `ti4-server`, a real `ti4-advisor` loaded
from `examples/reviewer/checkpoint-473312`, two bot agents, and one scripted
seat. It requires the pinned CPU libtorch runtime.

On Linux or WSL, install the runtime once if it is not already present:

```bash
scripts/install_libtorch_cpu_linux.sh
```

Then, from the repository root, configure the runtime and run the ignored E2E
test explicitly:

```bash
export LIBTORCH="$PWD/out/libtorch-2.9.1-cpu-linux"
export LIBTORCH_BYPASS_VERSION_CHECK=1
export LD_LIBRARY_PATH="$LIBTORCH/lib:${LD_LIBRARY_PATH:-}"

cargo test -p ti4-bot-agent --features real-e2e --test real_e2e -- --ignored
```

The test is ignored by default because it requires both libtorch and the
committed checkpoint. Supplying `--features real-e2e` compiles it; `-- --ignored`
executes it.

## Run a bot agent

Start `ti4-server` and `ti4-advisor` first, then run a bot with its assigned
seat token:

```bash
cargo run -p ti4-bot-agent -- \
  --server ws://127.0.0.1:8080 \
  --game game_123 \
  --seat p2 \
  --token <seat-token> \
  --advisor http://127.0.0.1:8081
```

Use `--sample-seed <u64>` to enable reproducible probability sampling.
Without it, the agent chooses the highest-probability option, preserving the
server choice's stable option order for ties.
