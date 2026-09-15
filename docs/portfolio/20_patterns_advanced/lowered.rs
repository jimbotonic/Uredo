//! 20 — Pattern matching, the full toolbox: nested patterns, `@` bindings, ranges,
//! slice patterns, `ref`/`ref mut`, or-patterns, and matching on tuples and references.
//! Every pattern here is Rust's (§13.1); Uredo only changes the arm punctuation.

#[derive(Debug)]
enum Token {
    Num(i64),
    Ident(String),
    Punct(char),
}

/// Slice patterns classify a token stream by shape.
fn shape(tokens: &[Token]) -> String {
    match tokens {
        [] => String::from("empty"),
        [Token::Num(n)] => ::std::format!("just {n}"),
        [Token::Ident(name), Token::Punct('='), rest @ ..] => {
            ::std::format!("assign {name} <- {} tokens", rest.len())
        }
        [first, .., last] => ::std::format!("{first:?} … {last:?}"),
        [single] => ::std::format!("single {single:?}"),
    }
}

/// `@` binds the whole while testing a sub-pattern; ranges and or-patterns on chars.
fn classify(c: char) -> &'static str {
    match c {
        'a'..='z' | 'A'..='Z' => "letter",
        d @ '0'..='9' if d != '0' => "nonzero digit",
        '0' => "zero",
        _ => "other",
    }
}

/// Matching a reference: `&(a, b)` destructures through the borrow; `ref` avoids moving.
fn sum_pairs(pairs: &[(i32, i32)]) -> i32 {
    let mut total = 0;
    for &(a, b) in pairs {
        total += a * b;
    }
    total
}

#[derive(Debug)]
struct Config {
    name: String,
    retries: u32,
    verbose: bool,
}

fn tune(cfg: &mut Config) {
    match *cfg {
        Config {
            ref mut retries,
            verbose: true,
            ..
        } => *retries += 1,
        Config { ref name, .. } => ::std::println!("quiet config {name}"),
    }
}

fn main() {
    let toks = Vec::from([
        Token::Ident(String::from("x")),
        Token::Punct('='),
        Token::Num(4),
        Token::Num(2),
    ]);
    ::std::println!("{}", shape(&toks));
    ::std::println!("{}", shape(&toks[2..]));
    ::std::println!("{}", shape(&toks[..0]));
    for c in ['q', '7', '0', '!'] {
        ::std::println!("{c}: {}", classify(c));
    }
    ::std::println!("{}", sum_pairs(&[(1, 2), (3, 4)]));
    let mut cfg = Config {
        name: String::from("dev"),
        retries: 1,
        verbose: true,
    };
    tune(&mut cfg);
    cfg.verbose = false;
    tune(&mut cfg);
    ::std::println!("{cfg:?}");

    let inputs: Vec<(u8, ::core::option::Option<Result<u8, String>>)> = Vec::from([
        (1, None),
        (2, Some(Ok(9))),
        (3, Some(Err(String::from("x")))),
    ]);
    for pair in &inputs {
        match pair {
            (id, None) => ::std::println!("{id}: nothing"),
            (id, Some(Ok(v))) if *v > 5 => ::std::println!("{id}: big {v}"),
            (id, Some(Ok(v))) => ::std::println!("{id}: {v}"),
            (id, Some(Err(e))) => ::std::println!("{id}: error {e}"),
        }
    }
}
