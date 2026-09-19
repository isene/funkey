//! Small things a game keeps between runs: a high score, a setting. One
//! file per value under ~/.funkey/<game>/.

use std::path::PathBuf;

fn dir(game: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".funkey").join(game)
}

pub fn load(game: &str, key: &str) -> Option<String> {
    std::fs::read_to_string(dir(game).join(key)).ok().map(|s| s.trim().to_string())
}

pub fn save(game: &str, key: &str, value: &str) {
    let d = dir(game);
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join(key), value);
}

pub fn high_score(game: &str) -> u32 {
    load(game, "highscore").and_then(|s| s.parse().ok()).unwrap_or(0)
}

/// Remember a score if it beats the old one. True when it did.
pub fn record_score(game: &str, score: u32) -> bool {
    if score <= high_score(game) { return false; }
    save(game, "highscore", &score.to_string());
    true
}
