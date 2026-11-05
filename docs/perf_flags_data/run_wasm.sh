#!/bin/bash
# wasm variants for docs/PERF_FLAGS.md: cold build of ti4-wasm (no wasm-opt: binaryen is not installed here), file size (raw, gzip -9, brotli -11),
# and node run time of crates/ti4-wasm/js/run.mjs (3 repetitions, same call and same state hash expected).
#   run_wasm.sh <name> <cargo-profile>
set -u
NAME=$1; PROF=$2
WT=/root/TI4-Bot-Dev/.claude/worktrees/core-perf-flags
OUTD=/root/deploytmp/flags-data; TD=/root/deploytmp/flags/$NAME
mkdir -p $OUTD/logs
exec 8>/root/.ti4-heavy.lock.2; flock 8
cd $WT; export CARGO_TARGET_DIR=$TD CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0
rm -rf $TD
/usr/bin/time -f "%e %U %S %M" -o $OUTD/logs/$NAME.cold.time nice -n 15 cargo build -p ti4-wasm --target wasm32-unknown-unknown --profile $PROF --config docs/perf_flags_data/profiles.toml > $OUTD/logs/$NAME.cold.out 2>&1
RC=$?; set -- $(tail -1 $OUTD/logs/$NAME.cold.time)
F=$TD/wasm32-unknown-unknown/$PROF/ti4_wasm.wasm
SZ=$(node -e '
const {readFileSync}=require("fs"),z=require("zlib");const b=readFileSync(process.argv[1]);
console.log(JSON.stringify({raw:b.length,gzip:z.gzipSync(b,{level:9}).length,brotli:z.brotliCompressSync(b,{params:{[z.constants.BROTLI_PARAM_QUALITY]:11,[z.constants.BROTLI_PARAM_SIZE_HINT]:b.length}}).length}))' $F 2>/dev/null || echo '{}')
echo "{\"variant\":\"$NAME\",\"kind\":\"wasm_build\",\"rc\":$RC,\"wall\":$1,\"user\":$2,\"sys\":$3,\"size\":$SZ}" >> $OUTD/results.jsonl
if [ $RC -eq 0 ]; then
  for rep in 1 2 3; do
    /usr/bin/time -f "%e %U %S %M" -o $OUTD/logs/$NAME.node.$rep.time node crates/ti4-wasm/js/run.mjs $F 3 2000 4 > $OUTD/logs/$NAME.node.$rep.out 2> $OUTD/logs/$NAME.node.$rep.err
    t=($(tail -1 $OUTD/logs/$NAME.node.$rep.time)); ms=$(grep -o '^[0-9]* ms' $OUTD/logs/$NAME.node.$rep.err | cut -d' ' -f1)
    echo "{\"variant\":\"$NAME\",\"kind\":\"wasm_run\",\"rep\":$rep,\"wall\":${t[0]},\"user\":${t[1]},\"rss_kb\":${t[3]},\"ms\":${ms:-0},\"out\":\"$(head -1 $OUTD/logs/$NAME.node.$rep.out)\"}" >> $OUTD/results.jsonl
  done
fi
rm -rf $TD
