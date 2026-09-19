//! `uredo lsp` (§28.1, §28.2): a language server over stdio.
//!
//! It shares the compiler's own front end rather than a second one, which is the property §28.2
//! asks for: the diagnostics an editor shows are the diagnostics `uredo check` prints, on the same
//! Uredo lines, and hover is what `uredo explain` says about that line.
//!
//! The parse is tolerant but **not lossless**, and the difference decides what is here. Tolerant
//! means the parser recovers at item boundaries, so a file with one broken line still yields every
//! other item — which is why the outline, jump-to-definition and completion keep working while a
//! file is mid-edit, and they are built on the parser rather than on a full compile for exactly
//! that reason. Lossless would mean keeping every token and space, which the parser does not; that
//! is what rename and code actions need, and it is why neither is offered.
//!
//! Supported: `initialize`, `shutdown`/`exit`, full-text sync (`didOpen`, `didChange`, `didSave`,
//! `didClose`), `publishDiagnostics` (errors, warnings and lints), `textDocument/formatting`
//! (§29's formatter, refused on a file that does not compile, as the formatter itself refuses) and
//! `textDocument/hover` (§26.2's elaboration record), `textDocument/documentSymbol` (the outline)
//! `textDocument/definition` (item-level, across the sibling files §20.3 makes modules) and
//! `textDocument/completion` (the items in scope and the keywords — and nothing at all after `.`
//! or `::`, where the right answer needs a type and Uredo has no type engine to ask).

use crate::diag::Level;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, Write};

/// One open document: the text as it is now, and the last text that compiled.
///
/// The compiler is fast enough to re-run on every keystroke, so there is no incremental path to
/// keep true. What the second field buys is the half of §28.2's tolerance that costs nothing: a
/// file mid-edit usually does not parse, and a server that answers only from the current text goes
/// dark on every keystroke. Hover falls back to the last good tree — but only for a line whose text
/// has not changed since, because answering about a line the author has since rewritten would be
/// worse than answering nothing.
#[derive(Default)]
struct Document {
    text: String,
    last_good: Option<String>,
}

type Documents = HashMap<String, Document>;

