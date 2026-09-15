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
