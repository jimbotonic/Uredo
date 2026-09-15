//! `uredo` command-line tool (§28.1): `rust`, `check`, `build`, `run`, `test`, `package`, `explain`.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, exit};
use uredo::api::{ApiEntry, Manifest};
use uredo::provenance::{Elab, Map};

fn usage() -> ! {
    eprintln!(
        "usage:
  uredo new <path> [--lib] [--name n]     create a package (§4.1): Cargo.toml, src/main.ure or lib.ure
  uredo init [dir] [--lib] [--name n]      the same, in a directory that already exists
  uredo rust <file.ure> [--raw] [--map]   print the generated Rust (raw: before rustfmt; map: provenance)
  uredo check [dir]                       lower the package and run `cargo check`
  uredo check --api [dir] [--update-api]  compare the public API with uredo-api.json (§4.4)
  uredo build [dir] [--release]           lower the package and run `cargo build`
  uredo run [dir] [-- args…]              lower the package and run `cargo run`
  uredo test [dir]                        lower the package and run `cargo test`
  uredo doc [dir] [--open]                lower the package and run `cargo doc` (§26.4)
  uredo package [dir] [--out path]        export a self-contained Cargo package (§4.6)
  uredo publish [dir] [--dry-run] [--allow-api-change] [cargo flags…]
                                          export, check the recorded API (§4.4), then `cargo publish`
  uredo explain <file.ure>:<line>         what Uredo elaborated on that line (§26.2)
  uredo fmt [--check] <files or dirs>     canonical formatting (§29); --check reports without writing
  uredo lsp                               a language server on stdio (§28.1)
  uredo report [dir] [--out path]         a bug bundle for a lowering defect (§5.4)
  uredo fix [--dry-run] [files or dirs]   apply the fixes that leave the generated Rust unchanged
  uredo lint [--deny] [--allow <lint>] [files or dirs]
                                          idiom findings (§28.1); --deny exits 1 when any fires
  uredo lint --measure [files or dirs]    the D52 reversal counts (§38), not findings
The generated crate is kept under <dir>/target/uredo with its provenance maps."
    );
    exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    match args[0].as_str() {
        "new" => cmd_new(&args[1..], false),
        "init" => cmd_new(&args[1..], true),
        "rust" => cmd_rust(&args[1..]),
        "check" | "build" | "run" | "test" | "doc" => cmd_cargo(&args[0], &args[1..]),
        "package" => cmd_package(&args[1..]),
        "publish" => cmd_publish(&args[1..]),
        "explain" => cmd_explain(&args[1..]),
        "fmt" => cmd_fmt(&args[1..]),
        "lint" => cmd_lint(&args[1..]),
        "fix" => cmd_fix(&args[1..]),
        "lsp" => exit(uredo::lsp::serve()),
        "report" => cmd_report(&args[1..]),
        _ => usage(),
    }
}

fn read(file: &str) -> String {
    fs::read_to_string(file).unwrap_or_else(|e| {
        eprintln!("error: cannot read {}: {}", file, e);
        exit(1)
    })
}

fn cmd_rust(args: &[String]) {
    let raw = args.iter().any(|a| a == "--raw");
    let show_map = args.iter().any(|a| a == "--map");
    let file = args.iter().find(|a| !a.starts_with("--")).unwrap_or_else(|| usage());
    let src = read(file);
    if args.iter().any(|a| a == "--tokens") {
        let (toks, diags) = uredo::lexer::lex(&src);
        for t in &toks {
            println!("{}:{} {:?}", t.line, t.col, t.tok);
        }
        for d in &diags {
            eprint!("{}", d.render(file, &src));
        }
        return;
    }
    let out = uredo::compile(&src, !raw);
    for d in &out.diags {
        eprint!("{}", d.render(file, &src));
    }
    if out.has_errors() {
        exit(1);
    }
    if show_map {
        for (i, l) in out.rust.lines().enumerate() {
            let ure = out.map.lines.get(i).copied().unwrap_or(0);
            println!("{:>4} {:>4} | {}", i + 1, if ure == 0 { "-".to_string() } else { ure.to_string() }, l);
        }
        return;
    }
    print!("{}", out.rust);
}

// ----- packages -----

struct Lowered {
    out_dir: PathBuf,
    /// generated file (relative, `src/main.rs`) -> provenance
    maps: BTreeMap<String, Map>,
    api: Manifest,
}

fn package_name(manifest: &str) -> String {
    let mut in_package = false;
    for l in manifest.lines() {
        let t = l.trim();
        if t.starts_with('[') {
            in_package = t == "[package]";
        }
        if in_package && t.starts_with("name") {
            if let Some(v) = t.split('=').nth(1) {
                return v.trim().trim_matches('"').to_string();
            }
        }
    }
    "package".to_string()
}

/// Lowers a package: `src/**/*.ure` -> `<out>/src/**/*.rs`, `.rs` files and `Cargo.toml`
/// copied alongside, automatic `mod` declarations added (§20.3), provenance written under
/// `<out>/provenance/`.
/// Rewrites relative `path = "…"` **dependencies** so they still resolve from the generated crate,
/// which does not sit where the authoring package does. A `path` that is not a dependency's — a
/// target's own source path, say — is left alone, and so is an absolute one.
fn rewrite_path_deps(manifest: &str, from: &Path, to: &Path) -> String {
    if !manifest.contains("path") {
        return manifest.to_string();
    }
    let base = std::fs::canonicalize(from).unwrap_or_else(|_| from.to_path_buf());
    let mut out = String::new();
    let mut in_deps = false;
    for line in manifest.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            let header = trimmed.trim_start_matches('[').split(']').next().unwrap_or("");
            in_deps = header.ends_with("dependencies");
        }
        if !in_deps {
            out.push_str(line);
            continue;
        }
        // `name = { path = "…" }`, or `path = "…"` inside a dependency's own table
        if let Some(i) = line.find("path = \"") {
            let rest = &line[i + 8..];
            if let Some(j) = rest.find('"') {
                let value = &rest[..j];
                let p = Path::new(value);
                if p.is_relative() && !trimmed.starts_with('#') {
                    let absolute = base.join(p);
                    let shown = absolute.to_string_lossy().replace('\\', "/");
                    out.push_str(&format!("{}path = \"{}\"{}", &line[..i], shown, &rest[j + 1..]));
                    continue;
                }
            }
        }
        out.push_str(line);
    }
    let _ = to;
    out
}