pub fn serve() -> i32 {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    let mut docs: Documents = HashMap::new();
    let mut shutdown_requested = false;

    while let Some(message) = read_message(&mut input) {
        let Some(method) = message.get("method").and_then(|m| m.as_str()) else { continue };
        let id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        match method {
            "initialize" => reply(&mut output, id, json!({
                "capabilities": {
                    "textDocumentSync": 1,          // full text on every change
                    "documentFormattingProvider": true,
                    "hoverProvider": true,
                    "documentSymbolProvider": true,
                    "definitionProvider": true,
                    "completionProvider": {"triggerCharacters": []},
                },
                "serverInfo": {"name": "uredo", "version": env!("CARGO_PKG_VERSION")},
            })),
            "initialized" => {}
            "shutdown" => {
                shutdown_requested = true;
                reply(&mut output, id, Value::Null);
            }
            "exit" => return if shutdown_requested { 0 } else { 1 },
            "textDocument/didOpen" => {
                let uri = uri_of(&params);
                let text = params["textDocument"]["text"].as_str().unwrap_or("").to_string();
                update(&mut docs, &uri, text);
                publish(&mut output, &uri, &docs[&uri].text);
            }
            "textDocument/didChange" => {
                let uri = uri_of(&params);
                // full sync: the last change carries the whole document
                if let Some(text) = params["contentChanges"].as_array().and_then(|c| c.last()).and_then(|c| c["text"].as_str()) {
                    update(&mut docs, &uri, text.to_string());
                }
                publish(&mut output, &uri, docs.get(&uri).map(|d| d.text.as_str()).unwrap_or(""));
            }
            "textDocument/didSave" => {
                let uri = uri_of(&params);
                if let Some(text) = params["text"].as_str() {
                    update(&mut docs, &uri, text.to_string());
                }
                publish(&mut output, &uri, docs.get(&uri).map(|d| d.text.as_str()).unwrap_or(""));
            }
            "textDocument/didClose" => {
                let uri = uri_of(&params);
                docs.remove(&uri);
                // an editor keeps showing the last diagnostics unless they are cleared
                notify(&mut output, "textDocument/publishDiagnostics", json!({"uri": uri, "diagnostics": []}));
            }
            "textDocument/formatting" => {
                let uri = uri_of(&params);
                let text = docs.get(&uri).map(|d| d.text.clone()).unwrap_or_default();
                reply(&mut output, id, formatting_edits(&text));
            }
            "textDocument/documentSymbol" => {
                let uri = uri_of(&params);
                let text = docs.get(&uri).map(|d| d.text.clone()).unwrap_or_default();
                reply(&mut output, id, document_symbols(&text));
            }
            "textDocument/definition" => {
                let uri = uri_of(&params);
                let line = params["position"]["line"].as_u64().unwrap_or(0) as usize + 1;
                let character = params["position"]["character"].as_u64().unwrap_or(0) as usize;
                let text = docs.get(&uri).map(|d| d.text.clone()).unwrap_or_default();
                reply(&mut output, id, definition(&text, &uri, line, character));
            }
            "textDocument/completion" => {
                let uri = uri_of(&params);
                let line = params["position"]["line"].as_u64().unwrap_or(0) as usize + 1;
                let character = params["position"]["character"].as_u64().unwrap_or(0) as usize;
                let text = docs.get(&uri).map(|d| d.text.clone()).unwrap_or_default();
                reply(&mut output, id, completions(&text, &uri, line, character));
            }
            "textDocument/hover" => {
                let uri = uri_of(&params);
                let line = params["position"]["line"].as_u64().unwrap_or(0) as usize + 1;
                let doc = docs.get(&uri);
                let text = doc.map(|d| d.text.as_str()).unwrap_or("");
                let last_good = doc.and_then(|d| d.last_good.as_deref());
                reply(&mut output, id, hover_tolerant(text, last_good, line));
            }
            _ => {
                // a request we do not answer still needs an answer, or the editor waits forever
                if id.is_some() {
                    reply(&mut output, id, Value::Null);
                }
            }
        }
    }
    0
}

/// Records the new text and, when it compiles, keeps it as the tree to fall back on.
fn update(docs: &mut Documents, uri: &str, text: String) {
    let compiles = !crate::compile(&text, false).has_errors();
    let entry = docs.entry(uri.to_string()).or_default();
    if compiles {
        entry.last_good = Some(text.clone());
    }
    entry.text = text;
}

fn uri_of(params: &Value) -> String {
    params["textDocument"]["uri"].as_str().unwrap_or("").to_string()
}

