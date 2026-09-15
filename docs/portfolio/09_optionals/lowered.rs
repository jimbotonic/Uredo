//! 09 — Optional values: `T?` is `Option<T>`; there is no null and no truthiness.

#[derive(Debug, Clone)]
struct User {
    name: String,
    email: ::core::option::Option<String>,
}

/// `T?` in a return type; `None`/`Some` keep Rust's names.
fn find(users: &[User], name: &str) -> ::core::option::Option<User> {
    for u in users {
        if u.name == name {
            return Some(u.clone());
        }
    }
    None
}

/// `?` works on `Option` in an `Option`-returning function (D30): `None` propagates.
fn email_domain(users: &[User], name: &str) -> ::core::option::Option<String> {
    let user = find(users, name)?;
    let email = user.email?;
    let domain = email.split('@').nth(1)?;
    Some(domain.to_string())
}

fn main() {
    let users = Vec::from([
        User {
            name: String::from("ada"),
            email: Some(String::from("ada@example.org")),
        },
        User {
            name: String::from("bob"),
            email: None,
        },
    ]);

    ::std::println!("{:?}", email_domain(&users, "ada"));
    ::std::println!("{:?}", email_domain(&users, "bob"));
    ::std::println!("{:?}", email_domain(&users, "cy"));

    let first = users.first();
    if let Some(u) = first {
        ::std::println!("first user {}", u.name);
    }

    let nick = find(&users, "zed")
        .map(|u| u.name.to_uppercase())
        .unwrap_or(String::from("nobody"));
    ::std::println!("{}", nick);

    ::std::println!("{}", describe(&users[1].email));
    ::std::println!("{} {}", score_band(Some(72)), score_band(None));
    ::std::println!("{}", initial(users.first().map(|u| &u.name)));
}

fn describe(email: &::core::option::Option<String>) -> &'static str {
    match email {
        Some(_) => "has email",
        None => "no email",
    }
}

/// `u32` is known-Copy, so `Option<u32>` is too: the parameter passes by value (P1, D54). Nothing
/// moves — a copy is not a transfer — so no `take` is needed and none is written.
fn score_band(score: ::core::option::Option<u32>) -> &'static str {
    match score {
        Some(s) if s >= 80 => "high",
        Some(_) => "low",
        None => "unscored",
    }
}

/// `&T?` is `Option<&T>` (D54), the shape Rust reaches for when it borrows an optional's payload;
/// a shared reference is known-Copy, so this one passes by value too.
fn initial(name: ::core::option::Option<&String>) -> char {
    name.and_then(|n| n.chars().next()).unwrap_or('?')
}
