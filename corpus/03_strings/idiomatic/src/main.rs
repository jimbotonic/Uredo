//! 3. String processing: borrowed `str` parameters, owned `String` results, chars and words.

use std::collections::HashMap;

fn is_palindrome(s: &str) -> bool {
    let letters: Vec<char> = s
        .chars()
        .filter(|c| c.is_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    letters.iter().eq(letters.iter().rev())
}

fn word_frequencies(text: &str) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for word in text.split_whitespace() {
        let cleaned: String = word
            .trim_matches(|c| !char::is_alphanumeric(c))
            .to_lowercase();
        if !cleaned.is_empty() {
            *counts.entry(cleaned).or_insert(0) += 1;
        }
    }
    counts
}

fn title_case(s: &str) -> String {
    let mut out = String::new();
    for (i, word) in s.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

fn main() {
    for s in ["A man, a plan, a canal: Panama", "hello"] {
        println!("{s:?} palindrome: {}", is_palindrome(s));
    }
    let text = "the quick brown fox jumps over the lazy dog. The dog sleeps.";
    let freq = word_frequencies(text);
    let mut pairs: Vec<(&String, &usize)> = freq.iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (word, n) in pairs.iter().take(3) {
        println!("{word}: {n}");
    }
    println!("{}", title_case("uredo compiles to readable rust"));
}
