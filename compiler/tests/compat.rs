//! §35 compatibility fixtures: entries whose contract is exercised by a package under
//! `examples/compat/`. Each test builds and runs the real thing against the real crates.

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

static LOCK: Mutex<()> = Mutex::new(());
fn serial() -> MutexGuard<'static, ()> {
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn uredo(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_uredo")).args(args).current_dir(root()).output().expect("run uredo");
    (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
}

/// A field or item attribute must reach a derive macro unchanged, wherever the macro expects it:
/// serde's field attributes, thiserror's variant and inline field attributes, clap's `@command` and
/// `@arg`, tracing's `@instrument`. One entry of §35, four crates.
#[test]
fn forwarded_attributes_reach_their_derive_macros() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/attributes"]);
    assert!(ok, "{}{}", out, err);
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines,
        vec![
            // serde: `listen_on` renamed into `address`, `retries` defaulted, `cached` skipped
            "127.0.0.1:80 0 false",
            "{\"listen_on\":\"127.0.0.1:80\",\"retries\":0}",
            // thiserror: `Display` from `@error`, and `?` converting through the inline `@from`
            "no configuration at <empty>",
            "configuration is not valid json",
            // clap: `@command` and the two `@arg` fields, parsed from an explicit argument list
            "3 Some(\"ada\")",
            // tracing: the instrumented function runs
            "1",
        ],
        "{}{}",
        out,
        err
    );

    // the attributes must be in the generated Rust exactly as written, in every position
    let (ok, rust, err) = uredo(&["rust", "../examples/compat/attributes/src/main.ure"]);
    assert!(ok, "{}", err);
    for expected in [
        "#[serde(rename = \"listen_on\")]",
        "#[serde(default)]",
        "#[serde(skip)]",
        "#[error(\"no configuration at {0}\")]",
        "Parse(#[from] serde_json::Error)",
        "#[command(name = \"attributes\", about = \"the §35 forwarded-attribute fixture\")]",
        "#[arg(short, long, default_value_t = 1)]",
        "#[instrument(level = \"debug\", skip(config))]",
    ] {
        assert!(rust.contains(expected), "missing {:?} from the generated Rust:\n{}", expected, rust);
    }
}

/// Cargo's JSON messages and the program's own output share one pipe (§27). A program that prints
/// JSON must not lose the line — the bug this fixture found.
#[test]
fn program_output_that_looks_like_a_cargo_message_survives() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/attributes"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("{\"listen_on\""), "a JSON line of program output was swallowed:\n{}", out);
}

/// Operator traits and `Copy` on a foreign generic type (nalgebra). A foreign type is known-Copy
/// only where this crate says so, and `@copy use` names one instantiation (§10.1, D56).
#[test]
fn a_foreign_generic_copy_type_passes_by_value() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/numeric"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["2 3 4", "2 true", "6", "3"], "{}{}", out, err);

    let (ok, rust, err) = uredo(&["rust", "../examples/compat/numeric/src/main.ure"]);
    assert!(ok, "{}", err);
    // the declared instantiation passes by value; the `use` carries no type arguments
    assert!(rust.contains("fn combine(a: Vector3<f64>, b: Vector3<f64>)"), "{}", rust);
    assert!(rust.contains("fn transform(m: Matrix3<f64>, v: Vector3<f64>)"), "{}", rust);
    assert!(rust.contains("use nalgebra::Vector3;"), "{}", rust);
    assert!(rust.contains("__uredo_assert_copy::<Vector3<f64>>"), "{}", rust);
}

/// A `@copy use` of one instantiation must not make the whole family known-Copy: `Vector3<f64>` is
/// `Copy` in Rust and `Vector3<String>` is not, and the passing mode has to follow (D56).
#[test]
fn copy_use_declares_one_instantiation_not_a_family() {
    let src = "@copy use nalgebra::Vector3<f64>\n\nfn a(v: Vector3<f64>) -> f64:\n    v[0]\n\nfn b(v: Vector3<String>) -> usize:\n    v[0].len()\n\nfn main():\n    print(\"x\")\n";
    let out = uredo::compile(src, true);
    assert!(!out.has_errors(), "{:?}", out.diags);
    assert!(out.rust.contains("fn a(v: Vector3<f64>)"), "{}", out.rust);
    assert!(out.rust.contains("fn b(v: &Vector3<String>)"), "{}", out.rust);
}