fn lower_crate(dir: &Path, out_dir: &Path, own_workspace: bool, with_provenance: bool) -> Result<Lowered, usize> {
    let src_dir = dir.join("src");
    let out_src = out_dir.join("src");
    let _ = fs::remove_dir_all(&out_src);
    let _ = fs::remove_dir_all(out_dir.join("provenance"));
    fs::create_dir_all(&out_src).expect("create the generated src directory");
    let mut manifest = fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_else(|e| {
        eprintln!("error: no Cargo.toml in {}: {}", dir.display(), e);
        exit(1)
    });
    if own_workspace && !manifest.contains("[workspace]") {
        // stops Cargo from walking up into the authoring manifest, whose targets are `.ure`
        manifest = format!("{}\n[workspace]\n", manifest.trim_end());
    }
    // A relative `path` dependency is written against the authoring directory; the generated crate
    // sits somewhere else, so every relative path is rewritten to point back at the original (§4.6).
    manifest = rewrite_path_deps(&manifest, dir, out_dir);
    if !own_workspace && !manifest.contains("rust-version") {
        // exported packages declare the generated code's MSRV (§28.2): edition 2024
        manifest = manifest.replacen("[package]\n", "[package]\nrust-version = \"1.85\"\n", 1);
    }
    fs::write(out_dir.join("Cargo.toml"), manifest).expect("write Cargo.toml");
    for extra in ["build.rs", "README.md", "LICENSE", "LICENSE-MIT", "LICENSE-APACHE"] {
        let p = dir.join(extra);
        if p.is_file() {
            let _ = fs::copy(&p, out_dir.join(extra));
        }
    }

    let mut errors = 0;
    // every file, not only the two source extensions: an asset beside the source — the target of an
    // `include_str!`, a `.pest` grammar, a fixture — has to reach the generated crate, or the
    // package `uredo package` exports is not the package that was written.
    let mut files: Vec<PathBuf> = Vec::new();
    collect_any(&src_dir, &mut files);
    files.sort();
    let crate_name = package_name(&fs::read_to_string(dir.join("Cargo.toml")).unwrap_or_default()).replace('-', "_");
    // first pass: index every `.ure` file's declarations under its module path (§20.3)
    let mut index = uredo::lower::CrateIndex::default();
    for f in &files {
        let rel = f.strip_prefix(&src_dir).unwrap();
        if rel.extension().map(|e| e != "ure").unwrap_or(true) {
            continue;
        }
        let (module_path, _) = module_of(rel);
        if let Ok(src) = fs::read_to_string(f) {
            index.merge(uredo::index_source(&src, &module_path));
        }
    }
    let mut maps: BTreeMap<String, Map> = BTreeMap::new();
    let mut api: Vec<ApiEntry> = Vec::new();
    let mut modules: BTreeMap<PathBuf, Vec<String>> = Default::default();
    for f in &files {
        let rel = f.strip_prefix(&src_dir).unwrap();
        let stem = rel.file_stem().unwrap().to_string_lossy().to_string();
        let ext = rel.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
        let (module_path, declared_in) = module_of(rel);
        if ext == "ure" || ext == "rs" {
            if let (Some(parent), Some(name)) = (declared_in, module_path.rsplit("::").next()) {
                modules.entry(parent).or_default().push(name.to_string());
            }
        }
        let dest = out_src.join(rel).with_extension("rs");
        fs::create_dir_all(dest.parent().unwrap()).ok();
        if ext == "ure" {
            let src = fs::read_to_string(f).unwrap();
            let out = uredo::compile_in_crate(&src, true, &index, &crate_name, &module_path);
            let shown = format!("src/{}", rel.display());
            for d in &out.diags {
                eprint!("{}", d.render(&shown, &src));
            }
            if out.has_errors() {
                errors += 1;
                continue;
            }
            fs::write(&dest, &out.rust).expect("write generated file");
            if stem != "main" {
                api.extend(out.api);
            }
            let mut map = out.map;
            map.ure = shown;
            map.rs = format!("src/{}", rel.with_extension("rs").display());
            maps.insert(map.rs.clone(), map);
        } else {
            fs::copy(f, out_src.join(rel)).expect("copy the file beside the source");
        }
    }
    // Auxiliary Cargo targets (§35): every file directly under `tests/`, `examples/` or `benches/`
    // is its own target, not a module of the crate, so no `mod` declaration is added and the module
    // path is empty; each sees the crate's declarations through `use <crate name>::…` (§10.3).
    for aux in ["tests", "examples", "benches"] {
        let aux_dir = dir.join(aux);
        if !aux_dir.is_dir() {
            continue;
        }
        let out_aux = out_dir.join(aux);
        let _ = fs::remove_dir_all(&out_aux);
        let mut aux_files: Vec<PathBuf> = Vec::new();
        collect_any(&aux_dir, &mut aux_files);
        aux_files.sort();
        for f in &aux_files {
            let rel = f.strip_prefix(&aux_dir).unwrap();
            let dest = out_aux.join(rel).with_extension("rs");
            fs::create_dir_all(dest.parent().unwrap()).expect("create the generated target directory");
            let ext = rel.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
            if ext == "ure" {
                let src = fs::read_to_string(f).unwrap();
                let out = uredo::compile_in_crate(&src, true, &index, &crate_name, "");
                let shown = format!("{}/{}", aux, rel.display());
                for d in &out.diags {
                    eprint!("{}", d.render(&shown, &src));
                }
                if out.has_errors() {
                    errors += 1;
                    continue;
                }
                fs::write(&dest, &out.rust).expect("write generated file");
                let mut map = out.map;
                map.ure = shown;
                map.rs = format!("{}/{}", aux, rel.with_extension("rs").display());
                maps.insert(map.rs.clone(), map);
            } else {
                fs::copy(f, out_aux.join(rel)).expect("copy the file beside the source");
            }
        }
    }
    if errors > 0 {
        return Err(errors);
    }
    // Automatic module declarations (§20.3), after the inner docs and attributes.
    for (parent, names) in &modules {
        let parent_rel = if parent.as_os_str().is_empty() {
            // with both roots present the modules belong to the library; the binary uses it
            if out_src.join("lib.rs").exists() { PathBuf::from("lib.rs") } else { PathBuf::from("main.rs") }
        } else if parent.iter().next().map(|s| s == "bin").unwrap_or(false) {
            // `src/bin/x/`: the binary's root is `main.rs`, and there is no module above it
            parent.join("main.rs")
        } else {
            let f = parent.with_extension("rs");
            if out_src.join(&f).exists() { f } else { parent.join("mod.rs") }
        };
        let parent_file = out_src.join(&parent_rel);
        let content = fs::read_to_string(&parent_file).unwrap_or_default();
        let mut decls: Vec<String> = Vec::new();
        for n in names {
            if !content.contains(&format!("mod {};", n)) && !content.contains(&format!("pub mod {};", n)) {
                decls.push(format!("mod {};", n));
            }
        }
        if decls.is_empty() {
            continue;
        }
        let lines: Vec<&str> = content.lines().collect();
        let head_len = lines.iter().take_while(|l| l.starts_with("//!") || l.starts_with("#![")).count();
        let mut new_lines: Vec<String> = lines[..head_len].iter().map(|s| s.to_string()).collect();
        let mut inserted: isize = 0;
        if head_len > 0 {
            new_lines.push(String::new());
            inserted += 1;
        }
        new_lines.extend(decls.iter().cloned());
        inserted += decls.len() as isize;
        let mut rest = &lines[head_len..];
        new_lines.push(String::new());
        inserted += 1;
        while rest.first().map(|l| l.is_empty()).unwrap_or(false) {
            rest = &rest[1..];
            inserted -= 1;
        }
        new_lines.extend(rest.iter().map(|s| s.to_string()));
        fs::write(&parent_file, format!("{}\n", new_lines.join("\n"))).expect("write module declarations");
        let key = format!("src/{}", parent_rel.display());
        if let Some(m) = maps.get_mut(&key) {
            // keep the line map aligned with the inserted (or removed) lines
            if inserted >= 0 {
                for _ in 0..inserted {
                    m.lines.insert(head_len.min(m.lines.len()), 0);
                }
            } else {
                for _ in 0..(-inserted) {
                    if head_len < m.lines.len() {
                        m.lines.remove(head_len);
                    }
                }
            }
        }
    }
    if with_provenance {
        let pdir = out_dir.join("provenance");
        fs::create_dir_all(&pdir).ok();
        for (rs, m) in &maps {
            let name = rs.replace('/', "__").replace(".rs", ".json");
            fs::write(pdir.join(name), serde_json::to_string_pretty(m).unwrap()).expect("write provenance");
        }
    }
    let manifest = Manifest::new(&crate_name, api);
    if with_provenance {
        fs::write(out_dir.join("api.json"), serde_json::to_string_pretty(&manifest).unwrap()).ok();
    }
    Ok(Lowered { out_dir: out_dir.to_path_buf(), maps, api: manifest })
}

/// `uredo check --api`: the public API manifest against the reviewed baseline (§4.4).
/// What the recorded public API says about this build: the same comparison `check --api` prints,
/// as a value, so `publish` can refuse on it without duplicating the rule (§4.4, D14).
enum ApiState {
    /// no `uredo-api.json` yet
    Unrecorded,
    Unchanged(usize),
    Changed(String),
}

fn api_state(dir: &Path, lowered: &Lowered) -> ApiState {
    let baseline_path = dir.join("uredo-api.json");
    let Ok(text) = fs::read_to_string(&baseline_path) else {
        return ApiState::Unrecorded;
    };
    let Ok(baseline) = serde_json::from_str::<Manifest>(&text) else {
        return ApiState::Changed(format!("{} is not a valid manifest\n", baseline_path.display()));
    };
    let d = uredo::api::diff(&baseline, &lowered.api);
    if d.is_empty() {
        ApiState::Unchanged(lowered.api.items.len())
    } else {
        ApiState::Changed(d.render(&baseline_path.display().to_string()))
    }
}

