//! funkeys: the games on funkey as cards with screenshots. Arrows move,
//! Enter plays the one under the cursor in this terminal, `?` shows its
//! keys. A game that is not installed stays on the grid, dimmed.
//!
//! In a terminal with kitty graphics the games get real pixels: the
//! picker sets `FUNKEY_PIXELS=kitty` for them unless it is set already.
//!
//! The picker does nothing while it waits: one blocking read on stdin.

use crust::style;
use crust::{Crust, Cursor, Input, Pane, Popup};
use std::io::Write;
use std::path::PathBuf;

const VERSION: &str = "1.1";
const PAGE: &str = "https://isene.org/funkey/";

struct Game {
    name: &'static str,
    kind: &'static str,
    blurb: &'static str,
    /// The keys, for `?`.
    keys: &'static str,
    /// The screenshot the landing page shows.
    shot: &'static [u8],
}

const GAMES: &[Game] = &[
    Game {
        name: "castle",
        kind: "Walk demo",
        blurb: "A walk to a castle and in through its gate",
        keys: "Up and Down walk, Left and Right turn, A and D sidestep, W and S\nchange the pace, Space hands the walk back to the autopilot, Q quits.\n\nThere is nothing to win. Stone, sun and shadow, at real pixels.",
        shot: include_bytes!("../../docs/img/castle.png"),
    },
    Game {
        name: "soar",
        kind: "Flight demo",
        blurb: "A flight over fractal mountains",
        keys: "Left and Right turn, Up and Down climb and dive, W and S change\nspeed, Space hands the controls back to the autopilot, Q quits.\n\nThere is nothing to win. It is the terminal, with real pixels.",
        shot: include_bytes!("../../docs/img/soar.png"),
    },
    Game {
        name: "doom",
        kind: "Doom",
        blurb: "The game itself, from any Doom WAD",
        keys: "Arrows turn and walk, A and D sidestep, Space fires, E or Enter\nuses, 1-7 pick a weapon, Tab shows the map, Escape quits. The old\ncheats work.\n\nThe WAD is ~/.funkey/freedoom1.wad; Freedoom is free at\nfreedoom.github.io. Any Doom-format WAD works: doom <file.wad>.",
        shot: include_bytes!("../../docs/img/doom.png"),
    },
    Game {
        name: "jumpman",
        kind: "Platformer",
        blurb: "Jumpman Junior in spirit, five levels",
        keys: "Left and Right run, Space jumps, Up and Down climb, R restarts a\nlife, Q quits. Take every bomb to finish a level. Some bombs do\nthings.",
        shot: include_bytes!("../../docs/img/jumpman.png"),
    },
    Game {
        name: "invaders",
        kind: "Shooter",
        blurb: "Fifty-five of them, marching down",
        keys: "Left and Right move, Space fires, Q quits.\n\nShields crumble where they are hit, the march quickens as the rows\nthin, and each wave starts a little lower.",
        shot: include_bytes!("../../docs/img/invaders.png"),
    },
    Game {
        name: "drive",
        kind: "Driving, in 3D",
        blurb: "Packages against the clock, over the hills",
        keys: "Up and Down for the gas and the brake, Left and Right steer, Space\nstarts, Q quits. The arrow at the top points at the nearest package.",
        shot: include_bytes!("../../docs/img/drive.png"),
    },
    Game {
        name: "climb",
        kind: "Platformer",
        blurb: "The first game on funkey: ledges, ladders, coins",
        keys: "Arrows or WASD move, Space or Up jumps, Up and Down climb a ladder,\nR restarts, Q quits. Take every coin, mind the blobs.",
        shot: include_bytes!("../../docs/img/climb.png"),
    },
];

const RUST_RGB: (u8, u8, u8) = (247, 76, 0);
const HEAD_RGB: (u8, u8, u8) = (247, 140, 60);
const DIM_RGB: (u8, u8, u8) = (110, 110, 120);
const BAR_BG: (u8, u8, u8) = (38, 38, 38);
/// The Play tint from the fe2o3 launcher, so the cards look like the
/// card that opened them.
const TINT: (u8, u8, u8) = (14, 30, 19);