/// A web framework's extractors and handlers (axum): each extractor is a pattern parameter (P7) and
/// each handler is passed to the router by value (P8). Server and client both run in this process.
#[test]
fn extractors_are_pattern_parameters_and_handlers_pass_by_value() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/web"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        vec!["hello, item 7", "{\"name\":\"hello/widget\",\"count\":2}"],
        "{}{}", out, err
    );
    let (ok, rust, err) = uredo(&["rust", "../examples/compat/web/src/main.ure"]);
    assert!(ok, "{}", err);
    // P7: the pattern is the parameter, passed by value, with no borrow inserted
    assert!(rust.contains("async fn show(State(db): State<Db>, Path(id): Path<u32>)"), "{}", rust);
    assert!(rust.contains("async fn add(State(db): State<Db>, Json(item): Json<Item>)"), "{}", rust);
    // P8: the handlers reach `get`/`post` verbatim
    assert!(rust.contains("get(show)") && rust.contains("post(add)"), "{}", rust);

    // `@rust(tokio::test)` follows the same rule as `tokio::main`
    let (ok, out, err) = uredo(&["test", "../examples/compat/web"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("test a_handler_answers_without_a_server ... ok"), "{}", out);
}

/// A builder-heavy client API (reqwest): every call consumes the builder and returns it, which is
/// what P8 does, and no `take` is written anywhere in the chain.
#[test]
fn a_consuming_builder_chain_needs_no_annotation() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/httpclient"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.trim(), "200 POST true", "{}{}", out, err);
    let (ok, rust, err) = uredo(&["rust", "../examples/compat/httpclient/src/main.ure"]);
    assert!(ok, "{}", err);
    assert!(rust.contains(".user_agent(\"uredo-compat\")"), "{}", rust);
    assert!(!rust.contains("take "), "the chain should need no ownership annotation:\n{}", rust);
}

/// §35's target-kind field: a package that is a library, a binary, an integration test and an
/// example at once. Each file under `tests/` and `examples/` is its own Cargo target, compiled
/// against the library by name.
#[test]
fn all_four_cargo_target_kinds_build_and_run() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["test", "../examples/compat/targets"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("test join_works ... ok"), "the library's own test\n{}", out);
    assert!(out.contains("test join_across_the_crate_boundary ... ok"), "the integration test\n{}", out);
    assert!(out.contains("test join_is_not_symmetric ... ok"), "{}", out);

    let (ok, out, err) = uredo(&["run", "../examples/compat/targets"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["bin-target", "quiet"], "{}", out);

    // the example is a target of the generated crate, run through Cargo as a user would
    let generated = root().join("../examples/compat/targets/target/uredo/Cargo.toml");
    let out = Command::new("cargo")
        .args(["run", "-q", "--manifest-path"])
        .arg(&generated)
        .args(["--example", "demo"])
        .output()
        .expect("cargo");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "example-target");

    // `src/bin/`: a file directly under it, and a directory with its own `main`. Cargo builds each
    // as its own crate root, so neither may be declared with `mod` — and the automatic declarations
    // must not write `src/bin/mod.rs`, which Cargo would build as a binary called `mod`.
    for (bin, expected) in [("aux", "aux-target"), ("grouped", "grouped-target")] {
        let out = Command::new("cargo")
            .args(["run", "-q", "--manifest-path"])
            .arg(&generated)
            .args(["--bin", bin])
            .output()
            .expect("cargo");
        assert!(out.status.success(), "{}: {}", bin, String::from_utf8_lossy(&out.stderr));
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), expected);
    }
    let gen_src = root().join("../examples/compat/targets/target/uredo/src");
    assert!(!gen_src.join("bin/mod.rs").exists(), "a `mod.rs` in `src/bin/` becomes a binary named `mod`");
    for root_file in ["lib.rs", "main.rs"] {
        let text = std::fs::read_to_string(gen_src.join(root_file)).expect(root_file);
        assert!(!text.contains("mod bin;"), "`src/bin` is not a module of the crate ({})", root_file);
    }
    // the module beside a binary's `main` is declared in that `main`, not in a `mod.rs`
    let grouped = std::fs::read_to_string(gen_src.join("bin/grouped/main.rs")).expect("grouped");
    assert!(grouped.contains("mod helper;"), "{}", grouped);
    assert!(!gen_src.join("bin/grouped/mod.rs").exists());

    // …and the bench, the one target kind that was lowered by the same rule and never run. On
    // stable there is no `#[bench]`, so it is a plain `main` with the harness switched off.
    assert!(gen_src.join("../benches/throughput.rs").exists(), "the bench was not lowered");
    let bench = Command::new("cargo")
        .args(["bench", "-q", "--manifest-path"])
        .arg(&generated)
        .args(["--bench", "throughput"])
        .output()
        .expect("cargo bench");
    let text = String::from_utf8_lossy(&bench.stdout).to_string() + &String::from_utf8_lossy(&bench.stderr);
    assert!(bench.status.success(), "the bench did not run:\n{}", text);
    assert!(text.contains("join: 20000 rounds in"), "{}", text);
    assert!(text.contains("last bench-target"), "the bench must reach the library it benchmarks:\n{}", text);
}

