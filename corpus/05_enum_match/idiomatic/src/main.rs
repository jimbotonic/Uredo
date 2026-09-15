//! 5. Enum and match: variants with data, exhaustive matching, guards, methods on enums.

#[derive(Debug, Clone, PartialEq)]
enum Command {
    Move { dx: i32, dy: i32 },
    Say(String),
    Repeat(u32, Box<Command>),
    Quit,
}

impl Command {
    fn describe(&self) -> String {
        match self {
            Command::Move { dx, dy } if *dx == 0 && *dy == 0 => String::from("stay"),
            Command::Move { dx, dy } => format!("move by ({dx}, {dy})"),
            Command::Say(text) => format!("say {text:?}"),
            Command::Repeat(n, inner) => format!("{n}x {}", inner.describe()),
            Command::Quit => String::from("quit"),
        }
    }
}

fn parse(line: &str) -> Option<Command> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    match parts.as_slice() {
        ["move", dx, dy] => Some(Command::Move {
            dx: dx.parse().ok()?,
            dy: dy.parse().ok()?,
        }),
        ["say", rest @ ..] => Some(Command::Say(rest.join(" "))),
        ["quit"] => Some(Command::Quit),
        _ => None,
    }
}

fn main() {
    let mut commands: Vec<Command> = Vec::new();
    for line in ["move 1 -2", "say hello there", "move 0 0", "dance", "quit"] {
        match parse(line) {
            Some(cmd) => commands.push(cmd),
            None => println!("cannot parse {line:?}"),
        }
    }
    commands.push(Command::Repeat(
        3,
        Box::new(Command::Say(String::from("hi"))),
    ));
    for cmd in &commands {
        println!("{}", cmd.describe());
    }
    let quits = commands.iter().filter(|c| **c == Command::Quit).count();
    println!("{quits} quit command(s)");
}
