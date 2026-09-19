//! A 3 by 5 pixel font for scores and short words. Upper case letters,
//! digits and a little punctuation; lower case is drawn as upper case.

/// Five rows of three bits, the high bit at the left.
pub fn glyph(c: char) -> Option<[u8; 5]> {
    let c = c.to_ascii_uppercase();
    let rows: [&str; 5] = match c {
        ' ' => ["...", "...", "...", "...", "..."],
        'A' => [".X.", "X.X", "XXX", "X.X", "X.X"],
        'B' => ["XX.", "X.X", "XX.", "X.X", "XX."],
        'C' => [".XX", "X..", "X..", "X..", ".XX"],
        'D' => ["XX.", "X.X", "X.X", "X.X", "XX."],
        'E' => ["XXX", "X..", "XX.", "X..", "XXX"],
        'F' => ["XXX", "X..", "XX.", "X..", "X.."],
        'G' => [".XX", "X..", "X.X", "X.X", ".XX"],
        'H' => ["X.X", "X.X", "XXX", "X.X", "X.X"],
        'I' => ["XXX", ".X.", ".X.", ".X.", "XXX"],
        'J' => ["..X", "..X", "..X", "X.X", ".X."],
        'K' => ["X.X", "X.X", "XX.", "X.X", "X.X"],
        'L' => ["X..", "X..", "X..", "X..", "XXX"],
        'M' => ["X.X", "XXX", "XXX", "X.X", "X.X"],
        'N' => ["X.X", "XXX", "XXX", "XXX", "X.X"],
        'O' => [".X.", "X.X", "X.X", "X.X", ".X."],
        'P' => ["XX.", "X.X", "XX.", "X..", "X.."],
        'Q' => [".X.", "X.X", "X.X", "XX.", ".XX"],
        'R' => ["XX.", "X.X", "XX.", "X.X", "X.X"],
        'S' => [".XX", "X..", ".X.", "..X", "XX."],
        'T' => ["XXX", ".X.", ".X.", ".X.", ".X."],
        'U' => ["X.X", "X.X", "X.X", "X.X", "XXX"],
        'V' => ["X.X", "X.X", "X.X", "X.X", ".X."],
        'W' => ["X.X", "X.X", "XXX", "XXX", "X.X"],
        'X' => ["X.X", "X.X", ".X.", "X.X", "X.X"],
        'Y' => ["X.X", "X.X", ".X.", ".X.", ".X."],
        'Z' => ["XXX", "..X", ".X.", "X..", "XXX"],
        '0' => [".X.", "X.X", "X.X", "X.X", ".X."],
        '1' => [".X.", "XX.", ".X.", ".X.", "XXX"],
        '2' => ["XX.", "..X", ".X.", "X..", "XXX"],
        '3' => ["XX.", "..X", ".X.", "..X", "XX."],
        '4' => ["X.X", "X.X", "XXX", "..X", "..X"],
        '5' => ["XXX", "X..", "XX.", "..X", "XX."],
        '6' => [".XX", "X..", "XX.", "X.X", ".X."],
        '7' => ["XXX", "..X", ".X.", ".X.", ".X."],
        '8' => [".X.", "X.X", ".X.", "X.X", ".X."],
        '9' => [".X.", "X.X", ".XX", "..X", "XX."],
        ':' => ["...", ".X.", "...", ".X.", "..."],
        '.' => ["...", "...", "...", "...", ".X."],
        ',' => ["...", "...", "...", ".X.", "X.."],
        '-' => ["...", "...", "XXX", "...", "..."],
        '+' => ["...", ".X.", "XXX", ".X.", "..."],
        '!' => [".X.", ".X.", ".X.", "...", ".X."],
        '?' => ["XX.", "..X", ".X.", "...", ".X."],
        '/' => ["..X", "..X", ".X.", "X..", "X.."],
        '(' => [".X.", "X..", "X..", "X..", ".X."],
        ')' => [".X.", "..X", "..X", "..X", ".X."],
        '\'' => [".X.", ".X.", "...", "...", "..."],
        '"' => ["X.X", "X.X", "...", "...", "..."],
        '%' => ["X.X", "..X", ".X.", "X..", "X.X"],
        '=' => ["...", "XXX", "...", "XXX", "..."],
        '<' => ["..X", ".X.", "X..", ".X.", "..X"],
        '>' => ["X..", ".X.", "..X", ".X.", "X.."],
        '_' => ["...", "...", "...", "...", "XXX"],
        '*' => ["X.X", ".X.", "XXX", ".X.", "X.X"],
        '#' => ["X.X", "XXX", "X.X", "XXX", "X.X"],
        _ => return None,
    };
    let mut out = [0u8; 5];
    for (i, r) in rows.iter().enumerate() {
        let b = r.as_bytes();
        out[i] = ((b[0] == b'X') as u8) << 2 | ((b[1] == b'X') as u8) << 1 | (b[2] == b'X') as u8;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_and_digits_have_glyphs_and_lower_case_maps_up() {
        assert_eq!(glyph('a'), glyph('A'));
        assert_eq!(glyph('T').unwrap()[0], 0b111);
        assert_eq!(glyph('1').unwrap()[4], 0b111);
        assert!(glyph('~').is_none());
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789:.-!?".chars() { assert!(glyph(c).is_some(), "{}", c); }
    }
}