fn check_api(dir: &Path, lowered: &Lowered, update: bool) -> ! {
    let baseline_path = dir.join("uredo-api.json");
    let new = &lowered.api;
    let text = serde_json::to_string_pretty(new).unwrap() + "\n";
    let baseline = match fs::read_to_string(&baseline_path) {
        Ok(t) => match serde_json::from_str::<Manifest>(&t) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("error: {} is not a valid manifest: {}", baseline_path.display(), e);
                exit(1);
            }
        },
        Err(_) => {
            fs::write(&baseline_path, &text).expect("write the API manifest");
            println!("API manifest created: {} ({} public items); review and commit it", baseline_path.display(), new.items.len());
            exit(0);
        }
    };
    let d = uredo::api::diff(&baseline, new);
    if d.is_empty() {
        println!("public API unchanged: {} items match {}", new.items.len(), baseline_path.display());
        exit(0);
    }
    if update {
        fs::write(&baseline_path, &text).expect("write the API manifest");
        print!("{}", d.render(&baseline_path.display().to_string()));
        println!("API manifest updated: {}", baseline_path.display());
        exit(0);
    }
    eprint!("{}", d.render(&baseline_path.display().to_string()));
    eprintln!("error: unreviewed public API change (§4.4); run `uredo check --api --update-api` after review");
    exit(1);
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(&p, out);
            } else if let Some(ext) = p.extension() {
                if ext == "ure" || ext == "rs" {
                    out.push(p);
                }
            }
        }
    }
}

fn split_args(args: &[String]) -> (PathBuf, Vec<String>, Vec<String>) {
    let dashdash = args.iter().position(|a| a == "--");
    let own: Vec<String> = match dashdash {
        Some(i) => args[..i].to_vec(),
        None => args.to_vec(),
    };
    let passthrough: Vec<String> = match dashdash {
        Some(i) => args[i + 1..].to_vec(),
        None => vec![],
    };
    let mut dir = PathBuf::from(".");
    let mut flags: Vec<String> = Vec::new();
    let mut i = 0;
    // flags whose value is the next argument (cargo's and ours)
    const VALUED: &[&str] = &["--out", "--features", "--target", "--bin", "--example", "--package", "-p", "--profile", "--jobs", "-j", "--config", "--target-dir"];
    while i < own.len() {
        if VALUED.contains(&own[i].as_str()) {
            flags.push(own[i].clone());
            if let Some(v) = own.get(i + 1) {
                flags.push(v.clone());
            }
            i += 2;
            continue;
        }
        if own[i].starts_with("--") {
            flags.push(own[i].clone());
        } else {
            dir = PathBuf::from(&own[i]);
        }
        i += 1;
    }
    (dir, flags, passthrough)
}

/// Cargo's `src/bin/` holds crate roots rather than modules (§20.3): `src/bin/x.ure` and
/// `src/bin/x/main.ure` are each their own binary, so neither is declared with `mod` and neither
/// sits in the crate's module tree. A file *beside* `src/bin/x/main.ure` is a module of that
/// binary, named relative to it. Without this the automatic declarations wrote `src/bin/mod.rs`,
/// which Cargo then built as a binary called `mod`.
fn bin_root(rel: &Path) -> bool {
    let segs: Vec<String> = rel.iter().map(|s| s.to_string_lossy().to_string()).collect();
    let stem = rel.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    segs.first().map(|s| s == "bin").unwrap_or(false) && (segs.len() == 2 || (segs.len() == 3 && stem == "main"))
}

/// The module path of a file under `src/`, and the directory whose root file declares it —
/// `None` when nothing declares it, which is every Cargo target root.
/// The package a source file belongs to: the nearest ancestor with a `Cargo.toml` and a `src/`
/// that contains the file.
///
/// Per-file commands used to compile each file with an empty index, which means none of them
/// could see anything the rest of the crate declares — including a crate-root
/// `@!default_error`, so `uredo lint` rejected a bare `throws` that `uredo check` accepts.
fn package_of(file: &Path) -> Option<(PathBuf, String)> {
    let abs = file.canonicalize().ok()?;
    let mut dir = abs.parent()?.to_path_buf();
    loop {
        if dir.join("Cargo.toml").is_file() {
            let src = dir.join("src");
            if abs.starts_with(&src) {
                let rel = abs.strip_prefix(&src).ok()?.to_path_buf();
                return Some((dir, module_of(&rel).0));
            }
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// Every `.ure` declaration in a package, indexed under its module path (§20.3).
fn index_package(dir: &Path) -> uredo::lower::CrateIndex {
    let src_dir = dir.join("src");
    let mut files = Vec::new();
    collect(&src_dir, &mut files);
    files.sort();
    let mut index = uredo::lower::CrateIndex::default();
    for f in &files {
        if f.extension().map(|e| e != "ure").unwrap_or(true) {
            continue;
        }
        if let (Ok(rel), Ok(text)) = (f.strip_prefix(&src_dir), fs::read_to_string(f)) {
            index.merge(uredo::index_source(&text, &module_of(rel).0));
        }
    }
    index
}

fn module_of(rel: &Path) -> (String, Option<PathBuf>) {
    let stem = rel.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let parent = rel.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let mut segs: Vec<String> = parent.iter().map(|s| s.to_string_lossy().to_string()).collect();
    let crate_root = segs.is_empty() && (stem == "main" || stem == "lib");
    if crate_root || bin_root(rel) {
        return (String::new(), None);
    }
    // inside `src/bin/x/`, names are relative to that binary's own root
    if segs.first().map(|s| s == "bin").unwrap_or(false) && segs.len() >= 2 {
        segs.drain(..2);
    }
    if stem != "mod" {
        segs.push(stem);
    }
    (segs.join("::"), Some(parent))
}

fn cmd_cargo(cmd: &str, args: &[String]) {
    let (dir, flags, passthrough) = split_args(args);
    let api_mode = flags.iter().any(|a| a == "--api");
    let update_api = flags.iter().any(|a| a == "--update-api");
    let cargo_args: Vec<String> = flags.into_iter().filter(|a| a != "--keep-rust" && a != "--api" && a != "--update-api").collect();
    let out_dir = dir.join("target").join("uredo");
    let lowered = match lower_crate(&dir, &out_dir, true, true) {
        Ok(l) => l,
        Err(n) => {
            eprintln!("error: {} file(s) failed to lower", n);
            exit(1);
        }
    };
    if api_mode {
        check_api(&dir, &lowered, update_api);
    }
    let manifest = lowered.out_dir.join("Cargo.toml");
    // artefacts go to the package's own target/, or to CARGO_TARGET_DIR when set (shared builds)
    let target_dir = std::env::var("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|_| dir.join("target"));
    let mut cmdline = Command::new("cargo");
    cmdline
        .arg(cmd)
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target_dir)
        .arg("--message-format=json-diagnostic-rendered-ansi")
        .args(&cargo_args);
    if !passthrough.is_empty() {
        cmdline.arg("--").args(&passthrough);
    }
    let mut child = cmdline.stdout(Stdio::piped()).spawn().unwrap_or_else(|e| {
        eprintln!("cannot run cargo: {}", e);
        exit(1)
    });
    let stdout = child.stdout.take().unwrap();
    let translator = Translator::new(&dir, &lowered);
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        // Cargo's messages and the program's own output share this pipe. Every cargo message carries
        // a `reason`; a line without one is the program talking, and a program that prints JSON —
        // which is not rare — must not lose it.
        if line.starts_with('{') {
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if v.get("reason").and_then(Value::as_str).is_some() {
                    if v["reason"] == "compiler-message" {
                        eprint!("{}", translator.translate(&v["message"]));
                    }
                    continue;
                }
            }
        }
        println!("{}", line);
    }
    let status = child.wait().expect("cargo");
    exit(status.code().unwrap_or(1));
}

fn cmd_package(args: &[String]) {
    let (dir, flags, _) = split_args(args);
    let manifest = read(&dir.join("Cargo.toml").display().to_string());
    let name = package_name(&manifest);
    let out = match flags.iter().position(|a| a == "--out") {
        Some(i) => PathBuf::from(flags.get(i + 1).unwrap_or_else(|| usage())),
        None => dir.join("target").join("package").join(&name),
    };
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).expect("create the package directory");
    match lower_crate(&dir, &out, false, false) {
        Ok(_) => {
            fs::write(
                out.join("UREDO-GENERATED.txt"),
                format!("Exported by `uredo package` from the Uredo sources of `{}`.\nThis package is plain Rust: build, test and publish it with Cargo; no Uredo is required (§4.6).\n", name),
            )
            .ok();
            println!("exported {} to {}", name, out.display());
        }
        Err(n) => {
            eprintln!("error: {} file(s) failed to lower", n);
            exit(1);
        }
    }
}

