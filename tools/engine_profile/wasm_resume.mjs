// Time the browser host's resume (a reload: the save is played again from its seed and answers)
// in a real WebAssembly instance, without JSPI: the `ask` import answers synchronously
// (CONTINUE at each `progress`, LEAVE at the first question after the replay).
//
//   node tools/engine_profile/wasm_resume.mjs <ti4_wasm.wasm> <report-or-save.json> [repeats]
//
// The file may be a feedback report (`context.local.save`) or a save blob, as for
// `cargo run -p ti4-wasm --example replay_save`.
import { readFileSync } from "node:fs";

const [file, savePath, repeats = "3"] = process.argv.slice(2);
const raw = JSON.parse(readFileSync(savePath, "utf8"));
const save = raw?.context?.local?.save ?? raw?.local?.save ?? raw?.save ?? raw;
const module = new WebAssembly.Module(readFileSync(file));
const encoder = new TextEncoder();
const decoder = new TextDecoder();
for (let i = 0; i < Number(repeats); i++) {
  let exports;
  let progress = 0;
  let asked = null;
  const ask = () => {
    const text = decoder.decode(
      new Uint8Array(exports.memory.buffer, exports.ti4_host_out_ptr(), exports.ti4_host_out_len()),
    );
    const envelope = JSON.parse(text);
    if (envelope.kind === "progress") {
      progress++;
      return -5; // CONTINUE
    }
    if (envelope.kind === "ask") asked = envelope;
    return -1; // LEAVE
  };
  ({ exports } = new WebAssembly.Instance(module, { host: { ask } }));
  exports.ti4_host_set_time(Date.now(), performance.now());
  const bytes = encoder.encode(JSON.stringify({ resume: save }));
  const pointer = exports.ti4_host_in(bytes.length);
  new Uint8Array(exports.memory.buffer, pointer, bytes.length).set(bytes);
  const started = performance.now();
  const cpu0 = process.cpuUsage();
  const status = exports.ti4_host_start(bytes.length);
  const cpu = process.cpuUsage(cpu0);
  const ms = performance.now() - started;
  const check = asked?.resume_check ?? null;
  console.log(
    `resume ${i + 1}: status ${status} in ${ms.toFixed(0)} ms (cpu ${((cpu.user + cpu.system) / 1000).toFixed(0)} ms), ` +
      `${Array.isArray(save.decisions) ? save.decisions.length : save.decisions} decisions, progress ${progress}, ` +
      `check ${JSON.stringify(check)?.slice(0, 120)}`,
  );
}
