#!/bin/bash
cd "$(dirname "$0")"
for v in wasm-release w-os w-o2 w-o3 w-thin w-unwind; do ./run_wasm.sh $v $v; done
./run_server.sh gate gate
./run_server.sh release release
./run_server.sh r-abort r-abort
echo ALLDONE2 >> /root/deploytmp/flags-data/done