/// `uredo publish` (§4.6): export the package Cargo can build, then let Cargo publish it.
///
/// The export is the only path to crates.io in v0.x, so this command exists to make the two steps
/// one and to run the check Uredo owes before an irreversible one. A published crate's public API
/// is a promise (D14, §4.4); publishing one that no longer matches `uredo-api.json` would make that
/// promise by accident, so an unreviewed change stops the command. Everything else is Cargo's:
/// flags are forwarded unchanged, `--dry-run` included, and the credentials never touch Uredo.
fn cmd_publish(args: &[String]) {
    let (dir, flags, _) = split_args(args);
    let allow_api_change = flags.iter().any(|a| a == "--allow-api-change");
    let dry_run = flags.iter().any(|a| a == "--dry-run");
    let cargo_args: Vec<String> = flags.into_iter().filter(|a| a != "--allow-api-change").collect();

    let manifest = read(&dir.join("Cargo.toml").display().to_string());
    let name = package_name(&manifest);

    // Every pre-flight problem is reported before the command gives up, rather than one per run:
    // publishing is a step nobody wants to attempt three times to learn three things.
    let mut refusals: Vec<String> = Vec::new();

    // crates.io rejects a crate with no licence, and Cargo only *warns* locally, so a real publish
    // would fail after the upload had begun.
    if !manifest.contains("license") {
        refusals.push(format!(
            "no `license` or `license-file` in {}\n  crates.io requires one; `cargo publish --dry-run` only warns, so the failure would arrive at the registry\n  the Rust ecosystem's convention is `license = \"MIT OR Apache-2.0\"` with both texts in the package",
            dir.join("Cargo.toml").display()
        ));
    }

    // lower once into the build directory: that is what the API manifest is compared against
    let lowered = match lower_crate(&dir, &dir.join("target").join("uredo"), true, true) {
        Ok(l) => l,
        Err(n) => {
            eprintln!("error: {} file(s) failed to lower", n);
            exit(1);
        }
    };
    match api_state(&dir, &lowered) {
        ApiState::Unchanged(n) => println!("public API unchanged: {} items match uredo-api.json", n),
        ApiState::Unrecorded => refusals.push(format!(
            "no `uredo-api.json` in {}\n  a published crate's public API is a promise (§4.4, D14); record it with `uredo check --api` and review it before publishing",
            dir.display()
        )),
        ApiState::Changed(rendered) => {
            eprint!("{}", rendered);
            if allow_api_change {
                eprintln!("warning: publishing an unreviewed public API change, as `--allow-api-change` asks");
            } else {
                refusals.push(String::from(
                    "unreviewed public API change (§4.4); review it, then `uredo check --api --update-api`\n  `--allow-api-change` publishes anyway, which is the right flag only when the change *is* the release",
                ));
            }
        }
    }
    if !refusals.is_empty() {
        for r in &refusals {
            eprintln!("error: {}", r);
        }
        eprintln!("{} thing(s) to settle before publishing {}", refusals.len(), name);
        exit(1);
    }

    // the exported package is what Cargo sees: generated Rust, copied assets, normalised manifest
    let out = dir.join("target").join("package").join(&name);
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).expect("create the package directory");
    if let Err(n) = lower_crate(&dir, &out, false, false) {
        eprintln!("error: {} file(s) failed to lower", n);
        exit(1);
    }
    fs::write(
        out.join("UREDO-GENERATED.txt"),
        format!("Exported by `uredo publish` from the Uredo sources of `{}`.\nThis package is plain Rust: build, test and publish it with Cargo; no Uredo is required (§4.6).\n", name),
    )
    .ok();
    println!("{} {} from {}", if dry_run { "verifying" } else { "publishing" }, name, out.display());

    let status = Command::new("cargo")
        .arg("publish")
        .arg("--manifest-path")
        .arg(out.join("Cargo.toml"))
        .args(&cargo_args)
        .status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("error: could not run cargo: {}", e);
            exit(1);
        }
    }
}

/// `uredo fix` (§28.1, §28.2): apply the rewrites the compiler can make and then prove it made them
/// safely.
///
/// §28.2 promises this command for migrating source across a breaking syntax change. None has
/// happened — the v0.1 surface is frozen — so there are no migrations, and what the command does
/// today is apply the findings whose rewrite is *provably* invisible in the output.
///
/// **The contract is the point.** A fix is applied only if the generated Rust is byte-identical
/// afterwards, and that is checked per file rather than argued: the file is lowered before and
/// after, and an edit that changes a single byte of output is put back. `redundant_take` is the
/// only finding that qualifies today, because the lint's whole claim is that the annotation changes
/// no signature. `borrowed_container` and `copy_candidate` change what a function accepts and are
/// therefore suggestions, not fixes; no amount of care makes them safe to apply to a caller nobody
/// looked at.
fn cmd_fix(args: &[String]) {
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let mut files: Vec<PathBuf> = Vec::new();
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let roots: Vec<PathBuf> = if paths.is_empty() { vec![PathBuf::from(".")] } else { paths.iter().map(PathBuf::from).collect() };
    for p in roots {
        if p.is_dir() {
            let mut all = Vec::new();
            collect(&p, &mut all);
            files.extend(all.into_iter().filter(|f| f.extension().map(|e| e == "ure").unwrap_or(false)));
        } else {
            files.push(p);
        }
    }
    files.sort();
    if files.is_empty() {
        eprintln!("error: no `.ure` files to fix");
        exit(1);
    }

    let mut applied = 0;
    let mut refused = 0;
    let mut broken = 0;
    for f in &files {
        let name = f.display().to_string();
        let original = read(&name);
        let mut text = original.clone();
        let before = uredo::compile(&text, true);
        if before.has_errors() {
            broken += 1;
            for d in before.diags.iter().filter(|d| d.level == uredo::diag::Level::Error) {
                eprint!("{}", d.render(&name, &text));
            }
            continue;
        }
        // one finding at a time: each is verified on its own, so a refusal costs only itself
        loop {
            let out = uredo::compile(&text, true);
            let Some(fix) = out.lints.iter().find_map(|d| d.fix.clone()) else { break };
            let Some(next) = apply_fix(&text, &fix) else { break };
            let after = uredo::compile(&next, true);
            if after.has_errors() || after.rust != before.rust {
                eprintln!("{}:{}: refused — {} would change the generated Rust", name, fix.line, fix.describe);
                refused += 1;
                break;
            }
            println!("{}:{}: {}", name, fix.line, fix.describe);
            text = next;
            applied += 1;
        }
        if text != original && !dry_run {
            fs::write(f, &text).expect("write the fixed file");
        }
    }
    if broken > 0 {
        eprintln!("{} file(s) did not compile and were left alone", broken);
    }
    println!(
        "{} {} fix(es) in {} file(s){}",
        if dry_run { "would apply" } else { "applied" },
        applied,
        files.len(),
        if refused > 0 { format!("; {} refused", refused) } else { String::new() }
    );
    if broken > 0 || refused > 0 {
        exit(1);
    }
}

/// Applies one rewrite, or `None` when the text it names is not where it said it would be — which
/// happens when an earlier fix on the same line has already moved it.
fn apply_fix(src: &str, fix: &uredo::diag::Fix) -> Option<String> {
    let mut lines: Vec<String> = src.split('\n').map(|l| l.to_string()).collect();
    let i = fix.line.checked_sub(1)?;
    let line = lines.get(i)?;
    let at = line.find(&fix.find)?;
    let mut replaced = line[..at].to_string();
    replaced.push_str(&fix.replace);
    replaced.push_str(&line[at + fix.find.len()..]);
    lines[i] = replaced;
    Some(lines.join("\n"))
}

// ----- fmt (§29) -----

fn cmd_fmt(args: &[String]) {
    let check = args.iter().any(|a| a == "--check");
    let mut files: Vec<PathBuf> = Vec::new();
    for a in args.iter().filter(|a| !a.starts_with("--")) {
        let p = PathBuf::from(a);
        if p.is_dir() {
            let mut all = Vec::new();
            collect(&p, &mut all);
            files.extend(all.into_iter().filter(|f| f.extension().map(|e| e == "ure").unwrap_or(false)));
        } else {
            files.push(p);
        }
    }
    if files.is_empty() {
        usage();
    }
    let mut changed = 0;
    let mut failed = 0;
    for f in &files {
        let name = f.display().to_string();
        let src = read(&name);
        match uredo::fmt::format(&src) {
            Ok(out) => {
                if out != src {
                    changed += 1;
                    if check {
                        println!("would reformat {}", name);
                    } else {
                        fs::write(f, out).expect("write formatted file");
                        println!("formatted {}", name);
                    }
                }
            }
            Err(diags) => {
                failed += 1;
                for d in diags {
                    eprint!("{}", d.render(&name, &src));
                }
            }
        }
    }
    if failed > 0 {
        exit(1);
    }
    if check && changed > 0 {
        eprintln!("{} file(s) would be reformatted", changed);
        exit(1);
    }
}