/// §35's mutation-rebuild field: an edit must take effect, and output for a file that is gone must
/// not survive. Runs on a copy, so the fixture in the repository is untouched.
#[test]
fn edits_rebuild_and_stale_output_is_removed() {
    let _guard = serial();
    let work = std::env::temp_dir().join(format!("uredo-mutation-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    copy_tree(&root().join("../examples/compat/targets"), &work);

    let run = |args: &[&str]| -> (bool, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_uredo")).args(args).current_dir(&work).output().expect("uredo");
        (out.status.success(), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string())
    };

    let (ok, out, err) = run(&["run"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("bin-target"), "{}", out);

    // 1. editing a `.ure` changes what runs
    let lib = work.join("src/lib.ure");
    let edited = std::fs::read_to_string(&lib).unwrap().replace("format(\"{a}-{b}\")", "format(\"{a}+{b}\")");
    std::fs::write(&lib, edited).unwrap();
    let (ok, out, err) = run(&["run"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("bin+target"), "the edit did not reach the build:\n{}", out);

    // 2. a feature changes which alternate is compiled
    let (ok, out, err) = run(&["run", "--features", "loud"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("QUIET"), "{}", out);

    // 3. a deleted source file leaves no generated file behind
    std::fs::write(work.join("src/extra.ure"), "pub fn extra() -> u32:\n    1\n").unwrap();
    assert!(run(&["check"]).0);
    assert!(work.join("target/uredo/src/extra.rs").exists(), "the new module was not generated");
    std::fs::remove_file(work.join("src/extra.ure")).unwrap();
    assert!(run(&["check"]).0);
    assert!(!work.join("target/uredo/src/extra.rs").exists(), "stale output survived a deletion");

    // 4. the same for an integration test
    assert!(work.join("target/uredo/tests/integration.rs").exists());
    std::fs::remove_file(work.join("tests/integration.ure")).unwrap();
    assert!(run(&["check"]).0);
    assert!(!work.join("target/uredo/tests/integration.rs").exists(), "a stale test target survived");

    // 5. a manifest edit reaches the generated manifest
    let manifest = work.join("Cargo.toml");
    let bumped = std::fs::read_to_string(&manifest).unwrap().replace("version = \"0.1.0\"", "version = \"0.2.0\"");
    std::fs::write(&manifest, bumped).unwrap();
    assert!(run(&["check"]).0);
    assert!(std::fs::read_to_string(work.join("target/uredo/Cargo.toml")).unwrap().contains("version = \"0.2.0\""));

    let _ = std::fs::remove_dir_all(&work);
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let name = e.file_name();
        if name == "target" {
            continue;
        }
        let dest = to.join(&name);
        if e.path().is_dir() {
            copy_tree(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}

/// An n-dimensional array crate: a macro body handed to rustc untouched, and a heap-backed foreign
/// type that is not known-Copy and so passes as a shared borrow with nothing written.
#[test]
fn a_slicing_macro_and_a_heap_backed_foreign_type() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/arrays"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["15 15", "2 6", "[1.0, 3.0]"], "{}{}", out, err);
    let (ok, rust, err) = uredo(&["rust", "../examples/compat/arrays/src/main.ure"]);
    assert!(ok, "{}", err);
    assert!(rust.contains("fn trace(m: &Array2<f64>)"), "not known-Copy, so borrowed:\n{}", rust);
    assert!(rust.contains("s![.., col]") && rust.contains("s![0, ..;2]"), "macro bodies verbatim:\n{}", rust);
}

/// A build script: plain Rust, copied beside the generated sources, and its output included from
/// Uredo through Rust's own `include!` (§4.6, §22.2).
#[test]
fn a_build_script_runs_and_survives_export() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/buildscript"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["8 49", "generated"], "{}", out);

    // §4.6: the exported package carries the script and builds with plain Cargo
    let export = std::env::temp_dir().join(format!("uredo-bs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&export);
    let (ok, out, err) = uredo(&["package", "../examples/compat/buildscript", "--out", export.to_str().unwrap()]);
    assert!(ok, "{}{}", out, err);
    assert!(export.join("build.rs").exists(), "the build script was not exported");
    let out = Command::new("cargo").args(["run", "-q"]).current_dir(&export).output().expect("cargo");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "8 49\ngenerated");
    let _ = std::fs::remove_dir_all(&export);
}

/// Consuming a procedural macro: an Uredo type carrying a derive that generates an `impl`, and the
/// name-value attribute the derive reads (§22.6, §23, D57).
#[test]
fn a_derive_macro_generates_an_impl_uredo_calls() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/procmacro"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["user.id, user.email", "[\"only\"]", "1"], "{}", out);
    let (ok, rust, err) = uredo(&["rust", "../examples/compat/procmacro/src/main.ure"]);
    assert!(ok, "{}", err);
    assert!(rust.contains("#[label = \"user.\"]"), "the name-value attribute:\n{}", rust);
}

/// wasm-bindgen: the attribute on functions, a type, its methods and a foreign block, and a real
/// artefact for a target the host does not run.
#[test]
fn a_wasm_target_builds_a_real_artefact() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["test", "../examples/compat/wasm"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("test describe_runs_on_the_host ... ok"), "{}", out);

    let (ok, out, err) = uredo(&["build", "../examples/compat/wasm", "--target", "wasm32-unknown-unknown"]);
    assert!(ok, "is the wasm32-unknown-unknown target installed?\n{}{}", out, err);
    let artefact = root().join("../examples/compat/wasm/target/wasm32-unknown-unknown/debug/compat_wasm.wasm");
    assert!(artefact.exists(), "no .wasm at {}", artefact.display());
    assert!(std::fs::metadata(&artefact).unwrap().len() > 1024, "the artefact is suspiciously small");

    // …and a JavaScript engine calls it. Until now the entry proved the attributes lower and the
    // code works on the host, which is not the same as a JS engine reaching it (§35's recorded gap).
    // The pre-bindgen module is called directly, so no `wasm-bindgen-cli` is needed; the calling
    // convention is hand-written in `js/run.mjs` and is wasm-bindgen's, not Uredo's.
    if Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        let script = root().join("../examples/compat/wasm/js/run.mjs");
        let run = Command::new("node").arg(&script).arg(&artefact).output().expect("node");
        let text = String::from_utf8_lossy(&run.stdout).to_string() + &String::from_utf8_lossy(&run.stderr);
        assert!(run.status.success(), "the module did not run in Node:\n{}", text);
        assert!(text.contains("ok   Counter::new(1) then bump(2): 3"), "{}", text);
        assert!(text.contains("ok   greet(\"wasm\"): \"hello, wasm\""), "{}", text);
    } else {
        eprintln!("SKIPPED: node is not installed, so the module was not run in a JavaScript engine");
        return;
    }

    // …and through wasm-bindgen's own generated glue, which is how a JavaScript caller actually
    // reaches such a module. The call above proved an engine can reach the artefact; this proves the
    // glue the toolchain generates works on what Uredo emitted, with `Counter` as a class and
    // `greet` taking and returning ordinary JavaScript strings.
    let cli = Command::new("wasm-bindgen").arg("--version").output();
    let version = cli.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    if !version.contains("0.2") {
        eprintln!("SKIPPED: no matching `wasm-bindgen` CLI, so the generated glue was not exercised");
        return;
    }
    let glue_dir = root().join("../examples/compat/wasm/target/glue");
    let generated = Command::new("wasm-bindgen")
        .args(["--target", "nodejs", "--out-dir"])
        .arg(&glue_dir)
        .arg(&artefact)
        .output()
        .expect("wasm-bindgen");
    assert!(generated.status.success(), "{}", String::from_utf8_lossy(&generated.stderr));

    let entry = glue_dir.join("compat_wasm.js");
    assert!(entry.exists(), "no glue at {}", entry.display());
    let run = Command::new("node")
        .arg(root().join("../examples/compat/wasm/js/glue.mjs"))
        .arg(entry.canonicalize().expect("glue path"))
        .output()
        .expect("node");
    let text = String::from_utf8_lossy(&run.stdout).to_string() + &String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "the glue did not reach the module:\n{}", text);
    assert!(text.contains("ok   greet(\"wasm\"): \"hello, wasm\""), "{}", text);
    assert!(text.contains("ok   new Counter(1).bump(2): 3"), "{}", text);
    // a string that is not ASCII crosses the boundary: the glue's own encoding, not ours
    assert!(text.contains("wörld"), "{}", text);
}

/// A database crate whose queries are checked while this crate compiles: the build script makes the
/// schema, so the macro has something to check against, and Uredo hands it the body untouched.
#[test]
fn a_compile_time_checked_query_is_really_checked() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/database"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(
        out.lines().collect::<Vec<_>>(),
        vec!["3 bolt", "[\"bolt\", \"nut\", \"washer\"]", "[\"bolt\", \"nut\", \"washer\"]"],
        "{}", out
    );

    // the negative case §35 asks for, at the entry's own level: a column that does not exist must
    // fail the build, with the database's own message, anchored at the Uredo line (§27)
    let src = root().join("../examples/compat/database/src/main.ure");
    let good = std::fs::read_to_string(&src).unwrap();
    let broken = good.replace("select name from widget", "select nmae from widget");
    assert_ne!(good, broken, "the fixture no longer contains the checked query");
    std::fs::write(&src, &broken).unwrap();
    let (ok, out, err) = uredo(&["check", "../examples/compat/database"]);
    std::fs::write(&src, &good).unwrap();
    assert!(!ok, "a misspelled column compiled:\n{}{}", out, err);
    assert!(err.contains("no such column: nmae"), "{}", err);
    assert!(err.contains("src/main.ure"), "the error was not anchored in Uredo source:\n{}", err);
}

