//! 8. JSON with Serde: derives, parsing, modifying and re-serialising; attribute forwarding.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Task {
    id: u32,
    title: String,
    #[serde(default)]
    done: bool,
    tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Board {
    name: String,
    tasks: Vec<Task>,
}

fn open_titles(board: &Board) -> Vec<String> {
    board
        .tasks
        .iter()
        .filter(|t| !t.done)
        .map(|t| t.title.clone())
        .collect()
}

fn main() -> Result<(), serde_json::Error> {
    let input = r#"{"name": "launch", "tasks": [
        {"id": 1, "title": "write spec", "done": true, "tags": ["docs"]},
        {"id": 2, "title": "ship compiler", "tags": ["code", "urgent"]}
    ]}"#;
    let mut board: Board = serde_json::from_str(input)?;
    println!("{} tasks on {:?}", board.tasks.len(), board.name);
    println!("open: {:?}", open_titles(&board));
    board.tasks.push(Task {
        id: 3,
        title: String::from("write corpus"),
        done: false,
        tags: Vec::new(),
    });
    for task in &mut board.tasks {
        if task.tags.iter().any(|t| t == "urgent") {
            task.done = true;
        }
    }
    let out = serde_json::to_string_pretty(&board)?;
    println!("{out}");
    let value: serde_json::Value = serde_json::from_str(&out)?;
    println!("first title: {}", value["tasks"][0]["title"]);
    Ok(())
}