// ----- new / init (§28.1) -----

/// The manifest of §4.1, with nothing a new package does not need.
fn manifest(name: &str) -> String {
    format!(
        "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\nrust-version = \"1.85\"\n\n[dependencies]\n",
        name
    )
}

const MAIN_URE: &str = "##! A new Uredo package. `uredo run` builds it and runs it.\n\nfn main():\n    name = \"world\"\n    print(\"hello, {name}\")\n";

const LIB_URE: &str = "##! A new Uredo library. `uredo test` runs the tests, `uredo check` type-checks it.\n\n## Adds two numbers.\npub fn add(a: i64, b: i64) -> i64:\n    a + b\n\n@test\nfn add_works():\n    assert_eq!(add(2, 2), 4)          # assertion macros are Rust's (§24)\n";

/// A Cargo package name: what Cargo accepts, derived from the directory name.
fn package_name_from(path: &Path) -> Option<String> {
    let raw = path.file_name()?.to_string_lossy().to_string();
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    let cleaned = cleaned.trim_matches('_').to_string();
    if cleaned.is_empty() || cleaned.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        return None;
    }
    Some(cleaned)
}

fn cmd_new(args: &[String], in_place: bool) {
    let lib = args.iter().any(|a| a == "--lib");
    let mut named: Option<String> = None;
    let mut skip = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--name" {
            match args.get(i + 1) {
                Some(v) => {
                    named = Some(v.clone());
                    skip.push(i + 1);
                }
                None => usage(),
            }
        }
    }
    let positional: Vec<&String> = args.iter().enumerate().filter(|(i, a)| !a.starts_with("--") && !skip.contains(i)).map(|(_, a)| a).collect();
    let dir = match (in_place, positional.first()) {
        (false, Some(p)) => PathBuf::from(p),
        (false, None) => {
            eprintln!("error: `uredo new` needs a path: `uredo new hello`");
            exit(2);
        }
        (true, Some(p)) => PathBuf::from(p),
        (true, None) => PathBuf::from("."),
    };
    let absolute = if dir.is_absolute() { dir.clone() } else { std::env::current_dir().unwrap_or_default().join(&dir) };
    let name = match named.or_else(|| package_name_from(&absolute)) {
        Some(n) => n,
        None => {
            eprintln!("error: cannot make a package name from `{}`; pass `--name`", dir.display());
            exit(2);
        }
    };
    if !in_place && dir.exists() {
        eprintln!("error: `{}` already exists; use `uredo init {}` to add a package to it", dir.display(), dir.display());
        exit(1);
    }
    let cargo = dir.join("Cargo.toml");
    if cargo.exists() {
        eprintln!("error: `{}` already has a Cargo.toml", dir.display());
        exit(1);
    }
    let entry = dir.join("src").join(if lib { "lib.ure" } else { "main.ure" });
    if entry.exists() {
        eprintln!("error: `{}` already exists", entry.display());
        exit(1);
    }
    if let Err(e) = fs::create_dir_all(dir.join("src")) {
        eprintln!("error: cannot create {}: {}", dir.join("src").display(), e);
        exit(1);
    }
    let write = |path: PathBuf, body: &str| {
        if let Err(e) = fs::write(&path, body) {
            eprintln!("error: cannot write {}: {}", path.display(), e);
            exit(1);
        }
    };
    write(cargo, &manifest(&name));
    write(entry.clone(), if lib { LIB_URE } else { MAIN_URE });
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        write(gitignore, "target\n");
    }
    println!("created {} package `{}` in {}", if lib { "library" } else { "binary" }, name, dir.display());
    println!("  {}", entry.display());
    println!("next: `uredo {}{}`", if lib { "test" } else { "run" }, if dir == PathBuf::from(".") { String::new() } else { format!(" {}", dir.display()) });
}

// ----- report (§5.4, §28.1) -----

/// A bug bundle: everything someone needs to see a lowering defect without the reporter's machine.
///
/// §5.4 divides what rustc says about generated code in two. A **barrier diagnostic** is the user's
/// own ownership or type error, arrives translated, and is not a compiler bug. A **lowering defect**
/// is anything the user's source could not have caused, and that is what this collects: the sources,
/// the generated crate, the map between them, the command, its whole output, and the versions.
/// The bundle is written, never sent, and it says on its face that it contains the reporter's code.
fn cmd_report(args: &[String]) {
    let (dir, flags, _) = split_args(args);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let out = match flags.iter().position(|a| a == "--out") {
        Some(i) => PathBuf::from(flags.get(i + 1).unwrap_or_else(|| usage())),
        None => dir.join("target").join("uredo").join(format!("report-{}", stamp)),
    };
    let generated = dir.join("target").join("uredo");
    let lowered = lower_crate(&dir, &generated, true, true);
    let lower_failed = lowered.is_err();

    // the command whose failure is being reported: `cargo check` over the generated crate
    let manifest = generated.join("Cargo.toml");
    let target_dir = std::env::var("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|_| dir.join("target"));
    let command = format!("cargo check --manifest-path {} --target-dir {}", manifest.display(), target_dir.display());
    let mut output = String::new();
    let mut mapped = 0usize;          // an error with a Uredo line behind it: the program's own
    let mut unmapped: Vec<String> = Vec::new();   // in generated code, with no Uredo line: ours
    let mut foreign: Vec<String> = Vec::new();    // in a hand-written `.rs` of theirs, or a dependency
    let mut status = String::from("not run: the crate did not lower");
    if !lower_failed {
        let run = Command::new("cargo")
            .args(["check", "--manifest-path"])
            .arg(&manifest)
            .arg("--target-dir")
            .arg(&target_dir)
            .arg("--message-format=json-diagnostic-rendered-ansi")
            .output();
        match run {
            Ok(o) => {
                status = format!("exit {}", o.status.code().unwrap_or(-1));
                let translator = Translator::new(&dir, lowered.as_ref().unwrap());
                for line in String::from_utf8_lossy(&o.stdout).lines() {
                    if let Ok(v) = serde_json::from_str::<Value>(line) {
                        if v["reason"] == "compiler-message" {
                            let msg = &v["message"];
                            if msg["level"] == "error" {
                                let text = translator.translate(msg);
                                let what = msg["message"].as_str().unwrap_or("").to_string();
                                let in_generated = msg["spans"]
                                    .as_array()
                                    .map(|a| {
                                        a.iter().any(|sp| {
                                            sp["file_name"].as_str().map(|f| lowered.as_ref().map(|l| l.maps.contains_key(f)).unwrap_or(false)).unwrap_or(false)
                                        })
                                    })
                                    .unwrap_or(false);
                                if text.contains(".ure:") {
                                    mapped += 1;
                                } else if in_generated {
                                    unmapped.push(what);   // generated code no rule accounts for
                                } else {
                                    foreign.push(what);    // their own Rust, or a dependency's
                                }
                            }
                            output.push_str(msg["rendered"].as_str().unwrap_or(""));
                        }
                    }
                }
                output.push_str(&String::from_utf8_lossy(&o.stderr));
            }
            Err(e) => status = format!("cargo could not be run: {}", e),
        }
    }

    let _ = fs::remove_dir_all(&out);
    if let Err(e) = fs::create_dir_all(&out) {
        eprintln!("error: cannot create {}: {}", out.display(), e);
        exit(1);
    }
    let mut copied = Vec::new();
    let copy_tree = |from: &Path, name: &str, copied: &mut Vec<String>| {
        if !from.exists() {
            return;
        }
        let mut files = Vec::new();
        collect_any(from, &mut files);
        for f in files {
            let rel = f.strip_prefix(from).unwrap_or(&f);
            let dest = out.join(name).join(rel);
            fs::create_dir_all(dest.parent().unwrap()).ok();
            if fs::copy(&f, &dest).is_ok() {
                copied.push(format!("{}/{}", name, rel.display()));
            }
        }
    };
    copy_tree(&dir.join("src"), "src", &mut copied);
    copy_tree(&generated.join("src"), "generated", &mut copied);
    copy_tree(&generated.join("provenance"), "provenance", &mut copied);
    for f in ["Cargo.toml", "Cargo.lock", "build.rs"] {
        if dir.join(f).is_file() && fs::copy(dir.join(f), out.join(f)).is_ok() {
            copied.push(f.to_string());
        }
    }
    fs::write(out.join("output.txt"), &output).ok();

    let version = |program: &str, args: &[&str]| -> String {
        Command::new(program)
            .args(args)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("{}: not found", program))
    };
    let verdict = if lower_failed {
        "**a lowering defect**: the crate did not lower at all, which §5.4 says never happens for a program that passes Uredo's own checks".to_string()
    } else if !unmapped.is_empty() {
        "**probably a lowering defect** (§5.4): an error lands in generated Rust that no Uredo line accounts for, so the source could not have written it".to_string()
    } else if mapped > 0 {
        "**probably a barrier diagnostic** (§5.4): every error maps back to a line of Uredo source, so it is likely the program's own and not the compiler's".to_string()
    } else if !foreign.is_empty() {
        format!("**not a lowering defect**: the {} error(s) are in hand-written Rust of your own or in a dependency, not in anything the compiler generated", foreign.len())
    } else {
        "**not reproduced**: the command reported no error, so whatever was seen is not in this bundle".to_string()
    };

    let mut md = String::new();
    md.push_str("# Uredo bug bundle\n\n");
    md.push_str("**This bundle contains your source code**, generated from it and beside it. Read it before\nsending it anywhere.\n\n");
    md.push_str("## What this looks like\n\n");
    md.push_str(&format!("{}\n\n", verdict));
    md.push_str(&format!("- errors that map back to Uredo source: **{}**\n", mapped));
    md.push_str(&format!("- errors in generated Rust with no Uredo line behind them: **{}**\n", unmapped.len()));
    for u in unmapped.iter().take(5) {
        md.push_str(&format!("  - {}\n", u));
    }
    md.push_str(&format!("- errors in hand-written Rust or a dependency: **{}**\n", foreign.len()));
    for f in foreign.iter().take(5) {
        md.push_str(&format!("  - {}\n", f));
    }
    md.push_str("\n## The command\n\n");
    md.push_str(&format!("```\n{}\n```\n\nResult: {}. Its whole output is in `output.txt`.\n\n", command, status));
    md.push_str("## Versions\n\n");
    md.push_str(&format!("| uredo | {} |\n|---|---|\n", env!("CARGO_PKG_VERSION")));
    md.push_str(&format!("| rustc | {} |\n", version("rustc", &["--version"])));
    md.push_str(&format!("| cargo | {} |\n", version("cargo", &["--version"])));
    md.push_str(&format!("| rustfmt | {} |\n", version("rustfmt", &["--version"])));
    md.push_str(&format!("| target | {} |\n\n", version("rustc", &["-vV"]).lines().find(|l| l.starts_with("host:")).unwrap_or("host: unknown")));
    md.push_str("## What is here\n\n");
    md.push_str("| Directory | What it holds |\n|---|---|\n");
    md.push_str("| `src/` | the Uredo sources, as written |\n");
    md.push_str("| `generated/` | the Rust the compiler produced from them |\n");
    md.push_str("| `provenance/` | the line map and the elaboration records: which Uredo line each Rust line came from, and which rule put it there (§5.3) |\n");
    md.push_str("| `output.txt` | everything the command printed |\n\n");
    md.push_str(&format!("{} file(s) in all.\n\n", copied.len()));
    md.push_str("## Reading it\n\n");
    md.push_str("Pick an error in `output.txt`, find its line in `generated/`, and look that line up in\n`provenance/` to see the Uredo line and the rule behind it. If the rule is right and the Rust is\nwrong, it is a lowering defect (§5.4).\n");
    fs::write(out.join("REPORT.md"), md).ok();

    println!("wrote a bug bundle to {}", out.display());
    println!("  {} file(s); it contains your source, so read it before sending it", copied.len());
    println!("  start with REPORT.md");
}

