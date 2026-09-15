//! 04 — Text and sequences: `str` vs `String`, arrays vs `Vec`, slices.
//! The rule that organises all of it: nothing allocates unless the source says so (§3.2).

/// `str` in a parameter is `&str` (rule P2): borrowed text, no allocation.
fn shout(s: &str) -> String {
    s.to_uppercase()
}

/// `[T]` in a parameter is `&[T]` (rule P2): a borrowed slice of any length.
fn total(values: &[i32]) -> i32 {
    values.iter().sum::<i32>()
}

fn main() {
    let literal = "borrowed text";
    let owned: String = String::from("owned text");
    let also_owned = literal.to_string();
    let joined = ::std::format!("{literal} / {owned} / {also_owned}");
    ::std::println!("{}", joined);
    ::std::println!("{}", shout(&literal));
    ::std::println!("{}", shout(&owned));

    let bytes = [1, 2, 3, 4];
    let numbers = Vec::from([10, 20, 30]);
    let more = vec![1, 2, 3];
    let typed: Vec<u8> = Vec::from([7, 8, 9]);
    ::std::println!(
        "{} {} {} {}",
        total(&bytes),
        total(&numbers),
        total(&more),
        typed.len()
    );

    let head = &numbers[..2];
    let last = numbers[numbers.len() - 1];
    ::std::println!("{head:?} {last}");

    for c in "héllo".chars() {
        ::std::println!("{c} {}", c.len_utf8());
    }
}