/// Card geometry: a screenshot box on top, three text rows under it.
/// The width comes from the terminal so the grid fills it.
const CARD_MIN_W: u16 = 40;
const SHOT_H: u16 = 10;
const CARD_H: u16 = SHOT_H + 5;
const GRID_Y: u16 = 3;

struct Ui {
    sel: usize,
    top: usize,
    installed: Vec<bool>,
    wad: bool,
    shots: PathBuf,
    images: Option<glow::Display>,
}

fn main() {
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "-h" | "--help" => {
                println!("funkeys {VERSION}: the games on funkey as cards");
                println!();
                println!("Usage: funkeys");
                println!();
                println!("In the grid: arrows move, Enter plays, ? shows a game's keys,");
                println!("             w opens {PAGE}, q quits");
                println!();
                for g in GAMES {
                    println!("  {:<10} {:<16} {}", g.name, g.kind, g.blurb);
                }
                return;
            }
            "-v" | "--version" => {
                println!("funkeys {VERSION}");
                return;
            }
            other => {
                eprintln!("funkeys: unknown option '{other}' (try -h)");
                std::process::exit(1);
            }
        }
    }

    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        for g in GAMES {
            let here = if bin_path(g.name).is_some() { "installed" } else { "missing" };
            println!("{:<10} {:<16} {:<48} {here}", g.name, g.kind, g.blurb);
        }
        return;
    }

    let mut ui = Ui {
        sel: 0,
        top: 0,
        installed: GAMES.iter().map(|g| bin_path(g.name).is_some()).collect(),
        wad: wad_path().is_file(),
        shots: unpack_shots(),
        images: None,
    };

    Crust::init();
    Crust::set_app_identity("funkeys");
    ui.images = {
        let d = glow::Display::new();
        if d.supported() { Some(d) } else { None }
    };
    let (mut cols, mut rows) = Crust::terminal_size();
    let mut status = Pane::new(1, rows, cols, 1, 250, 236);
    status.scroll = false;

    draw_all(&mut ui, &mut status, cols, rows);

    loop {
        let Some(key) = Input::getchr(None) else { continue };
        match key.as_str() {
            "q" | "ESC" => break,
            "RIGHT" | "l" => step(&mut ui, 1, &mut status, cols, rows),
            "LEFT" | "h" => step(&mut ui, -1, &mut status, cols, rows),
            "DOWN" | "j" => step(&mut ui, per_row(cols) as i32, &mut status, cols, rows),
            "UP" | "k" => step(&mut ui, -(per_row(cols) as i32), &mut status, cols, rows),
            "HOME" | "g" => {
                ui.sel = 0;
                draw_all(&mut ui, &mut status, cols, rows);
            }
            "END" | "G" => {
                ui.sel = GAMES.len() - 1;
                draw_all(&mut ui, &mut status, cols, rows);
            }
            "ENTER" | " " => {
                if !launch(&mut ui, &mut status) { continue; }
                let (c, r) = Crust::terminal_size();
                cols = c;
                rows = r;
                status.y = rows;
                status.w = cols;
                draw_all(&mut ui, &mut status, cols, rows);
            }
            "?" => {
                show_keys(&mut ui, cols, rows);
                draw_all(&mut ui, &mut status, cols, rows);
            }
            "w" => {
                let _ = std::process::Command::new("xdg-open")
                    .arg(PAGE)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                status.say(&style::dim(&format!(" {PAGE}")));
            }
            "RESIZE" => {
                let (c, r) = Crust::terminal_size();
                cols = c;
                rows = r;
                status.y = rows;
                status.w = cols;
                draw_all(&mut ui, &mut status, cols, rows);
            }
            _ => {}
        }
    }

    if let Some(d) = ui.images.as_mut() { d.clear_all(); }
    Crust::cleanup();
}

// ─────────────────────────── the grid ────────────────────────────────

fn grid(cols: u16) -> (usize, u16) {
    let n = ((cols / CARD_MIN_W).max(1)) as usize;
    (n, (cols / n as u16).max(24))
}

