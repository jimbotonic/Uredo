//! 6. Optional handling: `Option`, `if let`, `let-else`, `?` on Option, combinators.

#[derive(Debug)]
struct Account {
    owner: String,
    email: Option<String>,
}

fn find<'a>(accounts: &'a [Account], owner: &str) -> Option<&'a Account> {
    accounts.iter().find(|a| a.owner == owner)
}

fn email_domain(accounts: &[Account], owner: &str) -> Option<String> {
    let account = find(accounts, owner)?;
    let email = account.email.as_deref()?;
    let domain = email.split('@').nth(1)?;
    Some(domain.to_string())
}

fn describe(email: &Option<String>) -> &'static str {
    match email {
        Some(e) if e.ends_with(".org") => "non-profit",
        Some(_) => "has email",
        None => "no email",
    }
}

fn tier(score: Option<u32>) -> &'static str {
    match score {
        Some(s) if s >= 90 => "gold",
        Some(_) => "member",
        None => "guest",
    }
}

fn initial(name: Option<&String>) -> char {
    name.and_then(|n| n.chars().next()).unwrap_or('?')
}

fn main() {
    let accounts = vec![
        Account {
            owner: String::from("ada"),
            email: Some(String::from("ada@example.org")),
        },
        Account {
            owner: String::from("bob"),
            email: None,
        },
    ];
    for owner in ["ada", "bob", "cy"] {
        println!("{owner}: {:?}", email_domain(&accounts, owner));
    }
    if let Some(a) = find(&accounts, "bob") {
        println!("bob is {}", describe(&a.email));
    }
    let Some(first) = accounts.first() else {
        println!("no accounts");
        return;
    };
    println!("first owner {} ({})", first.owner, describe(&first.email));
    let fallback = email_domain(&accounts, "bob").unwrap_or_else(|| String::from("none"));
    println!("{fallback}");
    let lengths: Vec<usize> = accounts
        .iter()
        .filter_map(|a| a.email.as_ref().map(|e| e.len()))
        .collect();
    println!("{lengths:?}");
    println!("{} {} {}", tier(Some(95)), tier(Some(10)), tier(None));
    println!("{} {}", initial(accounts.first().map(|a| &a.owner)), initial(None));
}
