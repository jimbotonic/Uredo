//! 7. File reading: write a temporary file, read it back, count lines and words, propagate I/O errors.

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

fn write_sample(path: &Path) -> io::Result<()> {
    let mut file = fs::File::create(path)?;
    for i in 1..=5 {
        writeln!(file, "line {i}: the quick brown fox")?;
    }
    Ok(())
}

fn count_lines_and_words(path: &Path) -> io::Result<(usize, usize)> {
    let reader = BufReader::new(fs::File::open(path)?);
    let mut lines = 0;
    let mut words = 0;
    for line in reader.lines() {
        let line = line?;
        lines += 1;
        words += line.split_whitespace().count();
    }
    Ok((lines, words))
}

fn longest_line(path: &Path) -> io::Result<String> {
    let text = fs::read_to_string(path)?;
    let longest = text.lines().max_by_key(|l| l.len()).unwrap_or("");
    Ok(longest.to_string())
}

fn main() -> io::Result<()> {
    let path: PathBuf = std::env::temp_dir().join("uredo_corpus_07.txt");
    write_sample(&path)?;
    let (lines, words) = count_lines_and_words(&path)?;
    println!("{lines} lines, {words} words");
    println!("longest: {}", longest_line(&path)?);
    let size = fs::metadata(&path)?.len();
    println!("{size} bytes");
    fs::remove_file(&path)?;
    match fs::read_to_string(&path) {
        Ok(_) => println!("still there?"),
        Err(e) => println!("gone: {}", e.kind()),
    }
    Ok(())
}
