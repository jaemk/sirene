// Runs `wasm-smoke` built for wasm32-unknown-unknown (WORKSP-11). A panic in it traps, which
// fails this script.
//
//   cargo build -p wasm-smoke --target wasm32-unknown-unknown
//   node scripts/wasm-smoke.mjs [target/wasm32-unknown-unknown/debug/wasm-smoke.wasm]
import { readFileSync } from "node:fs";

const path = process.argv[2] ?? "target/wasm32-unknown-unknown/debug/wasm-smoke.wasm";
const { instance } = await WebAssembly.instantiate(readFileSync(path), {});
const status = instance.exports.main(0, 0);
if (status !== 0) {
  console.error(`wasm_smoke exited with ${status}`);
  process.exit(1);
}
console.log("wasm_smoke rendered every example");