/// Reads one `Content-Length`-framed JSON-RPC message, or `None` at end of input.
fn read_message(input: &mut impl BufRead) -> Option<Value> {
    let mut length = 0usize;
    loop {
        let mut header = String::new();
        if input.read_line(&mut header).ok()? == 0 {
            return None;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some(v) = header.strip_prefix("Content-Length:") {
            length = v.trim().parse().ok()?;
        }
    }
    let mut body = vec![0u8; length];
    input.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

fn send(output: &mut impl Write, value: Value) {
    let body = value.to_string();
    let _ = write!(output, "Content-Length: {}\r\n\r\n{}", body.len(), body);
    let _ = output.flush();
}

fn reply(output: &mut impl Write, id: Option<Value>, result: Value) {
    let Some(id) = id else { return };
    send(output, json!({"jsonrpc": "2.0", "id": id, "result": result}));
}

fn notify(output: &mut impl Write, method: &str, params: Value) {
    send(output, json!({"jsonrpc": "2.0", "method": method, "params": params}));
}

fn publish(output: &mut impl Write, uri: &str, text: &str) {
    notify(output, "textDocument/publishDiagnostics", json!({"uri": uri, "diagnostics": diagnostics(text)}));
}

/// The compiler's diagnostics and lints, as the editor's protocol wants them. Uredo counts lines
/// and columns from one and LSP from zero, which is the whole of the translation.
pub fn diagnostics(text: &str) -> Vec<Value> {
    let out = crate::compile(text, false);
    let mut all: Vec<Value> = Vec::new();
    for d in out.diags.iter().chain(out.lints.iter()) {
        let line = d.line.saturating_sub(1);
        let col = d.col.saturating_sub(1);
        let severity = match d.level {
            Level::Error => 1,
            Level::Warning => 2,
            Level::Note => 3,
        };
        let mut message = d.msg.clone();
        for n in &d.notes {
            message.push('\n');
            message.push_str(n);
        }
        all.push(json!({
            "range": {"start": {"line": line, "character": col}, "end": {"line": line, "character": col + 1}},
            "severity": severity,
            "source": "uredo",
            "message": message,
        }));
    }
    all
}

/// One edit replacing the whole document, which is what a whole-file formatter produces. A file
/// that does not compile is not formatted — §29's formatter refuses it too, rather than guessing.
pub fn formatting_edits(text: &str) -> Value {
    match crate::fmt::format(text) {
        Ok(formatted) if formatted != text => json!([{
            "range": {"start": {"line": 0, "character": 0}, "end": {"line": text.lines().count() + 1, "character": 0}},
            "newText": formatted,
        }]),
        _ => json!([]),
    }
}

/// What `uredo explain` says about a line (§26.2): the elaboration, the rule behind it, and what
/// Uredo inserted. On a line Uredo did not touch, the answer is that it did not touch it.
/// Hover on a document that may not compile.
///
/// Three answers, in the order they are worth having. While the file compiles, this is `hover`.
/// While it does not, the parser has still recovered every item but the broken one, so the
/// elaborations of *those* items are current and are used — that is §28.2's tolerance, and the only
/// cost is a note saying the file has an error in it. Only when even that yields nothing does the
/// last text that compiled answer, and only for a line unchanged since, because an answer about a
/// line the author has rewritten is worse than none.
pub fn hover_tolerant(text: &str, last_good: Option<&str>, line: usize) -> Value {
    let live = hover(text, line);
    if live != Value::Null {
        return live;
    }
    // the file does not compile: answer from the items that still parsed
    if let Some(partial) = hover_from(&crate::compile_tolerant(text), text, line) {
        return note_on(partial, "*this file has an error elsewhere; the rest of it still parsed*");
    }
    let Some(good) = last_good else { return Value::Null };
    let now = text.lines().nth(line.saturating_sub(1)).unwrap_or("").trim();
    let then = good.lines().nth(line.saturating_sub(1)).unwrap_or("").trim();
    if now.is_empty() || now != then {
        return Value::Null;
    }
    match hover(good, line) {
        Value::Null => Value::Null,
        stale => note_on(stale, "*from the last version that compiled — this line is unchanged since*"),
    }
}

/// Adds a line to a hover's markdown saying where the answer came from.
fn note_on(mut hover: Value, note: &str) -> Value {
    if let Some(v) = hover.pointer_mut("/contents/value") {
        let text = v.as_str().unwrap_or("").to_string();
        *v = Value::String(format!("{}\n{}", text, note));
    }
    hover
}

pub fn hover(text: &str, line: usize) -> Value {
    let out = crate::compile(text, false);
    if out.has_errors() {
        return Value::Null;
    }
    hover_from(&out, text, line).unwrap_or(Value::Null)
}

/// The hover for one line of an already-lowered file, or `None` when there is nothing to say —
/// a blank line, or a line the lowering never reached because the parser skipped past it.
fn hover_from(out: &crate::Output, text: &str, line: usize) -> Option<Value> {
    let source_line = text.lines().nth(line.saturating_sub(1)).unwrap_or("").trim();
    if source_line.is_empty() {
        return None;
    }
    let mut md = String::new();
    let elabs: Vec<_> = out.map.elabs.iter().filter(|e| e.line == line).collect();
    if elabs.is_empty() {
        // on a file that did not compile, silence means the parser skipped this line: say nothing
        if out.has_errors() {
            return None;
        }
        md.push_str("**emitted as written** — Uredo elaborated nothing on this line.\n");
    }
    for e in elabs {
        md.push_str(&format!("`{}`\n\nlowers to\n\n```rust\n{}\n```\n\n", e.before.trim(), e.after));
        if !e.rule.is_empty() {
            md.push_str(&format!("- rule: {}\n", e.rule));
        }
        if let Some(d) = &e.declared {
            md.push_str(&format!("- callee declares: `{}`\n", d));
        }
        md.push_str(&format!("- inserted: {}\n\n", e.inserted));
    }
    Some(json!({"contents": {"kind": "markdown", "value": md}}))
}

// ----- the outline, and what it is for (§28.2) -------------------------------------------------
//
// `documentSymbol` and `definition` are built on the *parser* rather than on a full compile, on
// purpose. A file mid-edit usually does not compile, and an outline that empties itself on every
// keystroke is worse than no outline at all. The parser recovers at item boundaries, so every item
// but the broken one is still there — which is the half of §28.2's tolerance that costs nothing.

/// One declaration a file makes: its name, what kind of thing it is, and how far it extends.
struct Decl {
    name: String,
    /// An LSP `SymbolKind`.
    kind: u64,
    /// The declaration as written, which is the most honest signature available.
    detail: String,
    /// 1-based, as Uredo counts.
    line: usize,
    end_line: usize,
    children: Vec<Decl>,
}

/// The last line belonging to a block that starts at `start`.
///
/// Uredo's layout answers this without the parser: a body is the lines indented under its header,
/// so the block ends where the indentation returns to the header's own level.
fn extent(lines: &[&str], start: usize) -> usize {
    let Some(first) = lines.get(start.saturating_sub(1)) else { return start };
    let indent = first.len() - first.trim_start().len();
    let mut end = start;
    for (i, l) in lines.iter().enumerate().skip(start) {
        if l.trim().is_empty() {
            continue;
        }
        if l.len() - l.trim_start().len() <= indent {
            break;
        }
        end = i + 1;
    }
    end
}

/// Where `name` sits on `line`, as a zero-based character offset and a width.
fn name_span(line: &str, name: &str) -> (usize, usize) {
    let bytes = line.as_bytes();
    let n = name.len();
    let word = |i: usize| {
        let before = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
        let after = i + n >= bytes.len() || !(bytes[i + n].is_ascii_alphanumeric() || bytes[i + n] == b'_');
        before && after
    };
    let mut from = 0;
    while let Some(rel) = line[from..].find(name) {
        let at = from + rel;
        if word(at) {
            return (line[..at].chars().count(), name.chars().count());
        }
        from = at + 1;
    }
    (0, line.trim_end().chars().count().max(1))
}

fn range(start_line: usize, start_col: usize, end_line: usize, end_col: usize) -> Value {
    json!({
        "start": {"line": start_line.saturating_sub(1), "character": start_col},
        "end": {"line": end_line.saturating_sub(1), "character": end_col},
    })
}

/// The declaration line, as its own best description: `pub async fn serve(…) throws Error`.
fn detail_of(lines: &[&str], line: usize) -> String {
    let text = lines.get(line.saturating_sub(1)).unwrap_or(&"").trim();
    let text = text.strip_suffix(':').unwrap_or(text);
    if text.chars().count() > 90 { text.chars().take(87).collect::<String>() + "..." } else { text.to_string() }
}

/// Every declaration in a file, nested as the source nests them.
fn outline(text: &str) -> Vec<Decl> {
    let lines: Vec<&str> = text.lines().collect();
    let (toks, _) = crate::lexer::lex(text);
    let (module, _) = crate::parser::parse(text, toks);
    module.items.iter().filter_map(|i| decl_of(i, &lines, false)).collect()
}

fn decl_of(item: &crate::ast::Item, lines: &[&str], in_impl: bool) -> Option<Decl> {
    use crate::ast::{ItemKind, StructKind};
    let line = item.line;
    let end_line = extent(lines, line);
    let detail = detail_of(lines, line);
    let mk = |name: String, kind: u64, children: Vec<Decl>| Decl { name, kind, detail: detail.clone(), line, end_line, children };
    Some(match &item.kind {
        // 12 is Function and 6 is Method; the distinction is what an editor's outline indents.
        ItemKind::Fn(f) => mk(f.name.clone(), if in_impl { 6 } else { 12 }, Vec::new()),
        ItemKind::Const { name, .. } => mk(name.clone(), 14, Vec::new()),
        ItemKind::Struct(s) => {
            let mut kids: Vec<Decl> = Vec::new();
            if let StructKind::Named(fields) = &s.kind {
                for f in fields {
                    kids.push(Decl { name: f.name.clone(), kind: 8, detail: detail_of(lines, f.line), line: f.line, end_line: f.line, children: Vec::new() });
                }
            }
            kids.extend(nested_decls(&s.nested, lines));
            mk(s.name.clone(), 23, kids)
        }
        ItemKind::Enum(e) => {
            let mut kids: Vec<Decl> = e.variants
                .iter()
                .map(|v| Decl {
                    name: v.text.split(['(', '{', ' ']).next().unwrap_or(&v.text).trim().to_string(),
                    kind: 22,
                    detail: detail_of(lines, v.line),
                    line: v.line,
                    end_line: v.line,
                    children: Vec::new(),
                })
                .collect();
            kids.extend(nested_decls(&e.nested, lines));
            mk(e.name.clone(), 10, kids)
        }
        ItemKind::Impl(i) => {
            let children = i.items.iter().filter_map(|x| decl_of(x, lines, true)).collect();
            match i.trait_path.as_deref() {
                // A `trait` declaration has no ItemKind of its own: `parse_trait` returns an
                // `Impl` whose trait_path carries a `trait ` marker and whose self_ty is the
                // supertrait list. Formatting that as an impl produced the outline entry
                // `trait Field for : Serialize + DeserializeOwned`.
                Some(t) if t.starts_with("trait ") => mk(t["trait ".len()..].to_string(), 11, children),
                Some(t) => mk(format!("{} for {}", t, i.self_ty), 19, children),
                None => mk(i.self_ty.clone(), 19, children),
            }
        }
        ItemKind::Mod { name, body } => {
            let kids = body.as_ref().map(|items| items.iter().filter_map(|x| decl_of(x, lines, false)).collect()).unwrap_or_default();
            mk(name.clone(), 2, kids)
        }
        ItemKind::TypeAlias { name, .. } => mk(name.clone(), 5, Vec::new()),
        ItemKind::MacroRules { name, .. } => mk(format!("{}!", name), 12, Vec::new()),
        // A `use`, a `rust { }` block and a macro invocation declare nothing to jump to.
        ItemKind::Use { .. } | ItemKind::Rust(_) | ItemKind::Macro { .. } => return None,
    })
}

/// Methods and trait impls written inside a `struct` or `enum` body (§12's nesting sugar).
fn nested_decls(nested: &[crate::ast::Nested], lines: &[&str]) -> Vec<Decl> {
    use crate::ast::Nested;
    let mut out = Vec::new();
    for n in nested {
        match n {
            Nested::Method(item) => out.extend(decl_of(item, lines, true)),
            Nested::TraitImpl { trait_path, items, line, .. } => out.push(Decl {
                name: trait_path.clone(),
                kind: 11,
                detail: detail_of(lines, *line),
                line: *line,
                end_line: extent(lines, *line),
                children: items.iter().filter_map(|x| decl_of(x, lines, true)).collect(),
            }),
        }
    }
    out
}

/// `textDocument/documentSymbol`: the outline an editor shows in its breadcrumbs and symbol picker.
pub fn document_symbols(text: &str) -> Value {
    let lines: Vec<&str> = text.lines().collect();
    Value::Array(outline(text).iter().map(|d| symbol_json(d, &lines)).collect())
}

fn symbol_json(d: &Decl, lines: &[&str]) -> Value {
    let src = lines.get(d.line.saturating_sub(1)).copied().unwrap_or("");
    let (col, width) = name_span(src, &d.name);
    let end_col = lines.get(d.end_line.saturating_sub(1)).map(|l| l.chars().count()).unwrap_or(0);
    json!({
        "name": d.name,
        "detail": d.detail,
        "kind": d.kind,
        "range": range(d.line, 0, d.end_line, end_col),
        "selectionRange": range(d.line, col, d.line, col + width),
        "children": d.children.iter().map(|c| symbol_json(c, lines)).collect::<Vec<_>>(),
    })
}

/// The identifier under a cursor, if there is one.
fn word_at(text: &str, line: usize, character: usize) -> Option<String> {
    let src = text.lines().nth(line.saturating_sub(1))?;
    let chars: Vec<char> = src.chars().collect();
    if chars.is_empty() {
        return None;
    }
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    // A cursor sitting just past the end of a word still means that word, which is where it lands
    // after double-clicking or after typing the name.
    let at = character.min(chars.len() - 1);
    let at = if ident(chars[at]) { at } else if at > 0 && ident(chars[at - 1]) { at - 1 } else { return None };
    let mut start = at;
    while start > 0 && ident(chars[start - 1]) {
        start -= 1;
    }
    let mut end = at;
    while end + 1 < chars.len() && ident(chars[end + 1]) {
        end += 1;
    }
    Some(chars[start..=end].iter().collect())
}

/// The first declaration called `name`, searching nested declarations too.
fn find_decl<'a>(decls: &'a [Decl], name: &str) -> Option<&'a Decl> {
    for d in decls {
        if d.name == name || d.name.trim_end_matches('!') == name {
            return Some(d);
        }
        if let Some(found) = find_decl(&d.children, name) {
            return Some(found);
        }
    }
    None
}