/// Every file under a directory, whatever its extension.
fn collect_any(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_any(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

// ----- lint (§28.1) -----

fn cmd_lint(args: &[String]) {
    let deny = args.iter().any(|a| a == "--deny");
    let measure = args.iter().any(|a| a == "--measure");
    let mut allowed: Vec<String> = Vec::new();
    let mut skip: Vec<usize> = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--allow" {
            match args.get(i + 1) {
                Some(name) if uredo::lint::NAMES.contains(&name.as_str()) => {
                    allowed.push(name.clone());
                    skip.push(i + 1);
                }
                Some(name) => {
                    eprintln!("error: unknown lint `{}`; known lints: {}", name, uredo::lint::NAMES.join(", "));
                    exit(2);
                }
                None => usage(),
            }
        }
    }
    let mut files: Vec<PathBuf> = Vec::new();
    let paths: Vec<&String> = args.iter().enumerate().filter(|(i, a)| !a.starts_with("--") && !skip.contains(i)).map(|(_, a)| a).collect();
    let roots: Vec<PathBuf> = if paths.is_empty() { vec![PathBuf::from(".")] } else { paths.iter().map(PathBuf::from).collect() };
    for p in roots {
        if p.is_dir() {
            let mut all = Vec::new();
            collect(&p, &mut all);
            files.extend(all.into_iter().filter(|f| f.extension().map(|e| e == "ure").unwrap_or(false)));
        } else {
            files.push(p);
        }
    }
    files.sort();
    if files.is_empty() {
        eprintln!("error: no `.ure` files to lint");
        exit(1);
    }
    let mut findings = 0;
    let mut broken = 0;
    let mut p8_params = 0usize;
    let mut p8_clones = 0usize;
    let mut d2_refs = 0usize;
    let mut own_refs = 0usize;
    let mut lifetimes = uredo::lint::Lifetimes::default();
    // Each file is compiled against its own package's index, so a lint sees what a build sees.
    let mut indexes: std::collections::HashMap<PathBuf, uredo::lower::CrateIndex> = Default::default();
    for f in &files {
        let name = f.display().to_string();
        let src = read(&name);
        let out = match package_of(f) {
            Some((pkg, module_path)) => {
                let crate_name = package_name(&fs::read_to_string(pkg.join("Cargo.toml")).unwrap_or_default()).replace('-', "_");
                let index = indexes.entry(pkg.clone()).or_insert_with(|| index_package(&pkg));
                uredo::compile_in_crate(&src, false, index, &crate_name, &module_path)
            }
            None => uredo::compile(&src, false),
        };
        if out.has_errors() {
            broken += 1;
            for d in out.diags.iter().filter(|d| d.level == uredo::diag::Level::Error) {
                eprint!("{}", d.render(&name, &src));
            }
            continue;
        }
        if measure {
            p8_params += out.measures.p8_params;
            p8_clones += out.measures.p8_clones.len();
            d2_refs += out.measures.d2_refs;
            own_refs += out.measures.own_refs;
            let l = &out.measures.lifetimes;
            lifetimes.in_reference += l.in_reference;
            lifetimes.in_bound += l.in_bound;
            lifetimes.in_generics += l.in_generics;
            lifetimes.label += l.label;
            lifetimes.static_named += l.static_named;
            lifetimes.anonymous += l.anonymous;
            lifetimes.lines += l.lines;
            for d in &out.measures.p8_clones {
                print!("{}", d.render(&name, &src));
            }
            continue;
        }
        for d in &out.lints {
            if uredo::lint::name_of(d).map(|n| allowed.iter().any(|a| a == n)).unwrap_or(false) {
                continue;
            }
            findings += 1;
            print!("{}", d.render(&name, &src));
        }
    }
    if broken > 0 {
        eprintln!("{} file(s) did not compile; their findings are not reported", broken);
        exit(1);
    }
    if measure {
        // §38: P8 is reversed when a project writes more clones to feed by-value parameters
        // than the `take` annotations a borrow default would have cost it.
        println!("D52 reversal counts over {} file(s) (§38):", files.len());
        println!("  by-value generic parameters (P8)   {:>6}   `take` annotations a borrow default would need", p8_params);
        println!("  `.clone()` at a P8 call site       {:>6}{}", p8_clones, match p8_params {
            0 => String::new(),
            n => format!("   {:.2} of that number", p8_clones as f64 / n as f64),
        });
        println!("  mapped E0382 naming a P8 parameter      —   count it in a `uredo build` log: the message ends `(P8)`");
        // What D2 costs the surface (§10.3, §37): every one of these is a reference Uredo would have
        // inserted had the callee been its own, and had to be written because it is Rust's.
        println!("D2 cost over the same files (§10.3):");
        println!("  `&`/`&mut` written for a Rust item {:>6}   Uredo inserts the borrow only for its own callees", d2_refs);
        println!("  the same at an Uredo call site     {:>6}   written where Uredo would have inserted it (D47)", own_refs);
        // §38 holds the apostrophe notation open on a *rate*: more than 3 written lifetimes per
        // 1,000 lines of real Uredo, over at least 2,300 lines. This is where that number comes
        // from; the buckets are the ones the corpus study used, so the two can be set side by side.
        let total = lifetimes.total();
        let rate = if lifetimes.lines > 0 { 1000.0 * total as f64 / lifetimes.lines as f64 } else { 0.0 };
        println!("Lifetimes written over the same files (§38), in {} non-blank lines:", lifetimes.lines);
        println!("  in a reference   `&'a T`      {:>6}", lifetimes.in_reference);
        println!("  as a bound       `T: 'a`      {:>6}", lifetimes.in_bound);
        println!("  a generic argument or binder  {:>6}", lifetimes.in_generics);
        println!("  a loop label     `'outer:`    {:>6}", lifetimes.label);
        println!("  total {:>4}, of which `'static` {}, `'_` {}   {:.1} per 1,000 lines{}", total, lifetimes.static_named, lifetimes.anonymous, rate,
            if lifetimes.lines < 2300 {
                format!("  (§38 asks for 2,300 lines before the rate counts; {} short)", 2300 - lifetimes.lines)
            } else if rate > 3.0 {
                "  — above §38's threshold of 3: the notation question reopens".to_string()
            } else {
                "  — below §38's threshold of 3".to_string()
            });
        if p8_params > 0 && p8_clones > p8_params {
            println!("the clones outnumber the annotations saved: §38's second condition is met on these files");
        }
        return;
    }
    if findings == 0 {
        println!("no findings in {} file(s)", files.len());
        return;
    }
    println!("{} finding(s) in {} file(s)", findings, files.len());
    if deny {
        exit(1);
    }
}