fn per_row(cols: u16) -> usize { grid(cols).0 }

fn per_page(cols: u16, rows: u16) -> usize {
    let card_rows = ((rows.saturating_sub(GRID_Y + 1)) / CARD_H).max(1) as usize;
    per_row(cols) * card_rows
}

fn step(ui: &mut Ui, delta: i32, status: &mut Pane, cols: u16, rows: u16) {
    let next = ui.sel as i32 + delta;
    if next < 0 || next as usize >= GAMES.len() { return; }
    let old_page = ui.sel / per_page(cols, rows);
    ui.sel = next as usize;
    if ui.sel / per_page(cols, rows) != old_page {
        draw_all(ui, status, cols, rows);
    } else {
        // Same page: the screenshots have not moved, so only the frames
        // repaint. Sending the pictures again would make every keypress
        // a graphics round trip.
        draw_cards(ui, cols, rows, false);
        status.say(&help_line(ui));
    }
}

fn draw_all(ui: &mut Ui, status: &mut Pane, cols: u16, rows: u16) {
    if let Some(d) = ui.images.as_mut() {
        d.clear(1, GRID_Y, cols, rows.saturating_sub(GRID_Y), cols, rows);
    }
    Crust::clear_screen();
    draw_header(ui, cols);
    draw_cards(ui, cols, rows, true);
    status.invalidate();
    status.say(&help_line(ui));
}

fn draw_header(ui: &Ui, cols: u16) {
    let n = GAMES.len();
    let here = ui.installed.iter().filter(|&&i| i).count();
    let info = format!(
        " {}  {}",
        style::rgb("funkeys", Some(RUST_RGB), None, "b"),
        style::dim("the games on funkey")
    );
    let right = if here == n {
        format!("✓ all {n} installed ")
    } else {
        format!("✓ {here} of {n} installed · fe2o3 fetches the rest ")
    };
    let pad = (cols as usize).saturating_sub(crust::display_width(&info) + crust::display_width(&right));
    let armed = style::rgb("", None, Some(BAR_BG), "");
    let armed = armed.trim_end_matches(style::RESET);
    let line = info.replace(style::RESET, &format!("{}{}", style::RESET, armed));
    print!(
        "{}{}",
        Cursor::at(1, 1),
        style::rgb(&format!("{line}{}{}", " ".repeat(pad), style::dim(&right)), None, Some(BAR_BG), "")
    );
    std::io::stdout().flush().ok();
}

/// The cards of the current page. `with_images` is false when only the
/// selection moved: the frames repaint, the screenshot boxes stay.
fn draw_cards(ui: &mut Ui, cols: u16, rows: u16, with_images: bool) {
    let page = per_page(cols, rows);
    let (cols_n, card_w) = grid(cols);
    ui.top = (ui.sel / page) * page;
    let mut s = String::new();
    for slot in 0..page {
        let Some(g) = GAMES.get(ui.top + slot) else { break };
        let x = 1 + (slot % cols_n) as u16 * card_w;
        let y = GRID_Y + (slot / cols_n) as u16 * CARD_H;
        s.push_str(&card(g, x, y, card_w, ui.top + slot == ui.sel, ui.installed[ui.top + slot], with_images));
    }
    print!("{s}");
    std::io::stdout().flush().ok();

    if !with_images { return; }
    let Some(d) = ui.images.as_mut() else { return };
    // A cell's shape in pixels, to centre a picture in its box. Where
    // the terminal does not say, a cell is about twice as tall as wide.
    let (cw, ch) = match glow::get_cell_size() {
        (0, _) | (_, 0) => (1u32, 2u32),
        (w, h) => (w as u32, h as u32),
    };
    for slot in 0..page {
        let Some(g) = GAMES.get(ui.top + slot) else { break };
        let x = 1 + (slot % cols_n) as u16 * card_w;
        let y = GRID_Y + (slot / cols_n) as u16 * CARD_H;
        let (bw, bh) = ((card_w - 4) as u32, SHOT_H as u32);
        let (pw, ph) = png_size(g.shot);
        let scale = ((bw * cw) as f32 / pw as f32).min((bh * ch) as f32 / ph as f32);
        let w = ((pw as f32 * scale / cw as f32).ceil() as u32).clamp(1, bw);
        let h = ((ph as f32 * scale / ch as f32).ceil() as u32).clamp(1, bh);
        let p = ui.shots.join(format!("{}.png", g.name));
        d.show(&p.display().to_string(), x + 2 + ((bw - w) / 2) as u16, y + 1 + ((bh - h) / 2) as u16, w as u16, h as u16);
    }
}