fn location(uri: &str, d: &Decl, lines: &[&str]) -> Value {
    let src = lines.get(d.line.saturating_sub(1)).copied().unwrap_or("");
    let (col, width) = name_span(src, &d.name);
    json!({"uri": uri, "range": range(d.line, col, d.line, col + width)})
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn uri_to_path(uri: &str) -> Option<std::path::PathBuf> {
    Some(std::path::PathBuf::from(percent_decode(uri.strip_prefix("file://")?)))
}

fn path_to_uri(path: &std::path::Path) -> String {
    let mut out = String::from("file://");
    for c in path.to_string_lossy().chars() {
        match c {
            ' ' => out.push_str("%20"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            _ => out.push(c),
        }
    }
    out
}

/// The other `.ure` files beside this one.
///
/// §20.3 makes a file a module, so a name the current file does not declare is most often declared
/// by a sibling. The server has no project model and does not need one for this: the directory is
/// the module list.
fn siblings(uri: &str) -> Vec<(std::path::PathBuf, String)> {
    let Some(path) = uri_to_path(uri) else { return Vec::new() };
    let Some(dir) = path.parent() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<(std::path::PathBuf, String)> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p != &path && p.extension().map(|x| x == "ure").unwrap_or(false))
        .filter_map(|p| std::fs::read_to_string(&p).ok().map(|t| (p, t)))
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// `textDocument/definition`: where the name under the cursor is declared.
///
/// Item-level only — a function, a type, a constant, a field, a variant, a module. A local binding
/// would need scope tracking the parser does not keep, and answering the wrong `x` would be worse
/// than answering nothing, so a name that is not an item gets no answer rather than a guess.
pub fn definition(text: &str, uri: &str, line: usize, character: usize) -> Value {
    let Some(word) = word_at(text, line, character) else { return Value::Null };
    let here = outline(text);
    if let Some(d) = find_decl(&here, &word) {
        let lines: Vec<&str> = text.lines().collect();
        return location(uri, d, &lines);
    }
    for (path, source) in siblings(uri) {
        let decls = outline(&source);
        if let Some(d) = find_decl(&decls, &word) {
            let lines: Vec<&str> = source.lines().collect();
            return location(&path_to_uri(&path), d, &lines);
        }
    }
    Value::Null
}

/// The words Uredo reserves, for completion. `ITEM_KEYWORDS` plus the ones that only appear inside
/// a body, which the parser recognises positionally rather than from a list.
const COMPLETION_KEYWORDS: &[&str] = &[
    "fn", "struct", "enum", "impl", "trait", "use", "const", "static", "mod", "type", "pub", "async", "macro_rules",
    "take", "inout", "var", "let", "throws", "throw", "return", "if", "else", "match", "for", "in", "while", "loop",
    "break", "continue", "self", "Self", "rust", "unsafe", "where", "as", "true", "false",
];

/// What `textDocument/completion` must not answer.
///
/// After `.` or `::` the useful answer is a member of whatever is on the left, and Uredo has no
/// type engine to ask (§4.4) — so there is nothing to offer that would be *right*. Offering every
/// top-level name instead would put `struct` and `main` in a list where the author wanted the
/// fields of one value, which is the kind of suggestion that teaches people to stop reading the
/// list. Returning nothing lets the editor fall back to its own word matching, which is honest
/// about what it knows.
fn after_a_selector(line: &str, character: usize) -> bool {
    let before: String = line.chars().take(character).collect();
    let trimmed = before.trim_end_matches(|c: char| c.is_alphanumeric() || c == '_');
    trimmed.ends_with('.') || trimmed.ends_with("::")
}

/// `textDocument/completion`: the names in scope, which here means the items this file and its
/// siblings declare, plus the keywords.
///
/// Deliberately not ranked, scored or filtered: the editor does that against what has been typed,
/// and a server that pre-filters on a stale prefix fights it.
pub fn completions(text: &str, uri: &str, line: usize, character: usize) -> Value {
    let current = text.lines().nth(line.saturating_sub(1)).unwrap_or("");
    if after_a_selector(current, character) {
        return json!({"isIncomplete": false, "items": []});
    }
    let mut items: Vec<Value> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut offer = |d: &Decl, from: Option<&str>| {
        // Fields and methods are reachable only through a receiver, and the branch above is exactly
        // where a receiver would be known. Offering them here would be offering them everywhere.
        let kind = match d.kind {
            12 => 3,  // Function
            23 => 22, // Struct
            10 => 13, // Enum
            14 => 21, // Constant
            5 => 7,   // a type alias, as a Class
            11 => 8,  // Interface, which is what a trait is offered as
            2 => 9,   // Module
            _ => return,
        };
        if !seen.insert(d.name.clone()) {
            return;
        }
        let mut item = json!({"label": d.name, "kind": kind, "detail": d.detail});
        if let Some(module) = from {
            item["labelDetails"] = json!({"description": module});
        }
        items.push(item);
    };

    for d in &outline(text) {
        offer(d, None);
    }
    for (path, source) in siblings(uri) {
        let module = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        for d in &outline(&source) {
            offer(d, Some(&module));
        }
    }
    for kw in COMPLETION_KEYWORDS {
        if seen.insert((*kw).to_string()) {
            items.push(json!({"label": kw, "kind": 14}));
        }
    }
    json!({"isIncomplete": false, "items": items})
}