// ----- explain (§26.2) -----

fn cmd_explain(args: &[String]) {
    let target = args.first().unwrap_or_else(|| usage());
    let (file, line) = match target.rsplit_once(':') {
        Some((f, l)) => (f.to_string(), l.parse::<usize>().unwrap_or_else(|_| usage())),
        None => usage(),
    };
    let src = read(&file);
    let out = uredo::compile(&src, true);
    for d in &out.diags {
        eprint!("{}", d.render(&file, &src));
    }
    if out.has_errors() {
        exit(1);
    }
    let rs_name = Path::new(&file).with_extension("rs").display().to_string();
    let elabs: Vec<&Elab> = out.map.elabs.iter().filter(|e| e.line == line).collect();
    let src_line = src.lines().nth(line.wrapping_sub(1)).unwrap_or("").trim();
    if elabs.is_empty() {
        println!("Statement:           {:<40} {}:{}", src_line, file, line);
        println!("Uredo elaboration:   none (emitted as written)");
        println!("Inserted by Uredo:   borrow: none · clone: none · allocation: none · dispatch: none · sync: none");
        println!();
    }
    for e in elabs {
        println!("Expression:          {:<40} {}:{}:{}", e.before, file, e.line, e.col);
        println!("Uredo elaboration:   {}", e.after.lines().next().unwrap_or(""));
        println!("    rule: {}", e.rule);
        if let Some(d) = &e.declared {
            println!("    callee declares: {}", d);
        }
        if !e.takes.is_empty() {
            println!("    ownership transferred: {}", e.takes.join(", "));
        }
        println!("Inserted by Uredo:   {}", e.inserted);
        println!("Rust adjustments:    unknown (no semantic engine in v0.x)");
        println!("Callee effects:      unknown (not analysed)");
        println!();
    }
    let rs_lines = out.map.rs_lines(line);
    if !rs_lines.is_empty() {
        println!("Generated Rust ({}):", rs_name);
        for l in rs_lines {
            println!("{:>5} | {}", l, out.rust.lines().nth(l - 1).unwrap_or(""));
        }
    }
}

// ----- rustc diagnostic translation (§27) -----

struct Translator {
    gen_dir: PathBuf,
    maps: BTreeMap<String, Map>,
    ure_sources: BTreeMap<String, Vec<String>>,
}

struct MappedSpan {
    ure_file: String,
    ure_line: usize,
    ure_col: usize,
    width: usize,
    label: String,
    is_primary: bool,
    rs_file: String,
    rs_line: usize,
    rs_col: usize,
}

fn backticked(s: &str) -> Option<String> {
    let start = s.find('`')? + 1;
    let end = s[start..].find('`')? + start;
    Some(s[start..end].to_string())
}

impl Translator {
    fn new(pkg_dir: &Path, lowered: &Lowered) -> Translator {
        let mut ure_sources = BTreeMap::new();
        for m in lowered.maps.values() {
            let text = fs::read_to_string(pkg_dir.join(&m.ure)).unwrap_or_default();
            ure_sources.insert(m.ure.clone(), text.lines().map(|s| s.to_string()).collect());
        }
        Translator { gen_dir: lowered.out_dir.clone(), maps: lowered.maps.clone(), ure_sources }
    }

    fn map_span(&self, span: &Value) -> Option<MappedSpan> {
        let file = span["file_name"].as_str()?.to_string();
        let Some(map) = self.maps.get(&file) else {
            // A diagnostic from inside a macro points at the macro's own file, in a dependency; the
            // call site — the line the programmer wrote — is the expansion's span (§27).
            return span.get("expansion").and_then(|e| e.get("span")).and_then(|s| self.map_span(s));
        };
        let rs_line = span["line_start"].as_u64()? as usize;
        let rs_col = span["column_start"].as_u64()? as usize;
        let rs_col_end = span["column_end"].as_u64().unwrap_or(rs_col as u64) as usize;
        let ure_line = map.ure_line(rs_line)?;
        // the span's text, searched in the Uredo line to find a column
        let gen_line = fs::read_to_string(self.gen_dir.join(&file)).ok().and_then(|t| t.lines().nth(rs_line - 1).map(|s| s.to_string())).unwrap_or_default();
        let gen_chars: Vec<char> = gen_line.chars().collect();
        let end = rs_col_end.saturating_sub(1).max(rs_col);
        let text: String = if rs_col >= 1 && rs_col - 1 <= gen_chars.len() { gen_chars[rs_col - 1..end.min(gen_chars.len())].iter().collect() } else { String::new() };
        let ure_src = self.ure_sources.get(&map.ure).and_then(|ls| ls.get(ure_line - 1)).cloned().unwrap_or_default();
        let indent = ure_src.len() - ure_src.trim_start().len();
        let whole = (indent + 1, ure_src.trim().chars().count().max(1));
        let t = text.trim();
        let (ure_col, width) = if t.is_empty() {
            whole
        } else if let Some(b) = ure_src.find(t) {
            (ure_src[..b].chars().count() + 1, t.chars().count())
        } else {
            let ident: String = t.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            match if ident.is_empty() { None } else { ure_src.find(&ident) } {
                Some(b) => (ure_src[..b].chars().count() + 1, ident.chars().count()),
                None => whole,
            }
        };
        Some(MappedSpan {
            ure_file: map.ure.clone(),
            ure_line,
            ure_col,
            width,
            label: span["label"].as_str().unwrap_or("").to_string(),
            is_primary: span["is_primary"].as_bool().unwrap_or(false),
            rs_file: file,
            rs_line,
            rs_col,
        })
    }

    fn elabs_on(&self, ure_file: &str, line: usize) -> Vec<&Elab> {
        self.maps.values().filter(|m| m.ure == ure_file).flat_map(|m| m.elabs.iter().filter(move |e| e.line == line)).collect()
    }

