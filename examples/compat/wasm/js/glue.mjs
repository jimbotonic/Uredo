// §35's wasm entry through **wasm-bindgen's own generated glue**, which is how a JavaScript caller
// actually reaches a wasm-bindgen module.
//
// `run.mjs` beside this file calls the pre-bindgen artefact directly, which needs no tool but makes
// the test hand-write wasm-bindgen's ABI — the hash-suffixed export names, a `&str` as a (pointer,
// length) pair, a returned `String` through a caller-owned return area. That proved a JavaScript
// engine can reach the module and proved nothing about the glue. This runs the glue: `wasm-bindgen
// --target nodejs` generates a JS module, and the exports are called by their real names, with
// `Counter` as a class and `greet` taking and returning ordinary JavaScript strings.
//
// usage: node glue.mjs <path to the generated .js>

import { createRequire } from "node:module";

const path = process.argv[2];
if (!path) {
  console.error("usage: node glue.mjs <path to the generated .js>");
  process.exit(2);
}

// the nodejs target emits CommonJS
const wasm = createRequire(import.meta.url)(path);

const failures = [];
const check = (what, got, want) => {
  const ok = got === want;
  console.log(`${ok ? "ok  " : "FAIL"} ${what}: ${JSON.stringify(got)}${ok ? "" : ` (expected ${JSON.stringify(want)})`}`);
  if (!ok) failures.push(what);
};

// a `String`-returning function, called with a JavaScript string: the glue does the copying
check('greet("wasm")', wasm.greet("wasm"), "hello, wasm");
check("greet with a non-ASCII argument", wasm.greet("wörld 🌍"), "hello, wörld 🌍");

// the exported type is a class, its `@wasm_bindgen(constructor)` its constructor, and a method
// taking `self: inout` mutates the instance behind it
const counter = new wasm.Counter(1);
check("new Counter(1).bump(2)", counter.bump(2), 3);
check("the same instance bumped again", counter.bump(40), 43);

// `free` is the glue's, not Uredo's: a wasm object owns memory the JS garbage collector cannot see
counter.free();

console.log(failures.length === 0 ? "\nthe generated glue reaches every exported item" : `\n${failures.length} failure(s)`);
process.exit(failures.length === 0 ? 0 : 1);
