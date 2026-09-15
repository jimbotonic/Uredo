// §35's wasm entry, actually run in a JavaScript engine.
//
// The fixture's recorded gap was that nothing called the module from JavaScript: it compiled,
// linked and passed a host test, which proves the attributes lower and the code works, but not
// that a JS engine can reach it. This calls it.
//
// It uses the *pre-bindgen* module — the artefact `uredo build --target wasm32-unknown-unknown`
// produces — rather than the glue `wasm-bindgen-cli` would generate, so the test needs no tool
// beyond Node. The cost is that the calling convention is hand-written here instead of generated:
// exported names carry wasm-bindgen's hash suffix, a `&str` argument is a (pointer, length) pair
// in the module's own memory, and a returned `String` arrives through the shadow stack. That is
// wasm-bindgen's ABI, not Uredo's, and this file depends on it — if a wasm-bindgen release changes
// it, this breaks and the fixture says so, which is the honest failure mode for a compatibility
// entry.
//
// usage: node run.mjs <path to .wasm>

import { readFileSync } from "node:fs";

const path = process.argv[2];
if (!path) {
  console.error("usage: node run.mjs <path to .wasm>");
  process.exit(2);
}

const module_ = await WebAssembly.compile(readFileSync(path));

// the extern block's `log` and wasm-bindgen's own hooks: the module only needs them to exist
const imports = {};
for (const { module: m, name, kind } of WebAssembly.Module.imports(module_)) {
  if (kind !== "function") continue;
  imports[m] ??= {};
  imports[m][name] = () => 0;
}
const { exports } = await WebAssembly.instantiate(module_, imports);

// wasm-bindgen suffixes every export with a hash of the crate, so look them up by prefix
const byPrefix = (p) => {
  const name = Object.keys(exports).find((k) => k.startsWith(p) && !k.startsWith("__"));
  if (!name) throw new Error(`no export starting with ${p}; found ${Object.keys(exports).join(", ")}`);
  return exports[name];
};

const failures = [];
const check = (what, got, want) => {
  const ok = got === want;
  console.log(`${ok ? "ok  " : "FAIL"} ${what}: ${JSON.stringify(got)}${ok ? "" : ` (expected ${JSON.stringify(want)})`}`);
  if (!ok) failures.push(what);
};

// --- the numeric surface: an exported constructor and an `inout` method, called from JavaScript
const counterNew = byPrefix("counter_new");
const counterBump = byPrefix("counter_bump");
const counter = counterNew(1);
check("Counter::new(1) then bump(2)", counterBump(counter, 2), 3);
check("the same counter bumped again", counterBump(counter, 40), 43);

// --- the string surface: `greet(name: str) -> String` across the boundary
const memory = exports.memory;
const malloc = exports.__wbindgen_malloc;
const free = exports.__wbindgen_free;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

const write = (s) => {
  const bytes = encoder.encode(s);
  const ptr = malloc(bytes.length, 1);
  new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
  return [ptr, bytes.length];
};

// A returned `String` does not fit in a wasm return value, so it comes back through a caller-owned
// return area: the caller passes a pointer as a hidden first argument and reads (ptr, len) out of
// it afterwards. Generated glue takes that area from a shadow stack; this build exports no such
// helper, so the area is an ordinary allocation.
const greet = byPrefix("greet");
const [namePtr, nameLen] = write("wasm");
const ret = malloc(8, 4);
greet(ret, namePtr, nameLen);
const view = new DataView(memory.buffer);
const ptr = view.getUint32(ret, true);
const len = view.getUint32(ret + 4, true);
const text = decoder.decode(new Uint8Array(memory.buffer, ptr, len));
free(ptr, len, 1);
free(ret, 8, 4);
check("greet(\"wasm\")", text, "hello, wasm");

console.log(failures.length === 0 ? "\nall calls from JavaScript agree with the host test" : `\n${failures.length} failure(s)`);
process.exit(failures.length === 0 ? 0 : 1);
