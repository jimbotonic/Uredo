//! `uredo lsp` (§28.1): the server an editor talks to, driven the way an editor drives it —
//! `Content-Length` framing over stdio, not the library functions behind it.

use std::io::Write;
use std::process::{Command, Stdio};

fn frame(body: &str) -> String {
    format!("Content-Length: {}\r\n\r\n{}", body.len(), body)
}

/// Runs a whole session and returns the messages the server sent back.
fn session(messages: &[&str]) -> (i32, Vec<serde_json::Value>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_uredo"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start the server");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        for m in messages {
            stdin.write_all(frame(m).as_bytes()).expect("write");
        }
    }
    let out = child.wait_with_output().expect("wait");
    let mut rest = out.stdout.as_slice();
    let mut got = Vec::new();
    while let Some(i) = rest.windows(4).position(|w| w == b"\r\n\r\n") {
        let header = String::from_utf8_lossy(&rest[..i]).to_string();
        let n: usize = header.split("Content-Length:").nth(1).expect("length").trim().parse().expect("number");
        got.push(serde_json::from_slice(&rest[i + 4..i + 4 + n]).expect("json"));
        rest = &rest[i + 4 + n..];
    }
    (out.status.code().unwrap_or(-1), got)
}

const SRC: &str = "fn greet(name: str) -> String:\\n    format(\\\"hello, {name}\\\")\\n\\nfn dup(x: take u32) -> u32:\\n    x\\n\\nfn main():\\n    who = String::from(\\\"world\\\")\\n    print(\\\"{}\\\", greet(who))\\n";

