//! Small things a game keeps between runs: a high score, a setting. One
//! file per value under ~/.funkey/<game>/. In a web page there are no
//! files, so the values last while the page is open.

#[cfg(not(target_arch = "wasm32"))]
fn dir(game: &str) -> std::path::PathBuf {
    // The engine's own tests keep their values out of the home folder.
    #[cfg(test)]
    return crate::test_dir().join(game);
    #[cfg(not(test))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        std::path::PathBuf::from(home).join(".funkey").join(game)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load(game: &str, key: &str) -> Option<String> {
    std::fs::read_to_string(dir(game).join(key)).ok().map(|s| s.trim().to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save(game: &str, key: &str, value: &str) {
    let d = dir(game);
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join(key), value);
}

#[cfg(target_arch = "wasm32")]
thread_local! {
    static KEPT: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default();
}

#[cfg(target_arch = "wasm32")]
pub fn load(game: &str, key: &str) -> Option<String> {
    KEPT.with(|k| k.borrow().get(&format!("{game}/{key}")).cloned())
}

#[cfg(target_arch = "wasm32")]
pub fn save(game: &str, key: &str, value: &str) {
    KEPT.with(|k| k.borrow_mut().insert(format!("{game}/{key}"), value.to_string()));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_score_is_kept_only_when_it_beats_the_old_one() {
        let game = "store-score";
        let _ = std::fs::remove_dir_all(dir(game));
        assert_eq!(high_score(game), 0, "no file yet: no score");
        assert!(record_score(game, 50));
        assert!(!record_score(game, 50), "the same score again is no record");
        assert!(!record_score(game, 40));
        assert!(record_score(game, 60));
        assert_eq!(high_score(game), 60);
    }

    #[test]
    fn a_value_comes_back_as_it_was_saved() {
        let game = "store-value";
        let _ = std::fs::remove_dir_all(dir(game));
        assert_eq!(load(game, "name"), None);
        save(game, "name", "ABC\n");
        assert_eq!(load(game, "name").as_deref(), Some("ABC"), "the line end is cut off");
        save(game, "name", "XYZ");
        assert_eq!(load(game, "name").as_deref(), Some("XYZ"));
        assert_eq!(load("store-other", "name"), None, "each game has its own values");
        save(game, "highscore", "not a number");
        assert_eq!(high_score(game), 0, "a file that is no number counts as none");
    }
}