/// `no_std`: §35 defers the entry, and this records how far the surface gets today.
#[test]
fn a_no_std_library_builds_and_excludes_std() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["test", "../examples/compat/nostd"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("test arithmetic_and_optionals_work_without_std ... ok"), "{}", out);

    let (ok, rust, err) = uredo(&["rust", "../examples/compat/nostd/src/lib.ure"]);
    assert!(ok, "{}", err);
    assert!(rust.contains("#![no_std]"), "{}", rust);
    let code = without_comments(&rust);
    assert!(code.contains("::core::option::Option") && !code.contains("::std::"), "std leaked into a no_std crate:\n{}", rust);
}

/// Generated Rust with its comments removed. An assertion about which paths the *code* reaches for
/// must not be tripped by a comment that names one, which is exactly what happened when the
/// `no_std` fixture grew a comment explaining that it does not use `::std::format!`.
fn without_comments(rust: &str) -> String {
    rust.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

/// §33.1's Phase 3 row: const generics, generic associated types and pinning, in native syntax.
/// The future is polled by a real runtime, not merely constructed.
#[test]
fn const_generics_gats_and_pinning_need_no_raw_rust() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["run", "../examples/compat/typelevel"]);
    assert!(ok, "{}{}", out, err);
    assert_eq!(out.lines().collect::<Vec<_>>(), vec!["12 4 2", "[1, 2, 3]", "0"], "{}{}", out, err);

    let (ok, rust, err) = uredo(&["rust", "../examples/compat/typelevel/src/main.ure"]);
    assert!(ok, "{}", err);
    // the phrase appears in the fixture's own prose, so look at the code rather than the comments
    let code: String = rust.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    assert!(!code.contains("rust {"), "the fixture must need no raw Rust:\n{}", code);
    assert!(rust.contains("struct Grid<const N: usize>") && rust.contains("impl<const N: usize> Grid<N>"), "{}", rust);
    assert!(rust.contains("type View<'a>: Debug") && rust.contains("Self: 'a"), "{}", rust);
    assert!(rust.contains("fn poll(self: Pin<&mut Self>"), "{}", rust);
}