#[test]
fn the_server_answers_an_editors_whole_session() {
    let (code, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///t.ure","text":"{}"}}}}}}"#, SRC),
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///t.ure"},"position":{"line":8,"character":18}}}"#,
        r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///t.ure"},"contentChanges":[{"text":"fn main():\n    x = = 1\n"}]}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    assert_eq!(code, 0, "a server that was asked to shut down exits 0");

    // initialize names what it can do
    let caps = &got[0]["result"]["capabilities"];
    assert_eq!(caps["hoverProvider"], true);
    assert_eq!(caps["documentFormattingProvider"], true);
    assert_eq!(caps["textDocumentSync"], 1);

    let published: Vec<&serde_json::Value> = got.iter().filter(|m| m["method"] == "textDocument/publishDiagnostics").collect();
    assert_eq!(published.len(), 2, "one on open, one on change");

    // the lint reaches the editor as a warning, on the line it is about (LSP counts from zero)
    let first = published[0]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(first.len(), 1, "{:?}", first);
    assert_eq!(first[0]["severity"], 2);
    assert_eq!(first[0]["range"]["start"]["line"], 3);
    assert!(first[0]["message"].as_str().unwrap().contains("redundant_take"), "{:?}", first[0]);

    // hover is what `uredo explain` says: the elaboration and the rule behind it
    let hover = got.iter().find(|m| m["id"] == 2).expect("a hover reply");
    let text = hover["result"]["contents"]["value"].as_str().expect("markdown");
    assert!(text.contains("greet(&who)"), "{}", text);
    assert!(text.contains("shared borrow inserted"), "{}", text);

    // a file that stops compiling reports the parse error, as an error
    let second = published[1]["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(second[0]["severity"], 1, "{:?}", second);
    assert_eq!(second[0]["range"]["start"]["line"], 1, "{:?}", second);
}

#[test]
fn closing_a_document_clears_what_the_editor_is_showing() {
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{"uri":"file:///c.ure","text":"fn e(x: take u32) -> u32:\n    x\n"}}}"#,
        r#"{"jsonrpc":"2.0","method":"textDocument/didClose","params":{"textDocument":{"uri":"file:///c.ure"}}}"#,
        r#"{"jsonrpc":"2.0","id":2,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    let published: Vec<&serde_json::Value> = got.iter().filter(|m| m["method"] == "textDocument/publishDiagnostics").collect();
    assert_eq!(published.len(), 2);
    assert_eq!(published[0]["params"]["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(published[1]["params"]["diagnostics"].as_array().unwrap().len(), 0, "a closed file leaves nothing behind");
}

#[test]
fn formatting_returns_one_edit_and_refuses_a_file_that_does_not_compile() {
    // messy but legal: the formatter has something to do
    let messy = "fn main():\\n    x    =   1\\n    print(\\\"{x}\\\")\\n";
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///f.ure","text":"{}"}}}}}}"#, messy),
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/formatting","params":{"textDocument":{"uri":"file:///f.ure"},"options":{}}}"#,
        r#"{"jsonrpc":"2.0","method":"textDocument/didChange","params":{"textDocument":{"uri":"file:///f.ure"},"contentChanges":[{"text":"fn main():\n    x = = 1\n"}]}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/formatting","params":{"textDocument":{"uri":"file:///f.ure"},"options":{}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    let edits = got.iter().find(|m| m["id"] == 2).expect("a formatting reply")["result"].as_array().unwrap().clone();
    assert_eq!(edits.len(), 1, "one edit replacing the document");
    assert!(edits[0]["newText"].as_str().unwrap().contains("x = 1"), "{:?}", edits[0]);

    let none = got.iter().find(|m| m["id"] == 3).expect("a second formatting reply")["result"].as_array().unwrap().clone();
    assert!(none.is_empty(), "a file that does not compile is not formatted: {:?}", none);
}

#[test]
fn an_unknown_request_still_gets_an_answer() {
    // a request left unanswered hangs the editor, so every `id` gets a reply even when the answer
    // is nothing
    let (code, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":7,"method":"textDocument/completion","params":{}}"#,
        r#"{"jsonrpc":"2.0","id":8,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    assert_eq!(code, 0);
    assert!(got.iter().any(|m| m["id"] == 7), "no reply to an unsupported request: {:?}", got);
}

#[test]
fn exiting_without_a_shutdown_is_an_error_exit() {
    let (code, _) = session(&[r#"{"jsonrpc":"2.0","method":"exit"}"#]);
    assert_eq!(code, 1, "the protocol asks for a non-zero exit when `exit` arrives before `shutdown`");
}

#[test]
fn hover_survives_a_keystroke_that_breaks_the_file() {
    // §28.2 asks the server to keep working on a file that does not compile. It does not have the
    // tolerant parser that would answer everything; what it does have is the last tree that
    // compiled, used only for a line the author has not touched since.
    let good = "fn greet(name: str) -> String:\\n    format(\\\"hello, {name}\\\")\\n\\nfn main():\\n    who = String::from(\\\"world\\\")\\n    print(\\\"{}\\\", greet(who))\\n";
    // the same file with line 2 half-typed: it no longer parses
    let broken = "fn greet(name: str) -> String:\\n    format(\\\"hello, {name}\\\"\\n\\nfn main():\\n    who = String::from(\\\"world\\\")\\n    print(\\\"{}\\\", greet(who))\\n";
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///k.ure","text":"{}"}}}}}}"#, good),
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didChange","params":{{"textDocument":{{"uri":"file:///k.ure"}},"contentChanges":[{{"text":"{}"}}]}}}}"#, broken),
        // line 6 (0-based 5) is untouched: the answer comes from the last good tree, marked stale
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///k.ure"},"position":{"line":5,"character":18}}}"#,
        // line 2 (0-based 1) is the one being typed: no answer is better than a wrong one
        r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///k.ure"},"position":{"line":1,"character":10}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    // the broken text still reports its error
    let published: Vec<&serde_json::Value> = got.iter().filter(|m| m["method"] == "textDocument/publishDiagnostics").collect();
    let latest = published.last().unwrap()["params"]["diagnostics"].as_array().unwrap();
    assert_eq!(latest[0]["severity"], 1, "{:?}", latest);

    // the parser recovered `fn main`, so the answer is *current* — not the last-good fallback —
    // and says the file has an error somewhere else
    let unchanged = got.iter().find(|m| m["id"] == 2).expect("a hover reply");
    let text = unchanged["result"]["contents"]["value"].as_str().expect("markdown on an unchanged line");
    assert!(text.contains("greet(&who)"), "{}", text);
    assert!(text.contains("error elsewhere"), "the answer should say the file is broken elsewhere:\n{}", text);
    assert!(!text.contains("last version that compiled"), "no need for the stale path here:\n{}", text);

    let edited = got.iter().find(|m| m["id"] == 3).expect("a hover reply");
    assert!(edited["result"].is_null(), "a line being typed gets no answer: {:?}", edited["result"]);
}

#[test]
fn the_last_good_tree_answers_when_even_a_partial_parse_cannot() {
    // The item *containing* the line is the broken one, so no partial parse reaches it. The answer
    // then comes from the last text that compiled, marked stale, and only because the line itself
    // is unchanged.
    let good = "fn greet(name: str) -> String:\\n    format(\\\"hello, {name}\\\")\\n\\nfn main():\\n    who = String::from(\\\"world\\\")\\n    print(\\\"{}\\\", greet(who))\\n";
    // `main` loses its header, so nothing in it parses; line 6 is untouched
    let broken = "fn greet(name: str) -> String:\\n    format(\\\"hello, {name}\\\")\\n\\nfn ():\\n    who = String::from(\\\"world\\\")\\n    print(\\\"{}\\\", greet(who))\\n";
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///s.ure","text":"{}"}}}}}}"#, good),
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didChange","params":{{"textDocument":{{"uri":"file:///s.ure"}},"contentChanges":[{{"text":"{}"}}]}}}}"#, broken),
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{"textDocument":{"uri":"file:///s.ure"},"position":{"line":5,"character":18}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    let reply = got.iter().find(|m| m["id"] == 2).expect("a hover reply");
    let text = reply["result"]["contents"]["value"].as_str().expect("markdown");
    assert!(text.contains("greet(&who)"), "{}", text);
    assert!(text.contains("last version that compiled"), "this one must be marked stale:\n{}", text);
}

// ----- the outline and jump-to-definition (§28.2) ----------------------------------------------

const TYPES: &str = "##! The types.\\n\\npub struct Item:\\n    id: u64\\n    name: String\\n\\n    fn label(self) -> String:\\n        format(\\\"{}\\\", self.id)\\n\\npub enum Status:\\n    Open\\n    Closed(String)\\n";

/// The outline an editor shows in its breadcrumbs and its symbol picker.
#[test]
fn the_outline_nests_fields_methods_and_variants() {
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///o.ure","text":"{}"}}}}}}"#, TYPES),
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///o.ure"}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    assert_eq!(got[0]["result"]["capabilities"]["documentSymbolProvider"], true);
    let symbols = got.iter().find(|m| m["id"] == 2).expect("a documentSymbol reply")["result"].as_array().unwrap().clone();
    assert_eq!(symbols.len(), 2, "a struct and an enum, and not the `##!` doc: {:?}", symbols);

    let item = &symbols[0];
    assert_eq!(item["name"], "Item");
    assert_eq!(item["kind"], 23, "SymbolKind::Struct");
    assert_eq!(item["detail"], "pub struct Item", "the declaration is its own best description");
    // the range covers the body; the selection range is the name, so `go to symbol` lands on it
    assert_eq!(item["range"]["start"]["line"], 2);
    assert_eq!(item["range"]["end"]["line"], 7, "through the nested method: {:?}", item["range"]);
    assert_eq!(item["selectionRange"]["start"]["character"], 11, "the `I` of `Item`");

    let kids = item["children"].as_array().unwrap();
    let names: Vec<&str> = kids.iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["id", "name", "label"], "fields then the nested method (§12)");
    assert_eq!(kids[0]["kind"], 8, "SymbolKind::Field");
    assert_eq!(kids[2]["kind"], 6, "a nested method is a Method, not a Function");

    let status = &symbols[1];
    assert_eq!(status["kind"], 10, "SymbolKind::Enum");
    let variants: Vec<&str> = status["children"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(variants, vec!["Open", "Closed"], "the payload is not part of the name");
}

/// The reason this is built on the parser and not on a full compile.
///
/// A file mid-edit usually does not compile, and an outline that empties itself on every keystroke
/// is worse than no outline at all. The parser recovers at item boundaries, so every item but the
/// broken one is still there — §28.2's tolerance, for the price of not asking the lowerer.
#[test]
fn the_outline_survives_a_file_that_does_not_compile() {
    let broken = "fn alpha() -> u32:\\n    1\\n\\nfn beta(x: u32) -> u32:\\n    x +\\n\\nfn gamma() -> u32:\\n    2\\n";
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///b.ure","text":"{}"}}}}}}"#, broken),
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/documentSymbol","params":{"textDocument":{"uri":"file:///b.ure"}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///b.ure"},"position":{"line":6,"character":4}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    let published: Vec<&serde_json::Value> = got.iter().filter(|m| m["method"] == "textDocument/publishDiagnostics").collect();
    assert!(!published[0]["params"]["diagnostics"].as_array().unwrap().is_empty(), "the file really does not compile");

    let symbols = got.iter().find(|m| m["id"] == 2).expect("a documentSymbol reply")["result"].as_array().unwrap().clone();
    let names: Vec<&str> = symbols.iter().map(|s| s["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["alpha", "beta", "gamma"], "every item but the broken one is still there");

    let def = got.iter().find(|m| m["id"] == 3).expect("a definition reply");
    assert_eq!(def["result"]["range"]["start"]["line"], 6, "and definition still answers: {:?}", def["result"]);
}

#[test]
fn definition_finds_an_item_and_declines_to_guess_at_a_local() {
    let src = "const LIMIT: usize = 10\\n\\nfn total() -> usize:\\n    extra = 1\\n    LIMIT + extra\\n";
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///d.ure","text":"{}"}}}}}}"#, src),
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///d.ure"},"position":{"line":4,"character":5}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{"textDocument":{"uri":"file:///d.ure"},"position":{"line":4,"character":14}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    assert_eq!(got[0]["result"]["capabilities"]["definitionProvider"], true);

    let constant = got.iter().find(|m| m["id"] == 2).expect("a definition reply");
    assert_eq!(constant["result"]["uri"], "file:///d.ure");
    assert_eq!(constant["result"]["range"]["start"]["line"], 0, "the `const` on line 1");
    assert_eq!(constant["result"]["range"]["start"]["character"], 6, "the `L` of `LIMIT`");

    // `extra` is a local binding. Tracking scopes is not something the parser keeps, and answering
    // with the wrong `extra` would be worse than answering nothing, so this declines.
    let local = got.iter().find(|m| m["id"] == 3).expect("a definition reply");
    assert!(local["result"].is_null(), "a local should get no answer rather than a guess: {:?}", local["result"]);
}

/// §20.3 makes a file a module, so a name the open file does not declare is most often declared by
/// a sibling. The server has no project model and needs none for this: the directory is the module
/// list. This is the one test that needs files on disk, because that lookup reads them.
#[test]
fn definition_crosses_into_a_sibling_module() {
    let dir = std::env::temp_dir().join(format!("uredo-lsp-def-{}", std::process::id()));
    let src = dir.join("src");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&src).expect("create the package");
    std::fs::write(src.join("model.ure"), "pub struct Item:\n    id: u64\n").expect("write model");
    let main = src.join("main.ure");
    std::fs::write(&main, "use crate::model::Item\n\nfn describe(item: Item) -> u64:\n    item.id\n").expect("write main");

    let uri = format!("file://{}", main.display());
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        // built rather than escaped by hand: a mis-escaped message would still be valid JSON and
        // the test would then pass on a document that is not the one on disk
        &serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {"textDocument": {"uri": uri, "text": std::fs::read_to_string(&main).unwrap()}},
        })
        .to_string(),
        // the cursor sits on `Item` in the parameter list, which this file only imports
        &format!(r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/definition","params":{{"textDocument":{{"uri":"{}"}},"position":{{"line":2,"character":19}}}}}}"#, uri),
        r#"{"jsonrpc":"2.0","id":3,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    let def = got.iter().find(|m| m["id"] == 2).expect("a definition reply")["result"].clone();
    assert!(!def.is_null(), "the sibling declares it: {:?}", def);
    assert!(def["uri"].as_str().unwrap().ends_with("model.ure"), "{:?}", def);
    assert_eq!(def["range"]["start"]["line"], 0, "`pub struct Item` is the first line of the sibling");
    assert_eq!(def["range"]["start"]["character"], 11, "the `I` of `Item`");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Completion offers what it knows and refuses what it would have to guess.
#[test]
fn completion_offers_items_and_keywords_but_nothing_after_a_selector() {
    // A body that parses. A function whose body is still empty does not (§11), so while it is being
    // typed the function itself is not yet an item and is not offered — which is a real limit worth
    // knowing, and not what this test is about.
    let src = "const LIMIT: usize = 10\\n\\nstruct Item:\\n    id: u64\\n\\nfn total(it: Item) -> usize:\\n    LIMIT\\n";
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///k.ure","text":"{}"}}}}}}"#, src),
        // inside a body, not after a selector: everything in scope
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///k.ure"},"position":{"line":6,"character":9}}}"#,
        r#"{"jsonrpc":"2.0","id":3,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    assert!(got[0]["result"]["capabilities"]["completionProvider"].is_object());

    let result = got.iter().find(|m| m["id"] == 2).expect("a completion reply")["result"].clone();
    let items = result["items"].as_array().unwrap();
    let labels: Vec<&str> = items.iter().map(|i| i["label"].as_str().unwrap()).collect();
    assert!(labels.contains(&"LIMIT"), "{:?}", labels);
    assert!(labels.contains(&"Item"), "{:?}", labels);
    assert!(labels.contains(&"total"), "{:?}", labels);
    assert!(labels.contains(&"take"), "the keywords are offered too: {:?}", labels);
    assert!(!labels.contains(&"id"), "a field is only reachable through a receiver: {:?}", labels);

    // kinds, so an editor's icons mean something: Constant, Struct, Function, Keyword
    let kind = |name: &str| items.iter().find(|i| i["label"] == name).unwrap()["kind"].as_u64().unwrap();
    assert_eq!(kind("LIMIT"), 21);
    assert_eq!(kind("Item"), 22);
    assert_eq!(kind("total"), 3);
    assert_eq!(kind("take"), 14);
}

/// The refusal is the feature. After `.` or `::` the useful answer is a member of whatever is on
/// the left, and Uredo has no type engine to ask (§4.4). Offering every top-level name instead
/// teaches people to stop reading the list.
#[test]
fn completion_says_nothing_where_it_would_have_to_guess() {
    let src = "struct Item:\\n    id: u64\\n\\nfn total(it: Item) -> usize:\\n    it.\\n";
    let (_, got) = session(&[
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"file:///s.ure","text":"{}"}}}}}}"#, src),
        // the cursor is just after `it.`
        r#"{"jsonrpc":"2.0","id":2,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///s.ure"},"position":{"line":4,"character":7}}}"#,
        // and just after `it.i`, part-way through a member name
        r#"{"jsonrpc":"2.0","id":3,"method":"textDocument/completion","params":{"textDocument":{"uri":"file:///s.ure"},"position":{"line":4,"character":8}}}"#,
        r#"{"jsonrpc":"2.0","id":4,"method":"shutdown"}"#,
        r#"{"jsonrpc":"2.0","method":"exit"}"#,
    ]);
    for id in [2, 3] {
        let items = got.iter().find(|m| m["id"] == id).expect("a completion reply")["result"]["items"].as_array().unwrap().clone();
        assert!(items.is_empty(), "nothing is the right answer after a selector: {:?}", items);
    }
}
