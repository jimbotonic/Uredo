//! `uredo lsp` (§28.1, §28.2): a language server over stdio.
//!
//! It shares the compiler's own front end rather than a second one, which is the property §28.2
//! asks for: the diagnostics an editor shows are the diagnostics `uredo check` prints, on the same
//! Uredo lines, and hover is what `uredo explain` says about that line. What it does *not* yet have
//! is the tolerant, lossless parse §28.2 also asks for — on a file that does not parse, the server
//! reports the parse errors and offers nothing else, where a tolerant parser would keep answering
//! from the last good tree. That limitation is stated here because it is the one an editor user
//! meets first.
//!
//! Supported: `initialize`, `shutdown`/`exit`, full-text sync (`didOpen`, `didChange`, `didSave`,
//! `didClose`), `publishDiagnostics` (errors, warnings and lints), `textDocument/formatting`
//! (§29's formatter, refused on a file that does not compile, as the formatter itself refuses) and
//! `textDocument/hover` (§26.2's elaboration record).

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