    fn translate(&self, msg: &Value) -> String {
        let level = msg["level"].as_str().unwrap_or("error");
        if level == "failure-note" {
            return String::new();
        }
        let code = msg["code"]["code"].as_str().unwrap_or("").to_string();
        let mut message = msg["message"].as_str().unwrap_or("").to_string();
        let rendered = msg["rendered"].as_str().unwrap_or("").to_string();
        let spans: Vec<MappedSpan> = msg["spans"].as_array().map(|a| a.iter().filter_map(|s| self.map_span(s)).collect()).unwrap_or_default();
        let mut notes: Vec<String> = Vec::new();
        for c in msg["children"].as_array().unwrap_or(&vec![]) {
            let lvl = c["level"].as_str().unwrap_or("note");
            let text = c["message"].as_str().unwrap_or("").to_string();
            if text.is_empty() {
                continue;
            }
            let cspans: Vec<MappedSpan> = c["spans"].as_array().map(|a| a.iter().filter_map(|s| self.map_span(s)).collect()).unwrap_or_default();
            match cspans.first() {
                Some(s) => notes.push(format!("{}: {} ({}:{})", lvl, text, s.ure_file, s.ure_line)),
                None => notes.push(format!("{}: {}", lvl, text)),
            }
        }
        if spans.is_empty() {
            if message.starts_with("aborting") || message.contains("could not compile") || message.starts_with("For more information") {
                return String::new();
            }
            let has_unmapped = msg["spans"].as_array().map(|a| !a.is_empty()).unwrap_or(false);
            if has_unmapped {
                // a span in a hand-written `.rs` file or a dependency: show rustc's own text
                return format!("{}\n", rendered);
            }
            return format!("{}{}: {}\n", level, if code.is_empty() { String::new() } else { format!("[{}]", code) }, message);
        }
        let primary = spans.iter().find(|s| s.is_primary).unwrap_or(&spans[0]);
        let mut labels: BTreeMap<usize, String> = BTreeMap::new();

        match code.as_str() {
            "E0382" => {
                let var = backticked(&message).unwrap_or_default();
                let moved = spans.iter().find(|s| s.label.contains("moved"));
                let callee_elab = moved.and_then(|m| self.elabs_on(&m.ure_file, m.ure_line).into_iter().find(|e| e.takes.iter().any(|t| *t == var || t.starts_with(&format!("{}.", var)))));
                if let Some(e) = callee_elab {
                    let callee = e.callee.clone().unwrap_or_default();
                    message = format!("`{}` cannot be used here because `{}` takes ownership of it", var, callee);
                    if let Some(m) = moved {
                        labels.insert(m.ure_line, "ownership transferred here".into());
                    }
                    labels.insert(primary.ure_line, "used after transfer".into());
                    notes.clear();
                    if let Some(d) = &e.declared {
                        notes.push(format!("`{}` declares: {}", callee, d));
                    }
                    notes.push(format!("help: borrow instead if `{}` does not need ownership, or write `{}({}.clone())`", callee, callee, var));
                    notes.push("note: Uredo did not insert a clone because cloning can allocate or copy substantial data (§10.4)".into());
                } else if let Some((m, e)) = moved.and_then(|m| self.elabs_on(&m.ure_file, m.ure_line).into_iter().find(|e| e.by_value.iter().any(|t| *t == var || t.starts_with(&format!("{}.", var)))).map(|e| (m, e))) {
                    // P8 (D52): moved through a type parameter or `impl Trait`
                    let callee = e.callee.clone().unwrap_or_default();
                    message = format!("`{}` cannot be used here because `{}` takes it by value: its parameter is a type parameter or `impl Trait` (P8)", var, callee);
                    labels.insert(m.ure_line, "moved here (by-value generic parameter)".into());
                    labels.insert(primary.ure_line, "used after the move".into());
                    notes.clear();
                    if let Some(d) = &e.declared {
                        notes.push(format!("`{}` declares: {}", callee, d));
                    }
                    notes.push(format!("help: write `{}(&{})` if a reference satisfies the parameter's bounds (the type parameter is then instantiated with `&…`), or `{}({}.clone())`", callee, var, callee, var));
                    notes.push("note: Uredo did not insert a clone because cloning can allocate or copy substantial data (§10.4)".into());
                } else if let Some(m) = moved {
                    message = format!("`{}` cannot be used here: it was moved", var);
                    labels.insert(m.ure_line, "moved here".into());
                    labels.insert(primary.ure_line, "used after the move".into());
                    notes.push("note: Uredo never inserts a clone; write `.clone()` where a copy is intended (§10.4)".into());
                }
            }
            "E0596" => {
                let var = backticked(&message).unwrap_or_default();
                let method = self.elabs_on(&primary.ure_file, primary.ure_line).into_iter().find(|e| e.kind == "method" && e.declared.as_deref().map(|d| d.contains("self: inout")).unwrap_or(false));
                message = match &method {
                    Some(e) => format!("`{}` must be a `var` binding: `{}` takes `self: inout`", var, e.callee.clone().unwrap_or_default()),
                    None => format!("`{}` is not a `var` binding, but it is mutated here", var),
                };
                labels.insert(primary.ure_line, "mutable access here".into());
                notes.clear();
                if let Some(e) = &method {
                    if let Some(d) = &e.declared {
                        notes.push(format!("`{}` declares: {}", e.callee.clone().unwrap_or_default(), d));
                    }
                }
                notes.push(format!("help: declare it with `var {} = …` (§8.1)", var));
            }
            "E0384" => {
                let var = backticked(&message).unwrap_or_default();
                message = format!("`{}` is immutable and cannot be assigned again", var);
                labels.insert(primary.ure_line, "second assignment".into());
                notes.clear();
                notes.push(format!("help: declare it with `var {} = …` (§8.1)", var));
            }
            "E0308" => {
                for e in self.elabs_on(&primary.ure_file, primary.ure_line) {
                    if e.rule.contains("inserted") {
                        notes.push(format!("note: Uredo elaborated `{}` to `{}`: {}", e.before, e.after.lines().next().unwrap_or(""), e.rule));
                    } else if e.rule.contains("verbatim") {
                        notes.push(format!("note: {}: arguments of `{}` are written as Rust wants them (`&x`, `&mut x`)", e.rule, e.callee.clone().unwrap_or_default()));
                    }
                }
                labels.insert(primary.ure_line, primary.label.clone());
            }
            "E0277" if message.contains("is not an iterator") => {
                // a `for` source that Uredo borrowed but that is itself an iterator (§16)
                if let Some(e) = self.elabs_on(&primary.ure_file, primary.ure_line).into_iter().find(|e| e.kind == "for-borrow" && e.after.starts_with('&')) {
                    let src = e.before.clone();
                    message = format!("`{}` is an iterator, not a collection: `for … in {}` borrows the place, and a reference to an iterator is not one", src, src);
                    labels.insert(primary.ure_line, "borrowed here by the `for` rule (§16)".into());
                    notes.clear();
                    notes.push(format!("help: write `for … in take {}` to consume it, or call it inline (`for … in {}.by_ref()`)", src, src));
                    notes.push("note: Uredo borrows a `for` source that is a place; ranges, iterators and method chains written inline are used as they are (§16)".into());
                } else {
                    labels.insert(primary.ure_line, primary.label.clone());
                }
            }
            "E0106" => {
                notes.push("note: a `str`/`[T]` return type with nothing to elide from must write its lifetime: `-> &'static str` (D48, §9.7)".into());
                labels.insert(primary.ure_line, primary.label.clone());
            }
            _ => {
                for s in &spans {
                    if !s.label.is_empty() {
                        labels.entry(s.ure_line).or_insert(s.label.clone());
                    }
                }
            }
        }

        // render in rustc's shape, on Uredo lines
        let mut out = String::new();
        let code_txt = if code.is_empty() { String::new() } else { format!("[{}]", code) };
        out.push_str(&format!("{}{}: {}\n", level, code_txt, message));
        out.push_str(&format!("  --> {}:{}:{}\n", primary.ure_file, primary.ure_line, primary.ure_col));
        let w = spans.iter().map(|s| s.ure_line).max().unwrap_or(1).to_string().len();
        out.push_str(&format!("{:w$} |\n", "", w = w));
        let mut shown: Vec<&MappedSpan> = spans.iter().filter(|s| s.ure_file == primary.ure_file).collect();
        shown.sort_by_key(|s| (s.ure_line, !s.is_primary));
        shown.dedup_by_key(|s| s.ure_line);
        for s in shown {
            let src = self.ure_sources.get(&s.ure_file).and_then(|ls| ls.get(s.ure_line - 1)).cloned().unwrap_or_default();
            out.push_str(&format!("{:>w$} | {}\n", s.ure_line, src, w = w));
            let label = labels.get(&s.ure_line).cloned().unwrap_or_else(|| s.label.clone());
            out.push_str(&format!("{:w$} | {}{}{}\n", "", " ".repeat(s.ure_col.saturating_sub(1)), "^".repeat(s.width.max(1)), if label.is_empty() { String::new() } else { format!(" {}", label) }, w = w));
        }
        if !notes.is_empty() {
            out.push_str(&format!("{:w$} |\n", "", w = w));
            for n in &notes {
                out.push_str(&format!("{:w$} = {}\n", "", n, w = w));
            }
        }
        // secondary panel: rustc's own diagnostic on the generated Rust
        out.push_str(&format!("{:w$} --- rustc, on generated {}:{}:{} ---\n", "", primary.rs_file, primary.rs_line, primary.rs_col, w = w));
        for l in rendered.lines() {
            out.push_str(&format!("{:w$}   {}\n", "", l, w = w));
        }
        out.push('\n');
        out
    }
}