/// §35's `no_std` entry, both halves. The non-allocating one has always worked; the allocating one
/// needs `alloc`, and needs Uredo's `format` intrinsic to know it is not `std`'s.
#[test]
fn no_std_reaches_the_allocating_half_through_alloc() {
    let _guard = serial();
    let (ok, out, err) = uredo(&["test", "../examples/compat/nostd"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("test arithmetic_and_optionals_work_without_std ... ok"), "{}", out);
    assert!(out.contains("test the_allocating_half_works_through_alloc ... ok"), "{}", out);

    // the intrinsic lowered to `alloc`'s macro, not `std`'s
    let generated = std::fs::read_to_string(root().join("../examples/compat/nostd/target/uredo/src/lib.rs")).expect("generated");
    let code = without_comments(&generated);
    assert!(code.contains("::alloc::format!"), "{}", generated);
    assert!(!code.contains("::std::"), "a `no_std` module must not reach for `std`:\n{}", generated);
}

/// …and the intrinsics that cannot work there are refused in Uredo terms rather than emitted and
/// left for rustc to reject on generated code.
#[test]
fn print_is_refused_in_a_no_std_module() {
    let out = uredo::compile("@!no_std\n\npub fn f() -> u8:\n    print(\"hi\")\n    1\n", false);
    let errors: Vec<&uredo::diag::Diag> = out.diags.iter().filter(|d| d.level == uredo::diag::Level::Error).collect();
    assert_eq!(errors.len(), 1, "{:?}", out.diags);
    assert!(errors[0].msg.contains("not available in a `@!no_std` module"), "{}", errors[0].msg);
    // `panic` still works: it is core's
    let out = uredo::compile("@!no_std\n\npub fn f() -> u8:\n    panic(\"no\")\n", false);
    assert!(!out.has_errors(), "{:?}", out.diags);
    assert!(out.rust.contains("::core::panic!"), "{}", out.rust);
}

/// The other half of §35's `no_std` entry: a target with no operating system under it. The host
/// build borrows `std`'s allocator and panic handler from the test harness, which proves the
/// language works and nothing about the target. This builds for `thumbv7em-none-eabihf`, where the
/// crate must supply both itself, and links a staticlib so rustc has to find them.
#[test]
fn no_std_builds_for_a_target_with_no_operating_system() {
    let _guard = serial();
    let target = "thumbv7em-none-eabihf";
    let installed = Command::new("rustup").args(["target", "list", "--installed"]).output();
    let have = installed.map(|o| String::from_utf8_lossy(&o.stdout).contains(target)).unwrap_or(false);
    if !have {
        eprintln!("SKIPPED: {} is not installed, so the bare-metal build was not exercised", target);
        return;
    }

    let (ok, out, err) = uredo(&["build", "../examples/compat/nostd", "--target", target]);
    assert!(ok, "{}{}", out, err);

    // an rlib defers everything; a staticlib makes rustc resolve the `#[panic_handler]` and the
    // `#[global_allocator]` the crate supplies. The generated crate is ordinary Cargo (§4.6), so
    // this is a plain `cargo rustc` against it.
    let manifest = root().join("../examples/compat/nostd/target/uredo/Cargo.toml");
    let linked = Command::new("cargo")
        .args(["rustc", "-q", "--manifest-path"])
        .arg(&manifest)
        .args(["--target", target, "--crate-type", "staticlib"])
        .output()
        .expect("cargo rustc");
    assert!(linked.status.success(), "{}", String::from_utf8_lossy(&linked.stderr));

    let archive = root().join("../examples/compat/nostd/target/uredo/target").join(target).join("debug/libcompat_nostd.a");
    assert!(archive.exists(), "no staticlib at {}", archive.display());

    // the allocator shim a `#[global_allocator]` generates is in the archive, so the crate's own
    // allocator is what `alloc` will call on that target
    let symbols = Command::new("nm").arg("--defined-only").arg(&archive).output();
    if let Ok(s) = symbols {
        let text = String::from_utf8_lossy(&s.stdout);
        assert!(text.contains("__rust_alloc"), "the crate's allocator is not in the archive");
    }

    // and the host build is unaffected: it still uses std's, and its tests still pass
    let (ok, out, err) = uredo(&["test", "../examples/compat/nostd"]);
    assert!(ok, "{}{}", out, err);
    assert!(out.contains("test the_allocating_half_works_through_alloc ... ok"), "{}", out);
}