/// One card: frame, the screenshot box, name, kind, hook. With
/// `with_box` false the box interior is left alone: the picture is
/// still there from the last full draw.
fn card(g: &Game, x: u16, y: u16, card_w: u16, selected: bool, installed: bool, with_box: bool) -> String {
    let w = (card_w - 2) as usize;
    let (frame, name_rgb) = if selected {
        (RUST_RGB, HEAD_RGB)
    } else if installed {
        ((70, 70, 80), (220, 220, 225))
    } else {
        ((45, 45, 52), DIM_RGB)
    };
    let text_rgb = if installed { (170, 170, 180) } else { DIM_RGB };
    let blurb_rgb = if installed { (135, 135, 145) } else { (85, 85, 92) };
    let bar = |c: &str| style::rgb(c, Some(frame), Some(TINT), "");
    let (rule, mark, mark_rgb) = if installed { ("─", "✓", (120, 205, 130)) } else { ("╌", "↓", (255, 180, 90)) };
    let mut s = String::new();
    s.push_str(&Cursor::at(x, y));
    s.push_str(&format!(
        "{}{}{}{}",
        bar(&format!("┌{}", rule.repeat(w - 2))),
        style::rgb(mark, Some(mark_rgb), Some(TINT), "b"),
        bar(rule),
        bar("┐")
    ));
    for r in 0..SHOT_H {
        s.push_str(&Cursor::at(x, y + 1 + r));
        if with_box {
            s.push_str(&format!("{}{}{}", bar("│"), style::rgb(&" ".repeat(w), None, Some(TINT), ""), bar("│")));
        } else {
            s.push_str(&bar("│"));
            s.push_str(&Cursor::at(x + card_w - 1, y + 1 + r));
            s.push_str(&bar("│"));
        }
    }
    let lines = [
        style::rgb(&fit(g.name, w - 2), Some(name_rgb), Some(TINT), if selected { "b" } else { "" }),
        style::rgb(&fit(g.kind, w - 2), Some(text_rgb), Some(TINT), ""),
        style::rgb(&fit(g.blurb, w - 2), Some(blurb_rgb), Some(TINT), ""),
    ];
    for (i, l) in lines.iter().enumerate() {
        s.push_str(&Cursor::at(x, y + 1 + SHOT_H + i as u16));
        s.push_str(&format!("{}{}{}{}", bar("│"), style::rgb(" ", None, Some(TINT), ""), l, bar(" │")));
    }
    s.push_str(&Cursor::at(x, y + CARD_H - 1));
    s.push_str(&bar(&format!("└{}┘", rule.repeat(w))));
    s
}

fn fit(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n <= w {
        format!("{s}{}", " ".repeat(w - n))
    } else {
        s.chars().take(w.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn help_line(ui: &Ui) -> String {
    if !ui.installed[ui.sel] {
        return style::dim(&format!(
            "{} is not installed · i on the funkey card in fe2o3 fetches the games · q quit",
            GAMES[ui.sel].name
        ));
    }
    style::dim("←↓↑→ move · Enter plays it here · ? its keys · w the games' page · q quit")
}

// ─────────────────────────── running one ─────────────────────────────

/// Hand the terminal over, run the game, take it back. False when
/// nothing ran; the status line then says why.
fn launch(ui: &mut Ui, status: &mut Pane) -> bool {
    let g = &GAMES[ui.sel];
    if !ui.installed[ui.sel] {
        status.say(&style::rgb(
            &format!(" {} is not installed; i on the funkey card in fe2o3 fetches the games", g.name),
            Some((255, 170, 80)),
            None,
            "",
        ));
        return false;
    }
    if g.name == "doom" && !ui.wad {
        status.say(&style::rgb(
            &format!(" doom needs {} (Freedoom is free at freedoom.github.io)", wad_path().display()),
            Some((255, 170, 80)),
            None,
            "",
        ));
        return false;
    }
    let bin = bin_path(g.name).unwrap_or_else(|| PathBuf::from(g.name));
    if let Some(d) = ui.images.as_mut() { d.clear_all(); }
    Crust::cleanup();
    let mut cmd = std::process::Command::new(bin);
    if kitty_terminal() && std::env::var_os("FUNKEY_PIXELS").is_none() {
        cmd.env("FUNKEY_PIXELS", "kitty");
    }
    let _ = cmd.status();
    Crust::init();
    Crust::set_app_identity("funkeys");
    ui.wad = wad_path().is_file();
    true
}

/// Does this terminal draw kitty graphics? Then the games get real
/// pixels instead of half blocks.
fn kitty_terminal() -> bool {
    std::env::var("TERM").map(|t| t == "xterm-kitty").unwrap_or(false)
        || std::env::var_os("KITTY_WINDOW_ID").is_some()
        || std::env::var_os("_GLASS_ID").is_some()
        || std::env::var("TERM_PROGRAM").map(|t| t == "WezTerm").unwrap_or(false)
}

fn show_keys(ui: &mut Ui, cols: u16, rows: u16) {
    let g = &GAMES[ui.sel];
    let foot = if ui.installed[ui.sel] {
        String::new()
    } else {
        "\n\nNot installed. In fe2o3, i on the funkey card fetches the games;\nor build them: cargo build --release --examples in the funkey repo.".to_string()
    };
    let text = format!(
        "{}  {}\n{}\n\n{}{}",
        style::rgb(g.name, Some(RUST_RGB), None, "b"),
        style::dim(g.kind),
        style::dim(g.blurb),
        g.keys,
        foot
    );
    if let Some(d) = ui.images.as_mut() {
        d.clear(1, GRID_Y, cols, rows.saturating_sub(GRID_Y), cols, rows);
    }
    let w = 76.min(cols.saturating_sub(6));
    let h = (rows.saturating_sub(8)).clamp(6, 16);
    let mut pop = Popup::centered(w, h, 253, 234);
    pop.view(&text);
}

// ─────────────────────────── odds and ends ───────────────────────────

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
}

fn wad_path() -> PathBuf { home().join(".funkey/freedoom1.wad") }

/// The PNG's width and height, from its header.
fn png_size(png: &[u8]) -> (u32, u32) {
    if png.len() < 24 { return (4, 3); }
    let be = |i: usize| u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]);
    (be(16).max(1), be(20).max(1))
}

/// Write the embedded screenshots under ~/.funkey/shots/, once. The
/// image protocols want a path. The common case is a stat() per file.
fn unpack_shots() -> PathBuf {
    let dir = home().join(".funkey").join("shots");
    let _ = std::fs::create_dir_all(&dir);
    for g in GAMES {
        let p = dir.join(format!("{}.png", g.name));
        let same = std::fs::metadata(&p).map(|m| m.len() as usize == g.shot.len()).unwrap_or(false);
        if !same { let _ = std::fs::write(&p, g.shot); }
    }
    dir
}

/// Where the game's binary is: PATH first, then the two places fe2o3
/// installs into.
fn bin_path(bin: &str) -> Option<PathBuf> {
    let here = |dir: &std::path::Path| {
        let p = dir.join(bin);
        std::fs::metadata(&p).ok().filter(|m| m.is_file()).map(|_| p)
    };
    if let Ok(path) = std::env::var("PATH") {
        if let Some(p) = path.split(':').find_map(|d| here(std::path::Path::new(d))) {
            return Some(p);
        }
    }
    here(&home().join("bin")).or_else(|| here(&home().join(".local/bin")))
}
