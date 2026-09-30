//! eliminator: the Eliminator, the maze beneath the royal castle in Amaron,
//! as a turn-based dungeon crawl played by the rules of the Amar RPG
//! (d6gaming.org). Every year the King's heralds call for contenders; one
//! in ten walks out, and the King makes them noble. Five levels up through
//! the maze, and the Raven Demon at the gate.
//!
//! Every roll is the open-ended O6, and the game shows it: the dice, the
//! totals and the sum. Offence against Defence, damage less armour, the
//! stances, double attacks at -5, power hits, initiative, wounds, light,
//! fear, criticals and fumbles, and marks that raise your skills. The
//! sheet adds Characteristic, Attribute and Skill into the totals the
//! rolls use.
//!
//!     cargo run --release --example eliminator
//!
//! Arrows or hjkl move, yubn diagonally; walk into a foe to attack. 1-6 or
//! Tab pick a stance, f fires, p drinks a potion, m bandages, r rests, t
//! douses or lights the light, g takes what lies here, < or Enter climbs,
//! s waits, ? shows the rules, q or Esc quits. `ELIMINATOR_START=<level>` starts
//! higher up; `ELIMINATOR_SEED=<n>` fixes the maze; `ELIMINATOR_FOE=<n>`
//! puts that foe, awake, at your right hand.

use funkey::*;
use std::collections::VecDeque;

const W: i32 = 640;
const H: i32 = 360;
const TILE: i32 = 14;
/// Tiles across and down the view.
const VW: i32 = 30;
const VH: i32 = 19;
const PANEL: i32 = 424;
/// The band under the map: the dice of the last exchange, and the log.
const BAND: i32 = 268;
const MW: i32 = 64;
const MH: i32 = 44;
const LEVELS: usize = 5;
const GAME: &str = "eliminator";
/// The game's own version; the engine has its own.
const VERSION: &str = "1.3";
const TEXT: Rgb = 0xe8e0d0;
const DIM: Rgb = 0x8a8478;
const GOLD: Rgb = 0xffd040;
const RED: Rgb = 0xff5040;
const GREEN: Rgb = 0x80e070;
const BLUE: Rgb = 0x80b0ff;
/// "The rest of the fight", in rounds.
const FIGHT: i32 = 30;

const LEVEL_NAMES: [&str; LEVELS] = ["THE PIT", "THE KENNELS", "THE DROWNED HALLS", "THE FAERIE CAGE", "THE GATE"];

// ------------------------------------------------------------------ dice

/// An open-ended d6, with every die it took.
#[derive(Clone, Debug, Default)]
struct Roll { total: i32, dice: Vec<u8>, crit: bool, fumble: bool }

/// The O6: a 6 rolls on, +1 for each 4-6 and stopping on 1-3; a 1 rolls on,
/// -1 for each 1-3 and stopping on 4-6. Two 6s in a row are a critical,
/// two 1s a fumble.
fn o6(rng: &mut Rng) -> Roll {
    let first = rng.below(6) as u8 + 1;
    let mut r = Roll { total: first as i32, dice: vec![first], crit: false, fumble: false };
    if (2..=5).contains(&first) { return r; }
    let mut prev = first;
    loop {
        let x = rng.below(6) as u8 + 1;
        r.dice.push(x);
        if first == 6 {
            if prev == 6 && x == 6 { r.crit = true; }
            if x >= 4 { r.total += 1; } else { return r; }
        } else {
            if prev == 1 && x == 1 { r.fumble = true; }
            if x <= 3 { r.total -= 1; } else { return r; }
        }
        prev = x;
    }
}

fn d6(rng: &mut Rng) -> i32 { rng.below(6) as i32 + 1 }

/// Does the attack land? A fumble never does, a critical always does
/// unless met by a higher critical defence; a critical defence holds, a
/// fumbled one fails; otherwise the higher total wins.
fn lands(a: &Roll, at: i32, d: &Roll, dt: i32) -> bool {
    if a.fumble { return false; }
    if a.crit { return !(d.crit && dt >= at); }
    if d.crit { return false; }
    if d.fumble { return true; }
    at > dt
}

// ------------------------------------------------------------------ the character

const CHARS: [&str; 3] = ["BODY", "MIND", "SPIRIT"];
/// The attributes the game uses, with their characteristic.
const ATTRS: [(&str, usize); 9] = [
    ("Strength", 0), ("Endurance", 0), ("Athletics", 0), ("Melee Combat", 0), ("Missile Combat", 0),
    ("Sleight", 0), ("Awareness", 1), ("Nature Knowledge", 1), ("Willpower", 1),
];

/// The attribute a skill sits under.
fn attr_of(skill: &str) -> usize {
    match skill {
        "Wield Weapon" => 0,
        "Fortitude" => 1,
        "Dodge" | "Move Quietly" | "Tumble" => 2,
        "Crossbow" | "Bow" | "Sling" | "Throwing" => 4,
        "Disarm Traps" => 5,
        "Reaction Speed" | "Alertness" | "Detect Traps" => 6,
        "Medical Lore" => 7,
        "Courage" => 8,
        _ => 3,
    }
}

#[derive(Clone, Debug)]
struct Sheet {
    name: &'static str,
    blurb: &'static str,
    size: f32,
    chars: [i32; 3],
    char_marks: [i32; 3],
    attrs: [i32; 9],
    attr_marks: [i32; 9],
    /// Name, rank, marks.
    skills: Vec<(&'static str, i32, i32)>,
}

impl Sheet {
    fn rank(&self, s: &str) -> i32 { self.skills.iter().find(|k| k.0 == s).map(|k| k.1).unwrap_or(0) }

    /// Characteristic, attribute and skill.
    fn parts(&self, s: &str) -> (i32, i32, i32) {
        let a = attr_of(s);
        (self.chars[ATTRS[a].1], self.attrs[a], self.rank(s))
    }

    /// What a roll adds: the three tiers summed.
    fn total(&self, s: &str) -> i32 {
        let (c, a, k) = self.parts(s);
        c + a + k
    }

    /// Body Points: SIZE x 2 + Fortitude / 3.
    fn bp_max(&self) -> i32 { (self.size * 2.0).floor() as i32 + self.total("Fortitude") / 3 }

    /// Damage Bonus: (SIZE + Wield Weapon) / 3.
    fn db(&self) -> i32 { ((self.size + self.total("Wield Weapon") as f32) / 3.0).floor() as i32 }

    /// Marks for a skill used well, or taken away by a fumble. When they
    /// reach 5 x (rank + 1) a d6 of 2 or more raises the skill, and the
    /// attribute above it gets a mark, and so on up.
    fn mark(&mut self, s: &'static str, n: i32, rng: &mut Rng) -> Option<String> {
        let i = match self.skills.iter().position(|k| k.0 == s) {
            Some(i) => i,
            None => { self.skills.push((s, 0, 0)); self.skills.len() - 1 }
        };
        let k = &mut self.skills[i];
        k.2 = (k.2 + n).max(0);
        let need = 5 * (k.1 + 1);
        if k.2 < need { return None; }
        let roll = d6(rng);
        if roll < 2 { return Some(format!("{} could rise, but the d6 shows 1", s)); }
        k.1 += 1;
        k.2 -= need;
        let mut msg = format!("{} rises to {} (d6 {})", s, k.1, roll);
        let a = attr_of(s);
        self.attr_marks[a] += 1;
        if self.attr_marks[a] >= 5 * (self.attrs[a] + 1) {
            self.attr_marks[a] = 0;
            self.attrs[a] += 1;
            msg = format!("{}; {} rises to {}", msg, ATTRS[a].0, self.attrs[a]);
            let c = ATTRS[a].1;
            self.char_marks[c] += 1;
            if self.char_marks[c] >= 5 * (self.chars[c] + 1) {
                self.char_marks[c] = 0;
                self.chars[c] += 1;
                msg = format!("{}; {} rises to {}", msg, CHARS[c], self.chars[c]);
            }
        }
        Some(msg)
    }
}

/// A contender, made by the creation rules: 1 in two characteristics and
/// 0 in the third, attributes 3 2 2 1 1 1, skills 3 2 2 2 1 1 1 1 1, and a
/// human's two bonus points.
struct Contender {
    sheet: Sheet,
    base: [(&'static str, i32); 9],
    bonus: [&'static str; 2],
    weapon: usize,
    missile: Option<usize>,
    ammo: i32,
    armour: usize,
    art: usize,
}

fn contenders() -> Vec<Contender> {
    type Base = [(&'static str, i32); 9];
    let make = |name, blurb, size, attrs: [(&str, i32); 6], base: Base, bonus: [&'static str; 2], weapon, missile, ammo, armour, art| {
        let mut a = [0; 9];
        for (n, v) in attrs { a[ATTRS.iter().position(|x| x.0 == n).unwrap()] = v; }
        let mut skills: Vec<(&'static str, i32, i32)> = base.iter().map(|&(n, r)| (n, r, 0)).collect();
        for b in bonus { skills.iter_mut().find(|k| k.0 == b).unwrap().1 += 1; }
        let sheet = Sheet { name, blurb, size, chars: [1, 1, 0], char_marks: [0; 3], attrs: a, attr_marks: [0; 9], skills };
        Contender { sheet, base, bonus, weapon, missile, ammo, armour, art }
    };
    vec![
        make("SELLSWORD", "A soldier for hire. Steady with the longsword.", 3.5,
            [("Melee Combat", 3), ("Endurance", 2), ("Athletics", 2), ("Strength", 1), ("Awareness", 1), ("Willpower", 1)],
            [("Longsword", 3), ("Fortitude", 2), ("Dodge", 2), ("Wield Weapon", 2), ("Reaction Speed", 1),
             ("Courage", 1), ("Detect Traps", 1), ("Medical Lore", 1), ("Move Quietly", 1)],
            ["Longsword", "Fortitude"], 3, None, 0, 2, 0),
        make("SCOUT", "Quiet and quick. A crossbow, and a short sword.", 3.0,
            [("Missile Combat", 3), ("Athletics", 2), ("Melee Combat", 2), ("Awareness", 1), ("Strength", 1), ("Endurance", 1)],
            [("Crossbow", 3), ("Dodge", 2), ("Short sword", 2), ("Move Quietly", 2), ("Detect Traps", 1),
             ("Reaction Speed", 1), ("Fortitude", 1), ("Alertness", 1), ("Medical Lore", 1)],
            ["Crossbow", "Dodge"], 2, Some(1), 24, 1, 1),
        make("BRUTE", "Big and strong. A heavy mace in both hands.", 4.0,
            [("Strength", 3), ("Melee Combat", 2), ("Endurance", 2), ("Athletics", 1), ("Willpower", 1), ("Awareness", 1)],
            [("Heavy mace", 3), ("Wield Weapon", 2), ("Fortitude", 2), ("Courage", 2), ("Dodge", 1),
             ("Reaction Speed", 1), ("Medical Lore", 1), ("Detect Traps", 1), ("Alertness", 1)],
            ["Heavy mace", "Fortitude"], 13, None, 0, 3, 2),
    ]
}

// ------------------------------------------------------------------ arms and armour

#[derive(Clone, Copy, PartialEq, Debug)]
enum Cat { Unarmed, Knife, Sword, Axe, Blunt, Pole }

#[derive(Clone, Copy, Debug)]
struct Weapon {
    name: &'static str,
    /// The weapon it is a form of: a longsword with a shield is a longsword.
    kind: &'static str,
    cat: Cat,
    str_req: i32,
    ini: i32,
    off: i32,
    def: i32,
    dam: i32,
    iron: bool,
}

const fn wp(name: &'static str, kind: &'static str, cat: Cat, str_req: i32, ini: i32, off: i32, def: i32, dam: i32, iron: bool) -> Weapon {
    Weapon { name, kind, cat, str_req, ini, off, def, dam, iron }
}

/// Melee weapons, from the Amar tables (shield pairs from the amar app).
const WEAPONS: [Weapon; 19] = [
    wp("Unarmed", "Unarmed", Cat::Unarmed, 0, 1, -2, -4, -4, false),
    wp("Knife", "Knife", Cat::Knife, 1, 2, -2, -3, -2, true),
    wp("Short sword", "Short sword", Cat::Sword, 2, 3, -1, -1, -2, true),
    wp("Longsword", "Longsword", Cat::Sword, 4, 5, 0, 0, -1, true),
    wp("Longsword & buckler", "Longsword", Cat::Sword, 4, 5, 1, 1, -1, true),
    wp("Longsword & round shield", "Longsword", Cat::Sword, 5, 5, 1, 3, -1, true),
    wp("Bastard sword", "Bastard sword", Cat::Sword, 4, 6, 0, 1, 0, true),
    wp("Great sword", "Great sword", Cat::Sword, 6, 7, 1, 1, 1, true),
    wp("Hatchet", "Hatchet", Cat::Axe, 3, 3, -2, -3, -1, true),
    wp("Broad axe", "Broad axe", Cat::Axe, 5, 4, -1, -2, 0, true),
    wp("Battle axe", "Battle axe", Cat::Axe, 5, 5, -1, 0, 2, true),
    wp("Club", "Club", Cat::Blunt, 4, 4, -1, -2, -2, false),
    wp("Light mace", "Light mace", Cat::Blunt, 3, 3, -1, -2, -2, true),
    wp("Heavy mace", "Heavy mace", Cat::Blunt, 4, 4, 0, 0, 0, true),
    wp("Heavy mace & round shield", "Heavy mace", Cat::Blunt, 6, 4, 1, 3, -1, true),
    wp("Hercules club", "Hercules club", Cat::Blunt, 6, 7, 0, 1, 2, false),
    wp("Staff", "Staff", Cat::Pole, 3, 6, 0, 2, -2, false),
    wp("Spear", "Spear", Cat::Pole, 4, 7, 0, 2, -1, true),
    wp("Halberd", "Halberd", Cat::Pole, 7, 7, 0, 2, 1, true),
];

fn weapon_named(name: &str) -> Option<&'static Weapon> { WEAPONS.iter().find(|w| w.name == name) }

#[derive(Clone, Copy, Debug)]
struct Missile { name: &'static str, skill: &'static str, str_req: i32, off: i32, dam: i32, range: i32, ammo: &'static str }

/// Missile weapons; damage carries the 2026 -1 for bows, crossbows and
/// slings, and DB is never added.
const MISSILES: [Missile; 4] = [
    Missile { name: "Throwing knives", skill: "Throwing", str_req: 1, off: -1, dam: -3, range: 15, ammo: "knives" },
    Missile { name: "Light crossbow", skill: "Crossbow", str_req: 2, off: 2, dam: 0, range: 20, ammo: "bolts" },
    Missile { name: "Light bow", skill: "Bow", str_req: 2, off: 0, dam: -1, range: 30, ammo: "arrows" },
    Missile { name: "Sling", skill: "Sling", str_req: 2, off: -3, dam: -3, range: 40, ammo: "stones" },
];

const ARMOURS: [(&str, i32); 7] = [
    ("Clothes", 0), ("Light leather", 1), ("Heavy leather", 2), ("Studded leather", 2), ("Cuir-bouilli", 3),
    ("Chainmail", 4), ("Banded mail", 5),
];

/// The six ways to fight: name, Off, Def, what it does.
const STANCES: [(&str, i32, i32, &str); 6] = [
    ("NORMAL", 0, 0, "no change"),
    ("OFFENSIVE", 3, -5, "Off +3, Def -5"),
    ("DEFENSIVE", -5, 3, "Off -5, Def +3"),
    ("ONLY DEFEND", 0, 5, "no attack, Def +5"),
    ("POWER HIT", -5, 0, "Off -5, DB added twice"),
    ("DOUBLE ATTACK", -5, 0, "two attacks at Off -5"),
];

// ------------------------------------------------------------------ foes

#[derive(Clone, Copy, Debug)]
struct Kind {
    name: &'static str,
    art: usize,
    bp: i32,
    ini: i32,
    off: i32,
    def: i32,
    dam: i32,
    ap: i32,
    /// Alertness total, against your Move Quietly.
    aware: i32,
    dodge: i32,
    courage: i32,
    tumble: i32,
    strength: i32,
    /// Actions a round.
    speed: f32,
    /// The DR of the Courage roll on first sight; 0 for none.
    fear: i32,
    attacks: u8,
    sp: i32,
    mindless: bool,
    armed: bool,
    big: bool,
}

const RAT: usize = 0;
const SPIDER: usize = 1;
const ARAXI: usize = 2;
const WOLF: usize = 3;
const ZOMBIE: usize = 4;
const RIVAL: usize = 5;
const PHOOKA: usize = 6;
const TROLL: usize = 7;
const DEMON: usize = 8;

const KINDS: [Kind; 9] = [
    Kind { name: "Giant rat", art: 3, bp: 3, ini: 3, off: 4, def: 3, dam: -2, ap: 0, aware: 5, dodge: 5, courage: 2, tumble: 5, strength: 1, speed: 1.0, fear: 0, attacks: 1, sp: 0, mindless: false, armed: false, big: false },
    Kind { name: "Cave spider", art: 4, bp: 4, ini: 5, off: 6, def: 5, dam: -1, ap: 1, aware: 6, dodge: 6, courage: 4, tumble: 7, strength: 2, speed: 1.0, fear: 0, attacks: 1, sp: 0, mindless: true, armed: false, big: false },
    Kind { name: "Araxi", art: 5, bp: 7, ini: 4, off: 6, def: 5, dam: 0, ap: 1, aware: 5, dodge: 4, courage: 5, tumble: 4, strength: 4, speed: 1.0, fear: 0, attacks: 1, sp: 4, mindless: false, armed: true, big: false },
    Kind { name: "Wolf", art: 6, bp: 6, ini: 6, off: 7, def: 6, dam: 0, ap: 0, aware: 7, dodge: 6, courage: 5, tumble: 6, strength: 3, speed: 1.5, fear: 0, attacks: 1, sp: 0, mindless: false, armed: false, big: false },
    Kind { name: "Zombie", art: 7, bp: 9, ini: 2, off: 5, def: 3, dam: 1, ap: 0, aware: 3, dodge: 1, courage: 99, tumble: 2, strength: 5, speed: 0.5, fear: 0, attacks: 1, sp: 2, mindless: true, armed: false, big: false },
    Kind { name: "Rival contender", art: 8, bp: 8, ini: 5, off: 8, def: 9, dam: 1, ap: 2, aware: 6, dodge: 5, courage: 6, tumble: 5, strength: 5, speed: 1.0, fear: 0, attacks: 1, sp: 8, mindless: false, armed: true, big: false },
    Kind { name: "Phooka", art: 9, bp: 7, ini: 7, off: 8, def: 8, dam: 1, ap: 0, aware: 7, dodge: 7, courage: 7, tumble: 7, strength: 4, speed: 1.5, fear: 0, attacks: 1, sp: 6, mindless: false, armed: false, big: false },
    Kind { name: "Troll", art: 10, bp: 13, ini: 5, off: 10, def: 6, dam: 4, ap: 2, aware: 4, dodge: 2, courage: 9, tumble: 3, strength: 10, speed: 1.0, fear: 9, attacks: 1, sp: 0, mindless: false, armed: true, big: true },
    Kind { name: "Raven Demon", art: 11, bp: 16, ini: 9, off: 14, def: 10, dam: 3, ap: 1, aware: 9, dodge: 9, courage: 99, tumble: 9, strength: 9, speed: 1.5, fear: 12, attacks: 2, sp: 0, mindless: false, armed: false, big: true },
];

/// Which foes walk each level.
const LEVEL_FOES: [&[usize]; LEVELS] = [
    &[RAT, RAT, SPIDER, ARAXI, ARAXI],
    &[RAT, ARAXI, ARAXI, WOLF, WOLF, RIVAL],
    &[SPIDER, ZOMBIE, ZOMBIE, ARAXI, RIVAL],
    &[ZOMBIE, WOLF, PHOOKA, PHOOKA, RIVAL],
    &[ZOMBIE, PHOOKA, RIVAL, RIVAL, TROLL],
];

#[derive(Clone, Debug)]
struct Foe {
    k: usize,
    x: i32,
    y: i32,
    bp: i32,
    max: i32,
    awake: bool,
    energy: f32,
    /// Penalty to every roll, and rounds left.
    status: Vec<(i32, i32)>,
    bleed: i32,
    down: bool,
    disarmed: bool,
    flee: i32,
    /// Out cold: a critical made it faint.
    out: i32,
    /// Cut by iron: a phooka stops mending.
    ironed: bool,
    seen: bool,
    snuck: bool,
    acted: bool,
    lose_attack: bool,
    bonus: i32,
    flash: f32,
}

impl Foe {
    fn new(k: usize, x: i32, y: i32) -> Foe {
        let bp = KINDS[k].bp;
        Foe { k, x, y, bp, max: bp, awake: false, energy: 0.0, status: Vec::new(), bleed: 0, down: false, disarmed: false,
              flee: 0, out: 0, ironed: false, seen: false, snuck: false, acted: false, lose_attack: false, bonus: 0, flash: 0.0 }
    }

    fn kind(&self) -> &'static Kind { &KINDS[self.k] }

    /// Wounds: at half the BP -2 to every roll, at a quarter -4.
    fn wound(&self) -> i32 { if self.bp * 4 <= self.max { -4 } else if self.bp * 2 <= self.max { -2 } else { 0 } }

    fn modifier(&self) -> i32 { self.status.iter().map(|s| s.0).sum::<i32>() + self.wound() }
}

// ------------------------------------------------------------------ the maze

#[derive(Clone, Copy, PartialEq, Debug)]
enum T { Wall, Floor, Up, Gate }

#[derive(Clone, Copy, Debug)]
enum Item { Silver(i32), Gold(i32), Potion, Torches(i32), Oil, Lantern, Weapon(usize), Missile(usize, i32), Ammo(i32), Armour(usize) }

impl Item {
    fn name(&self) -> String {
        match *self {
            Item::Silver(n) => format!("{} silver", n),
            Item::Gold(n) => format!("{} gold", n),
            Item::Potion => "a healing potion".into(),
            Item::Torches(n) => format!("{} torches", n),
            Item::Oil => "a flask of oil".into(),
            Item::Lantern => "a lantern".into(),
            Item::Weapon(w) => {
                let w = &WEAPONS[w];
                format!("{} (I{} O{:+} D{:+} d{:+}, STR {})", w.name, w.ini, w.off, w.def, w.dam, w.str_req)
            }
            Item::Missile(m, n) => format!("{} and {} {}", MISSILES[m].name, n, MISSILES[m].ammo),
            Item::Ammo(n) => format!("{} bolts or arrows", n),
            Item::Armour(a) => format!("{} (AP {})", ARMOURS[a].0, ARMOURS[a].1),
        }
    }
}

#[derive(Clone, Debug)]
struct Loot { x: i32, y: i32, item: Item }

#[derive(Clone, Debug)]
struct Trap { x: i32, y: i32, found: bool }

// ------------------------------------------------------------------ the game

#[derive(Clone, Copy, PartialEq, Debug)]
enum Mode { Title, Intro, Play, Help, Dead(f32), Won(f32) }

/// One line of the dice tray: who rolled, the dice, and the sum.
struct Line { who: String, dice: Vec<u8>, sum: String, color: Rgb }

/// The walk-through behind `i` on the title: the three tiers, the O6, and
/// one blow against an Araxi.
const INTRO_PAGES: usize = 3;

#[derive(Default)]
struct Intro {
    page: usize,
    /// The skill lit on the first page, in the order the tree draws them.
    skill: usize,
    /// The last O6 and the seconds since, so its dice land one by one.
    roll: Option<Roll>,
    age: f32,
    /// Every O6 rolled on the second page, by total from -3 to 12.
    tally: [u32; 16],
    sum: i64,
    crits: u32,
    fumbles: u32,
    /// The last blow on the third page, and the Araxi's BP.
    blow: Option<Blow>,
    araxi: i32,
}

struct Blow { a: Roll, at: i32, d: Roll, dt: i32, hit: bool, dam: Option<(Roll, i32)> }

/// A sheet's skills in the order the tree draws them: by attribute, so no
/// two lines cross.
fn tree_skills(sheet: &Sheet) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = sheet.skills.iter().map(|k| k.0).collect();
    v.sort_by_key(|s| attr_of(s));
    v
}

struct Player {
    sheet: Sheet,
    art: usize,
    x: i32,
    y: i32,
    bp: i32,
    weapon: usize,
    missile: Option<usize>,
    ammo: i32,
    armour: usize,
    potions: i32,
    torches: i32,
    burn: i32,
    lit: bool,
    lantern: bool,
    oil: i32,
    sp: i32,
    gp: i32,
    stance: usize,
    status: Vec<(i32, i32)>,
    /// A strain that holds until a Medical Lore roll mends it.
    strain: i32,
    /// 1 bleeds a BP a minute, 2 a BP a round.
    bleed: i32,
    frozen: i32,
    panic: i32,
    down: bool,
    dropped: bool,
    lose_attack: bool,
    /// Bandaged since the last wound.
    bandaged: bool,
}

struct Sounds {
    dice: Sample,
    hit: Sample,
    miss: Sample,
    crit: Sample,
    fumble: Sample,
    coin: Sample,
    drink: Sample,
    climb: Sample,
    rise: Sample,
    die: Sample,
    fear: Sample,
    kill: Sample,
    won: Sample,
    music: Sample,
    title: Sample,
}

struct Eliminator {
    mode: Mode,
    pick: usize,
    level: usize,
    seed: u64,
    map: Vec<T>,
    seen: Vec<bool>,
    vis: Vec<bool>,
    dist: Vec<i32>,
    tex: Vec<Vec<Rgb>>,
    foes: Vec<Foe>,
    loot: Vec<Loot>,
    traps: Vec<Trap>,
    p: Player,
    turn: u32,
    log: VecDeque<(String, Rgb)>,
    tray_head: String,
    tray: Vec<Line>,
    floats: Vec<(f32, f32, String, Rgb, f32)>,
    banner: Option<(String, f32)>,
    rng: Rng,
    particles: Particles,
    time: f32,
    hold: f32,
    high: u32,
    audio: Audio,
    s: Sounds,
    art: Vec<Sprite>,
    help_page: usize,
    intro: Intro,
}

fn idx(x: i32, y: i32) -> usize { (y * MW + x) as usize }
fn inside(x: i32, y: i32) -> bool { x >= 0 && y >= 0 && x < MW && y < MH }

impl Eliminator {
    fn new() -> Eliminator {
        let seed = std::env::var("ELIMINATOR_SEED").ok().and_then(|v| v.parse().ok())
            .unwrap_or_else(|| Rng::from_time().next_u32() as u64);
        let c = contenders().remove(0);
        let p = Player {
            sheet: c.sheet, art: 0, x: 0, y: 0, bp: 1, weapon: 3, missile: None, ammo: 0, armour: 2, potions: 0, torches: 0,
            burn: 0, lit: true, lantern: false, oil: 0, sp: 0, gp: 0, stance: 0, status: Vec::new(), strain: 0, bleed: 0,
            frozen: 0, panic: 0, down: false, dropped: false, lose_attack: false, bandaged: false,
        };
        let mut g = Eliminator {
            mode: Mode::Title, pick: 0, level: 0, seed, map: vec![T::Wall; (MW * MH) as usize], seen: vec![false; (MW * MH) as usize],
            vis: vec![false; (MW * MH) as usize], dist: vec![i32::MAX; (MW * MH) as usize], tex: Vec::new(),
            foes: Vec::new(), loot: Vec::new(), traps: Vec::new(), p, turn: 0, log: VecDeque::new(), tray_head: String::new(),
            tray: Vec::new(), floats: Vec::new(), banner: None, rng: Rng::from_time(), particles: Particles::new(), time: 0.0,
            hold: 0.0, high: funkey::store::high_score(GAME), audio: Audio::off(), s: sounds(), art: art(), help_page: 0, intro: Intro::default(),
        };
        g.particles.gravity = 60.0;
        g.build(0);
        g
    }

    fn say(&mut self, s: &str, c: Rgb) {
        self.log.push_back((s.to_string(), c));
        while self.log.len() > 40 { self.log.pop_front(); }
    }

    fn float(&mut self, x: i32, y: i32, s: &str, c: Rgb) {
        self.floats.push((x as f32, y as f32, s.to_string(), c, 0.0));
    }

    fn start(&mut self, pick: usize) {
        let c = contenders().remove(pick);
        self.p = Player {
            sheet: c.sheet, art: c.art, x: 0, y: 0, bp: 0, weapon: c.weapon, missile: c.missile, ammo: c.ammo, armour: c.armour,
            potions: 1, torches: 2, burn: 600, lit: true, lantern: false, oil: 0, sp: 10, gp: 0, stance: 0,
            status: Vec::new(), strain: 0, bleed: 0, frozen: 0, panic: 0, down: false, dropped: false, lose_attack: false, bandaged: false,
        };
        self.p.bp = self.p.sheet.bp_max();
        self.log.clear();
        self.tray.clear();
        self.tray_head.clear();
        self.turn = 0;
        let start = std::env::var("ELIMINATOR_START").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(1).clamp(1, LEVELS) - 1;
        self.build(start);
        // ELIMINATOR_FOE=<n>: that foe awake at your right hand, for
        // screenshots and tests.
        if let Some(k) = std::env::var("ELIMINATOR_FOE").ok().and_then(|v| v.parse::<usize>().ok()).filter(|&k| k < KINDS.len()) {
            let mut f = Foe::new(k, self.p.x + 1, self.p.y);
            f.awake = true;
            f.seen = true;
            self.foes.push(f);
        }
        self.mode = Mode::Play;
        self.say("You are lowered into the dark. Climb out.", GOLD);
        self.say("Walk into a foe to attack. ? explains the rules.", DIM);
        self.audio.play_loop(1, &self.s.music, 0.5);
    }

    // -------------------------------------------------------------- the intro

    /// Open the walk-through for the picked contender, its weapon skill lit.
    fn open_intro(&mut self) {
        let c = contenders().remove(self.pick);
        let w = WEAPONS[c.weapon].name;
        let skill = tree_skills(&c.sheet).iter().position(|s| *s == w).unwrap_or(0);
        self.intro = Intro { skill, araxi: KINDS[ARAXI].bp, ..Intro::default() };
        self.mode = Mode::Intro;
    }

    /// Roll `n` O6 on the second page and count them.
    fn intro_roll(&mut self, n: usize) {
        for _ in 0..n {
            let r = o6(&mut self.rng);
            let it = &mut self.intro;
            it.tally[(r.total.clamp(-3, 12) + 3) as usize] += 1;
            it.sum += r.total as i64;
            if r.crit { it.crits += 1; }
            if r.fumble { it.fumbles += 1; }
            it.roll = Some(r);
        }
        // A hundred at once land at once.
        self.intro.age = if n > 1 { 9.0 } else { 0.0 };
        self.audio.play(&self.s.dice, 0.5);
    }

    /// One blow on the third page, by the same sums as the maze, without
    /// stance, light or wounds. A fallen Araxi gets up for another go.
    fn intro_blow(&mut self) {
        if self.intro.araxi <= 0 {
            self.intro.araxi = KINDS[ARAXI].bp;
            self.intro.blow = None;
            return;
        }
        let c = contenders().remove(self.pick);
        let w = &WEAPONS[c.weapon];
        let k = &KINDS[ARAXI];
        let (a, d) = (o6(&mut self.rng), o6(&mut self.rng));
        let (at, dt) = (a.total + w.off + c.sheet.total(w.name), d.total + k.def);
        let hit = lands(&a, at, &d, dt);
        let dam = if hit {
            let r = o6(&mut self.rng);
            let n = (r.total + w.dam + c.sheet.db() - k.ap).max(0);
            self.intro.araxi -= n;
            Some((r, n))
        } else {
            None
        };
        self.intro.blow = Some(Blow { a, at, d, dt, hit, dam });
        self.intro.age = 0.0;
        self.audio.play(&self.s.dice, 0.5);
    }

    // -------------------------------------------------------------- building a level

    fn build(&mut self, n: usize) {
        self.level = n;
        let mut rng = Rng::new(self.seed.wrapping_add(n as u64 * 7919));
        self.map.fill(T::Wall);
        self.seen.fill(false);
        self.vis.fill(false);
        self.foes.clear();
        self.loot.clear();
        self.traps.clear();
        let mut rooms: Vec<(i32, i32, i32, i32)> = Vec::new();
        for _ in 0..400 {
            if rooms.len() >= 11 { break; }
            let (w, h) = (5 + rng.below(7) as i32, 4 + rng.below(5) as i32);
            let (x, y) = (1 + rng.below((MW - w - 2) as u32) as i32, 1 + rng.below((MH - h - 2) as u32) as i32);
            if rooms.iter().any(|&(rx, ry, rw, rh)| x < rx + rw + 2 && rx < x + w + 2 && y < ry + rh + 2 && ry < y + h + 2) { continue; }
            rooms.push((x, y, w, h));
        }
        for &(x, y, w, h) in &rooms {
            for yy in y..y + h { for xx in x..x + w { self.map[idx(xx, yy)] = T::Floor; } }
        }
        rooms.sort_by_key(|r| r.0 * 3 + r.1);
        let centre = |r: &(i32, i32, i32, i32)| (r.0 + r.2 / 2, r.1 + r.3 / 2);
        let dig = |map: &mut Vec<T>, a: (i32, i32), b: (i32, i32), across_first: bool| {
            let (mut x, mut y) = a;
            let go = |map: &mut Vec<T>, x: i32, y: i32| { if map[idx(x, y)] == T::Wall { map[idx(x, y)] = T::Floor; } };
            if across_first {
                while x != b.0 { go(map, x, y); x += (b.0 - x).signum(); }
                while y != b.1 { go(map, x, y); y += (b.1 - y).signum(); }
            } else {
                while y != b.1 { go(map, x, y); y += (b.1 - y).signum(); }
                while x != b.0 { go(map, x, y); x += (b.0 - x).signum(); }
            }
            go(map, x, y);
        };
        for i in 1..rooms.len() {
            let flip = rng.chance(0.5);
            dig(&mut self.map, centre(&rooms[i - 1]), centre(&rooms[i]), flip);
        }
        for _ in 0..3 {
            let (a, b) = (rng.below(rooms.len() as u32) as usize, rng.below(rooms.len() as u32) as usize);
            if a != b { let flip = rng.chance(0.5); dig(&mut self.map, centre(&rooms[a]), centre(&rooms[b]), flip); }
        }
        // The start in the first room, the way up in the room farthest from it.
        let start = centre(&rooms[0]);
        self.p.x = start.0;
        self.p.y = start.1;
        self.flood();
        let far = (1..rooms.len()).max_by_key(|&i| { let c = centre(&rooms[i]); self.dist[idx(c.0, c.1)] }).unwrap_or(0);
        let up = centre(&rooms[far]);
        self.map[idx(up.0, up.1)] = if n == LEVELS - 1 { T::Gate } else { T::Up };
        // Foes and what lies about.
        let free = |g: &Eliminator, x: i32, y: i32| g.map[idx(x, y)] == T::Floor && (x, y) != start && !g.foes.iter().any(|f| f.x == x && f.y == y)
            && !g.loot.iter().any(|l| l.x == x && l.y == y);
        let spot = |rng: &mut Rng, r: &(i32, i32, i32, i32)| (r.0 + rng.below(r.2 as u32) as i32, r.1 + rng.below(r.3 as u32) as i32);
        let menu = LEVEL_FOES[n];
        let want = 8 + 2 * n;
        let mut tries = 0;
        while self.foes.len() < want && tries < 500 {
            tries += 1;
            let r = rooms[1 + rng.below(rooms.len() as u32 - 1) as usize];
            let (x, y) = spot(&mut rng, &r);
            if !free(self, x, y) || (x - start.0).abs() + (y - start.1).abs() < 8 { continue; }
            let k = menu[rng.below(menu.len() as u32) as usize];
            self.foes.push(Foe::new(k, x, y));
        }
        if n == LEVELS - 1 {
            // The Raven Demon beside the gate.
            let mut f = Foe::new(DEMON, up.0 - 1, up.1);
            if self.map[idx(f.x, f.y)] != T::Floor { f.x = up.0; f.y = up.1 - 1; }
            self.foes.retain(|o| (o.x - up.0).abs() > 2 || (o.y - up.1).abs() > 2);
            self.foes.push(f);
        }
        let place = |g: &mut Eliminator, rng: &mut Rng, item: Item| {
            for _ in 0..100 {
                let r = rooms[rng.below(rooms.len() as u32) as usize];
                let (x, y) = spot(rng, &r);
                if free(g, x, y) { g.loot.push(Loot { x, y, item }); return; }
            }
        };
        for _ in 0..6 + n {
            let s = d6(&mut rng) + d6(&mut rng) + 3 * n as i32;
            place(self, &mut rng, Item::Silver(s));
        }
        for _ in 0..(n / 2 + rng.below(2) as usize) { let g = 1 + rng.below(2 + n as u32) as i32; place(self, &mut rng, Item::Gold(g)); }
        place(self, &mut rng, Item::Potion);
        if rng.chance(0.6) { place(self, &mut rng, Item::Potion); }
        place(self, &mut rng, Item::Torches(2));
        if n == 1 { place(self, &mut rng, Item::Lantern); }
        if n >= 1 { place(self, &mut rng, Item::Oil); }
        let shots = 8 + rng.below(8) as i32;
        place(self, &mut rng, Item::Ammo(shots));
        let arms: [&[Item]; LEVELS] = [
            &[Item::Weapon(4), Item::Armour(2), Item::Missile(0, 6)],
            &[Item::Weapon(17), Item::Weapon(9), Item::Armour(3), Item::Missile(3, 20)],
            &[Item::Armour(5), Item::Weapon(6), Item::Weapon(5), Item::Missile(2, 20)],
            &[Item::Weapon(10), Item::Weapon(14), Item::Armour(4), Item::Weapon(7)],
            &[Item::Armour(6), Item::Weapon(15), Item::Weapon(18)],
        ];
        for _ in 0..2 {
            let a = arms[n][rng.below(arms[n].len() as u32) as usize];
            place(self, &mut rng, a);
        }
        // Traps in the corridors and rooms, never on the way in or out.
        let mut t = 0;
        while self.traps.len() < 4 + n && t < 400 {
            t += 1;
            let (x, y) = (1 + rng.below(MW as u32 - 2) as i32, 1 + rng.below(MH as u32 - 2) as i32);
            if self.map[idx(x, y)] != T::Floor || (x - start.0).abs() + (y - start.1).abs() < 5 { continue; }
            if (x - up.0).abs() + (y - up.1).abs() < 3 || self.traps.iter().any(|t| t.x == x && t.y == y) { continue; }
            self.traps.push(Trap { x, y, found: false });
        }
        self.tex = textures(n);
        self.flood();
        self.fov();
    }

    /// Steps from the player to every floor tile, for the foes to follow.
    fn flood(&mut self) {
        self.dist.fill(i32::MAX);
        let mut q = VecDeque::new();
        self.dist[idx(self.p.x, self.p.y)] = 0;
        q.push_back((self.p.x, self.p.y));
        while let Some((x, y)) = q.pop_front() {
            let d = self.dist[idx(x, y)];
            for (dx, dy) in DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if !inside(nx, ny) || self.map[idx(nx, ny)] == T::Wall || self.dist[idx(nx, ny)] != i32::MAX { continue; }
                self.dist[idx(nx, ny)] = d + 1;
                q.push_back((nx, ny));
            }
        }
    }

    /// How far the light reaches: a lantern 7, a torch 5, the dark 1.
    fn radius(&self) -> i32 {
        if !self.p.lit { return 1; }
        if self.p.lantern && self.p.oil > 0 { 7 } else if self.p.burn > 0 { 5 } else { 1 }
    }

    /// What the player sees: every tile in the light with nothing between.
    fn fov(&mut self) {
        self.vis.fill(false);
        let r = self.radius();
        let (px, py) = (self.p.x, self.p.y);
        for y in (py - r)..=(py + r) {
            for x in (px - r)..=(px + r) {
                if !inside(x, y) || (x - px) * (x - px) + (y - py) * (y - py) > r * r + r { continue; }
                if self.clear_line(px, py, x, y) {
                    self.vis[idx(x, y)] = true;
                    self.seen[idx(x, y)] = true;
                }
            }
        }
    }

    fn clear_line(&self, x0: i32, y0: i32, x1: i32, y1: i32) -> bool {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        loop {
            if (x, y) == (x1, y1) { return true; }
            if (x, y) != (x0, y0) && self.map[idx(x, y)] == T::Wall { return false; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }

    fn foe_at(&self, x: i32, y: i32) -> Option<usize> { self.foes.iter().position(|f| f.x == x && f.y == y && f.bp > 0) }

    // -------------------------------------------------------------- the player's numbers

    fn wound(&self) -> i32 {
        let m = self.p.sheet.bp_max();
        if self.p.bp * 4 <= m { -4 } else if self.p.bp * 2 <= m { -2 } else { 0 }
    }

    fn status(&self) -> i32 { self.p.status.iter().map(|s| s.0).sum::<i32>() + self.p.strain }

    /// Torchlight costs -1 Off and -2 Def, the dark -5 and -10.
    fn light(&self) -> (i32, i32) { if self.radius() > 1 { (-1, -2) } else { (-5, -10) } }

    /// A weapon heavier than your Wield Weapon total: -1, -3, -5, or not at all.
    fn strength(&self, req: i32) -> Option<i32> {
        match req - self.p.sheet.total("Wield Weapon") {
            i32::MIN..=0 => Some(0),
            1 => Some(-1),
            2 => Some(-3),
            3 => Some(-5),
            _ => None,
        }
    }

    /// The skill behind the weapon in hand: its own, or a trained one of
    /// the same kind (-1), the same group (-3) or another (-5).
    fn weapon_skill(&self) -> (i32, i32, &'static str) {
        let w = &WEAPONS[if self.p.dropped { 0 } else { self.p.weapon }];
        let mut best = (self.p.sheet.total(w.name), 0, w.name);
        for &(name, rank, _) in &self.p.sheet.skills {
            if attr_of(name) != 3 || rank == 0 || name == w.name { continue; }
            let Some(o) = weapon_named(name) else { continue };
            let pen = if o.kind == w.kind { -1 } else if o.cat == w.cat { -3 } else { -5 };
            let t = self.p.sheet.total(name) + pen;
            if t > best.0 { best = (t, pen, name); }
        }
        best
    }

    fn held(&self) -> &'static Weapon { &WEAPONS[if self.p.dropped { 0 } else { self.p.weapon }] }

    /// Initiative, Offence, Defence and damage with everything that bears
    /// on them now, and the modifiers alone.
    fn numbers(&self) -> (i32, i32, i32, i32, i32) {
        let w = self.held();
        let (skill, _, _) = self.weapon_skill();
        let st = STANCES[self.p.stance];
        let mods = self.status() + self.wound() + self.strength(w.str_req).unwrap_or(-5);
        let (lo, ld) = self.light();
        let dodge = self.p.sheet.total("Dodge") / 5;
        let ini = w.ini + self.p.sheet.total("Reaction Speed") + mods;
        let off = w.off + skill + st.1 + mods + lo;
        let mut def = w.def + skill + dodge + st.2 + mods + ld;
        if self.p.down { def -= 5; }
        if self.p.dropped { def -= 5; }
        let dam = w.dam + self.p.sheet.db() + if self.p.stance == 4 { self.p.sheet.db() } else { 0 };
        (ini, off, def, dam, mods)
    }

    fn mark(&mut self, skill: &'static str, n: i32) {
        if let Some(msg) = self.p.sheet.mark(skill, n, &mut self.rng) {
            let up = !msg.contains("shows 1");
            self.say(&msg, if up { GOLD } else { DIM });
            if up {
                self.banner = Some((format!("{} {}", skill.to_uppercase(), self.p.sheet.rank(skill)), 2.2));
                self.audio.play(&self.s.rise, 1.0);
                let m = self.p.sheet.bp_max();
                self.p.bp = self.p.bp.min(m);
            }
        }
    }

    fn line(&mut self, who: &str, r: &Roll, sum: String, color: Rgb) {
        self.tray.push(Line { who: who.to_string(), dice: r.dice.clone(), sum, color });
    }

    // -------------------------------------------------------------- a turn

    fn end_turn(&mut self) {
        self.turn += 1;
        // The light burns down.
        if self.p.lit {
            if self.p.lantern && self.p.oil > 0 {
                self.p.oil -= 1;
                if self.p.oil == 0 { self.say("The lantern runs dry.", RED); }
            } else if self.p.burn > 0 {
                self.p.burn -= 1;
                if self.p.burn == 0 {
                    if self.p.torches > 0 {
                        self.p.torches -= 1;
                        self.p.burn = 600;
                        self.say("The torch gutters out. You light another.", DIM);
                    } else {
                        self.say("Your last torch dies. Darkness: -5 Off, -10 Def.", RED);
                    }
                }
            }
        }
        // Bleeding: a BP a round, or a BP a minute.
        if self.p.bleed == 2 || (self.p.bleed == 1 && self.turn % 10 == 0) {
            self.p.bp -= 1;
            self.p.bandaged = false;
            self.say("You bleed: -1 BP.", RED);
        }
        tick(&mut self.p.status);
        self.p.frozen = (self.p.frozen - 1).max(0);
        self.p.panic = (self.p.panic - 1).max(0);
        if self.p.bp <= 0 { self.die("You bleed out"); return; }
        // The foes.
        self.flood();
        for i in 0..self.foes.len() {
            self.foes[i].acted = false;
            let f = &mut self.foes[i];
            if f.bp <= 0 { continue; }
            tick(&mut f.status);
            f.flee = (f.flee - 1).max(0);
            if f.out > 0 { f.out -= 1; }
            if f.bleed == 2 || (f.bleed == 1 && self.turn % 10 == 0) { f.bp -= 1; }
            // A phooka mends a BP a round, unless iron has cut it.
            if f.k == PHOOKA && !f.ironed && f.bp < f.max && f.bp > 0 { f.bp += 1; }
        }
        for i in 0..self.foes.len() {
            if self.foes[i].bp <= 0 { continue; }
            self.foes[i].energy += self.foes[i].kind().speed;
            while self.foes[i].energy >= 1.0 {
                self.foes[i].energy -= 1.0;
                self.foe_act(i);
                if matches!(self.mode, Mode::Dead(_)) { return; }
            }
        }
        let dead: Vec<usize> = (0..self.foes.len()).filter(|&i| self.foes[i].bp <= 0).collect();
        for &i in dead.iter().rev() { self.kill(i); }
        self.fov();
        self.notice();
    }

    /// Foes that might hear you, fears to face, traps to spot.
    fn notice(&mut self) {
        let (px, py) = (self.p.x, self.p.y);
        let sneak = self.p.sheet.total("Move Quietly") + if self.p.lit { 0 } else { 3 };
        for i in 0..self.foes.len() {
            let (fx, fy, bp, awake, snuck, seen, k) = {
                let f = &self.foes[i];
                (f.x, f.y, f.bp, f.awake, f.snuck, f.seen, *f.kind())
            };
            if bp <= 0 { continue; }
            let d = (fx - px).abs().max((fy - py).abs());
            if !awake && d <= 7 && self.clear_line(px, py, fx, fy) {
                let (a, s) = (o6(&mut self.rng), o6(&mut self.rng));
                let (at, st) = (a.total + k.aware, s.total + sneak);
                if at > st {
                    self.foes[i].awake = true;
                    self.say(&format!("The {} hears you ({} against your {}).", k.name, at, st), RED);
                } else if d <= 4 && !snuck {
                    self.foes[i].snuck = true;
                    self.mark("Move Quietly", 1);
                }
            }
            if self.vis[idx(fx, fy)] && !seen {
                self.foes[i].seen = true;
                if k.fear > 0 { self.face_fear(i, k.fear); }
            }
        }
        for t in 0..self.traps.len() {
            let tr = &self.traps[t];
            if tr.found || (tr.x - px).abs().max((tr.y - py).abs()) > 1 { continue; }
            let r = o6(&mut self.rng);
            let total = r.total + self.p.sheet.total("Detect Traps");
            if total >= 8 + self.level as i32 {
                self.traps[t].found = true;
                self.say(&format!("You spot a trap (Detect Traps {} against DR {}).", total, 8 + self.level), GREEN);
                self.mark("Detect Traps", 1);
            }
        }
    }

    /// The Courage roll on first sight of something dreadful.
    fn face_fear(&mut self, i: usize, dr: i32) {
        let r = o6(&mut self.rng);
        let c = self.p.sheet.total("Courage");
        let total = r.total + c;
        let name = self.foes[i].kind().name;
        self.tray_head = format!("FEAR OF THE {}", name.to_uppercase());
        self.tray.clear();
        self.line("Courage", &r, format!("{} + {} = {}  vs DR {}", r.total, c, total, dr), BLUE);
        self.audio.play(&self.s.fear, 1.0);
        let miss = dr - total;
        if miss <= 0 && !r.fumble {
            self.say(&format!("The {} is dreadful, but you hold ({} against {}).", name, total, dr), GREEN);
            self.mark("Courage", 1);
        } else if miss <= 3 && !r.fumble {
            self.p.status.push((-1, miss));
            self.say(&format!("Fear grips you: -1 for {} rounds.", miss), RED);
        } else if miss == 4 && !r.fumble {
            self.p.frozen = 1;
            self.say("You freeze with fear for a round.", RED);
        } else {
            self.p.panic = d6(&mut self.rng);
            self.say(&format!("You panic and must flee for {} rounds!", self.p.panic), RED);
        }
    }

    fn die(&mut self, how: &str) {
        if matches!(self.mode, Mode::Dead(_)) { return; }
        self.say(&format!("{}. The Eliminator claims another.", how), RED);
        let score = self.score();
        if funkey::store::record_score(GAME, score) { self.high = score; }
        self.mode = Mode::Dead(0.0);
        self.audio.stop(1);
        self.audio.play(&self.s.die, 1.0);
        let (x, y) = self.screen_of(self.p.x, self.p.y);
        self.particles.burst(x as f32, y as f32, 40, 90.0, 1.0, 0xa02020);
    }

    fn score(&self) -> u32 {
        let won = if matches!(self.mode, Mode::Won(_)) { 2000 } else { 0 };
        (self.p.sp + 50 * self.p.gp + 100 * self.level as i32 + won).max(0) as u32
    }

    // -------------------------------------------------------------- the player's actions

    fn step(&mut self, dx: i32, dy: i32) {
        if self.p.frozen > 0 { self.say("You are frozen with fear.", RED); self.end_turn(); return; }
        if self.p.down {
            self.p.down = false;
            self.say("You get back on your feet.", DIM);
            self.end_turn();
            return;
        }
        let (nx, ny) = (self.p.x + dx, self.p.y + dy);
        if !inside(nx, ny) { return; }
        if let Some(i) = self.foe_at(nx, ny) {
            if self.p.panic > 0 { self.say("You are panicking: you cannot attack, only run.", RED); return; }
            self.attack(i);
            self.end_turn();
            return;
        }
        if self.map[idx(nx, ny)] == T::Wall { return; }
        if let Some(t) = self.traps.iter().position(|t| t.x == nx && t.y == ny) {
            self.trap(t);
            if matches!(self.mode, Mode::Dead(_)) { return; }
        }
        self.p.x = nx;
        self.p.y = ny;
        self.pick_up();
        match self.map[idx(nx, ny)] {
            T::Up => self.say("Stairs up. Press < or Enter to climb.", GOLD),
            T::Gate => self.say("The gate out. Press < or Enter to leave.", GOLD),
            _ => {}
        }
        self.end_turn();
    }

    fn trap(&mut self, t: usize) {
        let dr = if self.traps[t].found { 8 } else { 12 };
        let r = o6(&mut self.rng);
        let dodge = self.p.sheet.total("Dodge");
        let total = r.total + dodge;
        self.tray_head = if self.traps[t].found { "LEAPING THE TRAP".into() } else { "A HIDDEN TRAP".into() };
        self.tray.clear();
        self.line("Dodge", &r, format!("{} + {} = {}  vs DR {}", r.total, dodge, total, dr), BLUE);
        self.traps[t].found = true;
        if total >= dr && !r.fumble {
            self.say(&format!("Spikes spring up; you leap clear (Dodge {} against {}).", total, dr), GREEN);
            self.mark("Dodge", 1);
        } else {
            let d = o6(&mut self.rng);
            let dmg = (d.total + self.level as i32 - 1).max(1);
            self.line("Spikes", &d, format!("{} + level {} = {}", d.total, self.level as i32 - 1, dmg), RED);
            self.hurt_player(dmg);
            self.say(&format!("Spikes! {} damage.", dmg), RED);
            if self.p.bp <= 0 { self.die("Spikes run you through"); }
        }
    }

    fn hurt_player(&mut self, dmg: i32) {
        if dmg <= 0 { return; }
        self.p.bp -= dmg;
        self.p.bandaged = false;
        let (x, y) = self.screen_of(self.p.x, self.p.y);
        self.float(x, y - 8, &format!("-{}", dmg), RED);
        self.particles.burst(x as f32, y as f32, 8 + dmg as usize * 2, 50.0, 0.5, 0xc02020);
        self.audio.play(&self.s.hit, 1.0);
    }

    fn pick_up(&mut self) {
        let (x, y) = (self.p.x, self.p.y);
        let mut i = 0;
        while i < self.loot.len() {
            if self.loot[i].x != x || self.loot[i].y != y { i += 1; continue; }
            let item = self.loot[i].item;
            let taken = match item {
                Item::Silver(n) => { self.p.sp += n; true }
                Item::Gold(n) => { self.p.gp += n; true }
                Item::Potion => { self.p.potions += 1; true }
                Item::Torches(n) => { self.p.torches += n; true }
                Item::Oil => { self.p.oil += 1200; true }
                Item::Lantern => { self.p.lantern = true; true }
                Item::Ammo(n) => {
                    if self.p.missile.is_some_and(|m| m != 0) { self.p.ammo += n; true } else { false }
                }
                _ => false,
            };
            if taken {
                self.say(&format!("You take {}.", item.name()), GOLD);
                self.audio.play(&self.s.coin, 1.0);
                self.loot.remove(i);
            } else {
                let how = match item {
                    Item::Weapon(_) | Item::Missile(..) => "g swaps it for yours",
                    Item::Armour(_) => "g puts it on",
                    _ => "g takes it",
                };
                self.say(&format!("Here: {}. {}.", item.name(), how), TEXT);
                i += 1;
            }
        }
    }

    /// g: take the weapon, bow or armour lying here, leaving your own.
    fn take(&mut self) {
        let (x, y) = (self.p.x, self.p.y);
        let Some(i) = self.loot.iter().position(|l| l.x == x && l.y == y && matches!(l.item, Item::Weapon(_) | Item::Missile(..) | Item::Armour(_) | Item::Ammo(_))) else {
            if self.p.dropped { self.p.dropped = false; self.say("You pick up your weapon.", TEXT); self.end_turn(); }
            return;
        };
        match self.loot[i].item {
            Item::Weapon(w) => {
                let Some(_) = self.strength(WEAPONS[w].str_req) else {
                    self.say(&format!("The {} is too heavy for your Wield Weapon {}.", WEAPONS[w].name, self.p.sheet.total("Wield Weapon")), RED);
                    return;
                };
                let old = WEAPONS[self.p.weapon].name;
                self.loot[i].item = Item::Weapon(self.p.weapon);
                self.p.weapon = w;
                self.p.dropped = false;
                let (skill, pen, name) = self.weapon_skill();
                let why = if pen == 0 { String::new() } else { format!(", trained as {} {:+}", name, pen) };
                self.say(&format!("You take the {}: skill {}{}. Your {} lies here.", WEAPONS[w].name, skill, why, old), GOLD);
            }
            Item::Missile(m, n) => {
                let left = self.p.missile.map(|o| format!(" Your {} lies here.", MISSILES[o].name)).unwrap_or_default();
                self.loot[i].item = match self.p.missile { Some(o) => Item::Missile(o, self.p.ammo), None => Item::Silver(0) };
                if matches!(self.loot[i].item, Item::Silver(0)) { self.loot.remove(i); }
                self.p.missile = Some(m);
                self.p.ammo = n;
                self.say(&format!("You take the {} ({} {}).{}", MISSILES[m].name, n, MISSILES[m].ammo, left), GOLD);
            }
            Item::Armour(a) => {
                let (new, old) = (ARMOURS[a], ARMOURS[self.p.armour]);
                if new.1 <= old.1 {
                    self.say(&format!("The {} (AP {}) is no better than your {} (AP {}).", new.0, new.1, old.0, old.1), DIM);
                    return;
                }
                self.loot.remove(i);
                self.p.armour = a;
                self.say(&format!("You put on the {}, AP {}, and leave your {} behind.", new.0, new.1, old.0), GOLD);
            }
            Item::Ammo(n) => {
                if self.p.missile.is_some() { self.p.ammo += n; self.loot.remove(i); self.say(&format!("You take {} shots.", n), GOLD); }
                else { self.say("You have nothing to shoot them with.", DIM); }
                return;
            }
            _ => return,
        }
        self.end_turn();
    }

    /// One round of melee against foe `i`: initiative, then your attack or
    /// attacks, the foe striking first if it won the initiative.
    fn attack(&mut self, i: usize) {
        self.tray.clear();
        if self.p.stance == 3 {
            self.tray_head = "ONLY DEFENDING".into();
            self.say("You only defend (stance 4): no attack, Def +5.", DIM);
            return;
        }
        if self.p.lose_attack {
            self.p.lose_attack = false;
            self.say("Your fumble costs you this attack.", RED);
            return;
        }
        if self.p.dropped {
            self.p.dropped = false;
            self.say("You grab your weapon from the floor.", TEXT);
            return;
        }
        let (ini, ..) = self.numbers();
        let (a, b) = (o6(&mut self.rng), o6(&mut self.rng));
        let (pi, fi) = (a.total + ini, b.total + self.foes[i].kind().ini);
        let name = self.foes[i].kind().name;
        self.tray_head = format!("{} AGAINST THE {}", STANCES[self.p.stance].0, name.to_uppercase());
        let foe_first = fi > pi && self.can_strike(i);
        self.line("Initiative", &a, format!("{} + {} = {}  vs {} {}", a.total, ini, pi, name, fi), DIM);
        if foe_first {
            self.foe_attack(i);
            self.foes[i].acted = true;
            if matches!(self.mode, Mode::Dead(_)) || self.foes[i].bp <= 0 { return; }
        } else {
            self.mark("Reaction Speed", 1);
        }
        let n = if self.p.stance == 5 { 2 } else { 1 };
        for k in 0..n {
            if self.foes[i].bp <= 0 { break; }
            self.strike(i, k == 1);
        }
    }

    fn can_strike(&self, i: usize) -> bool {
        let f = &self.foes[i];
        f.bp > 0 && f.awake && f.out == 0 && !f.down && f.flee == 0 && !f.acted
            && (f.x - self.p.x).abs().max((f.y - self.p.y).abs()) == 1
    }

    /// One swing: your O6 + Off against the foe's O6 + Def, then O6 + DAM +
    /// DB less its AP.
    fn strike(&mut self, i: usize, second: bool) {
        let (_, off, _, dam, _) = self.numbers();
        let w = *self.held();
        let skill = self.weapon_skill().2;
        let f = &self.foes[i];
        let k = *f.kind();
        let mut def = k.def + f.modifier();
        let mut why = String::new();
        if !f.awake { def -= 10; why = " asleep -10".into(); }
        else if f.out > 0 { def -= 10; why = " fainted -10".into(); }
        else if f.flee > 0 { def -= 7; why = " back -7".into(); }
        if f.down { def -= 5; why.push_str(" down -5"); }
        let (a, d) = (o6(&mut self.rng), o6(&mut self.rng));
        let (at, dt) = (a.total + off, d.total + def);
        let label = if second { "2nd attack" } else { "You attack" };
        self.line(label, &a, format!("{} + Off {} = {}", a.total, off, at), TEXT);
        self.line(k.name, &d, format!("{} + Def {}{} = {}", d.total, def, why, dt), DIM);
        self.foes[i].awake = true;
        self.audio.play(&self.s.dice, 0.6);
        let (sx, sy) = self.screen_of(self.foes[i].x, self.foes[i].y);
        if a.fumble {
            self.float(sx, sy - 10, "FUMBLE", RED);
            self.mark(skill, -1);
            self.fumble(true, i);
            return;
        }
        if !lands(&a, at, &d, dt) {
            self.float(sx, sy - 10, "miss", DIM);
            self.say(&format!("You miss the {} ({} against {}).", k.name, at, dt), DIM);
            self.audio.play(&self.s.miss, 0.8);
            return;
        }
        self.mark(skill, 1);
        let r = o6(&mut self.rng);
        let mut dmg = dam + r.total - k.ap;
        let mut text = format!("{} + d {} - AP {}", r.total, dam, k.ap);
        if a.crit {
            self.mark(skill, 1);
            let c = self.critical(true, i);
            dmg += c.0;
            if c.0 != 0 { text.push_str(&format!(" + {}", c.0)); }
            if c.1 { dmg *= 2; text.push_str(" x2"); }
            if c.2 && self.foes[i].bp > 0 {
                self.say("Opportunity: a free attack!", GOLD);
                self.strike_free(i);
            }
        }
        let dmg = dmg.max(0);
        self.line("Damage", &r, format!("{} = {}", text, dmg), GOLD);
        if w.iron { self.foes[i].ironed = true; }
        self.foes[i].bp -= dmg;
        self.foes[i].flash = 0.15;
        self.float(sx, sy - 10, &format!("{}", dmg), GOLD);
        self.particles.burst(sx as f32, sy as f32, 4 + dmg as usize * 2, 60.0, 0.4, 0xb02020);
        self.audio.play(&self.s.hit, 1.0);
        let left = self.foes[i].bp.max(0);
        self.say(&format!("You hit the {} for {} ({} against {}); {} BP left.", k.name, dmg, at, dt, left), TEXT);
    }

    /// The free attack a critical can open.
    fn strike_free(&mut self, i: usize) {
        let keep = self.tray.len();
        self.strike(i, true);
        self.tray.truncate(keep + 3);
    }

    /// A foe's turn.
    fn foe_act(&mut self, i: usize) {
        let f = &self.foes[i];
        if f.bp <= 0 || !f.awake || f.out > 0 { return; }
        if f.down {
            self.foes[i].down = false;
            return;
        }
        let d = (f.x - self.p.x).abs().max((f.y - self.p.y).abs());
        if d == 1 && f.flee == 0 {
            if !f.acted { self.foe_attack(i); self.foes[i].acted = true; }
            return;
        }
        // Step along the flood: towards you, or away when it flees.
        let (x, y) = (f.x, f.y);
        let here = self.dist[idx(x, y)];
        let away = f.flee > 0;
        let mut best: Option<(i32, i32, i32)> = None;
        for (dx, dy) in DIRS {
            let (nx, ny) = (x + dx, y + dy);
            if !inside(nx, ny) || self.map[idx(nx, ny)] == T::Wall || (nx, ny) == (self.p.x, self.p.y) || self.foe_at(nx, ny).is_some() { continue; }
            let v = self.dist[idx(nx, ny)];
            if v == i32::MAX { continue; }
            let score = if away { -v } else { v };
            if (away && v > here) || (!away && v < here) {
                if best.is_none_or(|b| score < b.2) { best = Some((nx, ny, score)); }
            }
        }
        if let Some((nx, ny, _)) = best {
            self.foes[i].x = nx;
            self.foes[i].y = ny;
        }
    }

    /// A foe strikes you: its O6 + Off against your O6 + Def.
    fn foe_attack(&mut self, i: usize) {
        let k = *self.foes[i].kind();
        if self.foes[i].lose_attack {
            self.foes[i].lose_attack = false;
            self.say(&format!("The {} fumbled and loses its attack.", k.name), DIM);
            return;
        }
        if self.tray.is_empty() || !self.tray_head.contains("AGAINST") {
            self.tray.clear();
            self.tray_head = format!("THE {} ATTACKS", k.name.to_uppercase());
        }
        for n in 0..k.attacks {
            if matches!(self.mode, Mode::Dead(_)) { return; }
            let (_, _, def, _, _) = self.numbers();
            let def = def + if self.p.stance == 3 { 0 } else { 0 };
            let f = &self.foes[i];
            let mut off = k.off + f.modifier() + f.bonus;
            if k.attacks > 1 { off -= 5; }
            if f.disarmed { off -= 3; }
            self.foes[i].bonus = 0;
            let (a, d) = (o6(&mut self.rng), o6(&mut self.rng));
            let (at, dt) = (a.total + off, d.total + def);
            let who = if k.attacks > 1 { format!("{} {}", k.name, n + 1) } else { k.name.to_string() };
            self.line(&who, &a, format!("{} + Off {} = {}", a.total, off, at), RED);
            self.line("You defend", &d, format!("{} + Def {} = {}", d.total, def, dt), TEXT);
            self.audio.play(&self.s.dice, 0.5);
            let (px, py) = self.screen_of(self.p.x, self.p.y);
            if a.fumble {
                self.float(px, py - 12, "its fumble", GREEN);
                self.fumble(false, i);
                continue;
            }
            if !lands(&a, at, &d, dt) {
                self.float(px, py - 12, "parried", BLUE);
                self.say(&format!("The {} misses you ({} against {}).", k.name, at, dt), DIM);
                let (skill, ..) = (self.weapon_skill().2, 0);
                self.mark(skill, 1);
                continue;
            }
            let r = o6(&mut self.rng);
            let ap = ARMOURS[self.p.armour].1;
            let mut dam = k.dam + if self.foes[i].disarmed { -3 } else { 0 };
            let mut text = format!("{} + d {} - AP {}", r.total, dam, ap);
            let mut dmg = dam + r.total - ap;
            if a.crit {
                let c = self.critical(false, i);
                dam += c.0;
                dmg += c.0;
                if c.0 != 0 { text.push_str(&format!(" + {}", c.0)); }
                if c.1 { dmg *= 2; text.push_str(" x2"); }
                let _ = dam;
            }
            let dmg = dmg.max(0);
            self.line("Damage", &r, format!("{} = {}", text, dmg), RED);
            self.hurt_player(dmg);
            self.say(&format!("The {} hits you for {} ({} against {}).", k.name, dmg, at, dt), RED);
            if self.p.bp <= 0 { self.die(&format!("The {} strikes you down", k.name)); return; }
        }
    }

    // -------------------------------------------------------------- criticals and fumbles

    /// A critical hit: roll a category and an entry and apply it. Returns
    /// damage to add, whether damage doubles, and whether a free attack
    /// follows.
    fn critical(&mut self, by_player: bool, i: usize) -> (i32, bool, bool) {
        let mut out = (0, false, false);
        let cat = d6(&mut self.rng);
        let rolls = if cat == 6 { if by_player { let s = self.weapon_skill().2; self.mark(s, 1); } 2 } else { 1 };
        self.audio.play(&self.s.crit, 1.0);
        for r in 0..rolls {
            let c = if rolls == 2 { loop { let c = d6(&mut self.rng); if c != 6 { break c; } } } else { cat };
            let e = d6(&mut self.rng);
            let name = self.foes[i].kind().name;
            let target = if by_player { format!("The {}", name) } else { "You".into() };
            let text = match (c, e) {
                (1, 1..=3) => "Looks really cool.".to_string(),
                (1, _) => {
                    let adj = [9, 6, 3][(e - 4) as usize];
                    if by_player { self.foe_fear(i, adj) } else { self.player_fear(adj) }
                }
                (2, _) => {
                    let (pen, rounds, what) = match e {
                        1 => (-1, 1, "off balance"),
                        2 => (-3, 1, "confused"),
                        3 => (-3, 3, "stunned"),
                        4 => { let d = d6(&mut self.rng); (-d, d6(&mut self.rng), "staggered") }
                        5 => { let o = o6(&mut self.rng).total.max(1); (-o, o, "reeling") }
                        _ => (-(o6(&mut self.rng).total.max(1) + 3), FIGHT, "shocked"),
                    };
                    if by_player { self.foes[i].status.push((pen, rounds)); } else { self.p.status.push((pen, rounds)); }
                    format!("{} {}: {} for {} rounds.", target, what, pen, rounds.min(FIGHT))
                }
                (3, _) => {
                    match e {
                        1 => { out.0 += 1; "Good hit: +1 damage.".into() }
                        2 => { out.0 += 3; "Tough hit: +3 damage.".into() }
                        3 => { let d = d6(&mut self.rng) + 1; out.0 += d; format!("Great hit: +{} damage.", d) }
                        4 => { let o = o6(&mut self.rng).total + 2; out.0 += o; format!("Greater hit: +{} damage.", o) }
                        5 => { out.1 = true; "Power hit: double damage.".into() }
                        _ => { out.2 = by_player; "Opportunity found.".into() }
                    }
                }
                (4, 1 | 2) => {
                    let dr = if e == 1 { 8 } else { 12 };
                    let tum = if by_player { self.foes[i].kind().tumble } else { self.p.sheet.total("Tumble") };
                    let roll = o6(&mut self.rng).total + tum;
                    if roll < dr {
                        if by_player { self.foes[i].down = true; } else { self.p.down = true; }
                        format!("{} knocked down (Tumble {} against {}).", target, roll, dr)
                    } else { format!("{} stays up (Tumble {} against {}).", target, roll, dr) }
                }
                (4, 3) => {
                    if by_player && self.foes[i].kind().armed { self.foes[i].disarmed = true; format!("{} is disarmed.", target) }
                    else if !by_player { self.p.dropped = true; "Your weapon is knocked from your hand.".into() }
                    else { "It has nothing to disarm.".into() }
                }
                (4, 4 | 5) => { out.0 += 1; "A cut to the weapon, and to the hand: +1 damage.".into() }
                (4, _) => {
                    if by_player { let n = d6(&mut self.rng); self.p.sp += n; format!("It loses {} silver; you catch it.", n) }
                    else if self.p.potions > 0 { self.p.potions -= 1; "A potion smashes on the floor.".into() }
                    else { "Your pack tears, but nothing falls.".into() }
                }
                (5, 1 | 2) => {
                    let b = e as i32;
                    if by_player { self.foes[i].bleed = self.foes[i].bleed.max(b); } else { self.p.bleed = self.p.bleed.max(b); }
                    format!("{} bleeding: -1 BP a {}.", target, if b == 1 { "minute" } else { "round" })
                }
                (5, 3..=5) => {
                    if by_player { self.foes[i].status.push((-3, FIGHT)); } else { self.p.strain = -3; }
                    format!("{} strained: -3 until Medical Lore mends it.", target)
                }
                _ => {
                    if by_player { self.foes[i].out = 2 * d6(&mut self.rng); } else { self.p.frozen = d6(&mut self.rng); }
                    format!("{} faints!", target)
                }
            };
            let cats = ["Impression", "Side effect", "Increased effect", "Added effect", "Special"];
            let head = if rolls == 2 && r == 0 { "Critical, rolled twice" } else { "Critical" };
            self.say(&format!("{}: {} {}. {}", head, cats[(c - 1) as usize], e, text), GOLD);
        }
        let (x, y) = if by_player { self.screen_of(self.foes[i].x, self.foes[i].y) } else { self.screen_of(self.p.x, self.p.y) };
        self.float(x, y - 20, "CRITICAL", GOLD);
        self.particles.burst(x as f32, y as f32, 24, 100.0, 0.6, GOLD);
        out
    }

    fn foe_fear(&mut self, i: usize, adj: i32) -> String {
        let k = *self.foes[i].kind();
        if k.mindless || k.courage >= 99 { return format!("The {} knows no fear.", k.name); }
        let r = o6(&mut self.rng).total + k.courage + adj;
        if r < 12 {
            let n = d6(&mut self.rng);
            self.foes[i].flee = n;
            format!("The {} breaks and flees for {} rounds (Courage {} against 12).", k.name, n, r)
        } else { format!("The {} holds its nerve (Courage {} against 12).", k.name, r) }
    }

    fn player_fear(&mut self, adj: i32) -> String {
        let r = o6(&mut self.rng).total + self.p.sheet.total("Courage") + adj;
        if r < 12 { self.p.panic = d6(&mut self.rng); format!("You panic for {} rounds (Courage {} against 12).", self.p.panic, r) }
        else { format!("You hold your nerve (Courage {} against 12).", r) }
    }

    /// A fumble by you (`player`) or by foe `i`.
    fn fumble(&mut self, player: bool, i: usize) {
        self.audio.play(&self.s.fumble, 1.0);
        let cat = d6(&mut self.rng);
        let rolls = if cat == 1 { if player { let s = self.weapon_skill().2; self.mark(s, -1); } 2 } else { 1 };
        for r in 0..rolls {
            let c = if rolls == 2 { loop { let c = d6(&mut self.rng); if c != 1 { break c; } } } else { cat };
            let e = d6(&mut self.rng);
            let name = self.foes[i].kind().name;
            let who = if player { "You".to_string() } else { format!("The {}", name) };
            let text = match (c, e) {
                (2, 1) => {
                    if player { self.p.lose_attack = true; self.foes[i].bonus = 10; } else { self.foes[i].lose_attack = true; }
                    format!("{} lose the next attack; the other side gets +10.", who)
                }
                (2, 2) => {
                    let d = o6(&mut self.rng).total;
                    if player {
                        let dmg = (d + self.held().dam + self.p.sheet.db() - ARMOURS[self.p.armour].1).max(0);
                        self.hurt_player(dmg);
                        if self.p.bp <= 0 { self.die("You fall on your own weapon"); }
                        format!("You hit yourself for {}.", dmg)
                    } else {
                        let dmg = (d + KINDS[self.foes[i].k].dam - KINDS[self.foes[i].k].ap).max(0);
                        self.foes[i].bp -= dmg;
                        format!("The {} hits itself for {}.", name, dmg)
                    }
                }
                (2, 3..=5) => format!("{} stumble, but no friend is near to hit.", who),
                (2, _) | (3, 1 | 2) => {
                    if player { self.p.strain = -3; } else { self.foes[i].status.push((-3, FIGHT)); }
                    format!("{} strain a muscle: -3 until mended.", who)
                }
                (3, 3) => {
                    let s = if player { self.p.sheet.total("Wield Weapon") } else { self.foes[i].kind().strength };
                    let roll = o6(&mut self.rng).total + s;
                    if roll < 10 { if player { self.p.lose_attack = true; } else { self.foes[i].lose_attack = true; } }
                    format!("{} weapon sticks (Strength {} against 10).", if player { "Your" } else { "Its" }, roll)
                }
                (3, 4) => {
                    if player { self.p.dropped = true; "You drop your weapon: -5 Def until you pick it up (g).".into() }
                    else { self.foes[i].disarmed = true; format!("The {} drops its weapon.", name) }
                }
                (3, _) => {
                    let dr = if e == 5 { 12 } else { 8 };
                    let t = if player { self.p.sheet.total("Tumble") } else { self.foes[i].kind().tumble };
                    let roll = o6(&mut self.rng).total + t;
                    if roll < dr { if player { self.p.down = true; } else { self.foes[i].down = true; } format!("{} fall down (Tumble {} against {}).", who, roll, dr) }
                    else { format!("{} stay on your feet (Tumble {} against {}).", who, roll, dr) }
                }
                (4, _) => {
                    let (pen, rounds) = match e {
                        1 => (-(o6(&mut self.rng).total.max(1) + 3), FIGHT),
                        2 => { let o = o6(&mut self.rng).total.max(1); (-o, o) }
                        3 => { let d = d6(&mut self.rng); (-d, d) }
                        4 => (-3, 3),
                        5 => (-3, 1),
                        _ => (-1, 1),
                    };
                    if player { self.p.status.push((pen, rounds)); } else { self.foes[i].status.push((pen, rounds)); }
                    format!("{} stunned: {} for {} rounds.", who, pen, rounds)
                }
                (5, _) => {
                    let pen = if e <= 3 { -2 } else { -1 };
                    if player { self.p.status.push((pen, FIGHT)); } else { self.foes[i].status.push((pen, FIGHT)); }
                    format!("{} tire: {} for the rest of the fight.", who, pen)
                }
                _ => ["Awkward looking.", "Giggles are heard.", "Laughter is heard.", "A bad showing.", "A very bad showing.", "Terrible to watch."][(6 - e) as usize].to_string(),
            };
            let cats = ["", "Special", "Unwanted effect", "Stun effect", "Added effect", "Impression"];
            let head = if rolls == 2 && r == 0 { "Fumble, rolled twice" } else { "Fumble" };
            self.say(&format!("{}: {} {}. {}", head, cats[(c - 1) as usize], e, text), if player { RED } else { GREEN });
        }
    }

    // -------------------------------------------------------------- other actions

    /// f: shoot at the nearest foe you can see. O6 + total against a DR by
    /// range; an aware foe adds +5, or its O6 + Dodge - 5 if better.
    fn fire(&mut self) {
        let Some(m) = self.p.missile else { self.say("You have nothing to shoot with.", DIM); return; };
        let ms = MISSILES[m];
        if self.p.ammo <= 0 { self.say(&format!("No {} left.", ms.ammo), RED); return; }
        let (px, py) = (self.p.x, self.p.y);
        let target = (0..self.foes.len())
            .filter(|&i| self.foes[i].bp > 0 && self.vis[idx(self.foes[i].x, self.foes[i].y)])
            .min_by_key(|&i| (self.foes[i].x - px).pow(2) + (self.foes[i].y - py).pow(2));
        let Some(i) = target else { self.say("No foe in sight.", DIM); return; };
        let k = *self.foes[i].kind();
        let dist = (((self.foes[i].x - px).pow(2) + (self.foes[i].y - py).pow(2)) as f32).sqrt();
        let range = ms.range as f32;
        let dr = if dist <= range / 2.0 { 5 } else if dist <= range { 10 } else if dist <= range * 2.0 { 15 } else { 20 };
        let mut aware = 0;
        self.tray.clear();
        self.tray_head = format!("SHOOTING THE {}", k.name.to_uppercase());
        if self.foes[i].awake {
            if k.dodge >= 7 {
                let r = o6(&mut self.rng);
                aware = (r.total + k.dodge - 5).max(5);
                self.line(k.name, &r, format!("dodges: {} + {} - 5 = {}", r.total, k.dodge, aware), DIM);
            } else { aware = 5; }
        }
        let skill = self.p.sheet.total(ms.skill);
        let mods = self.status() + self.wound() + self.light().0 + self.strength(ms.str_req).unwrap_or(-5);
        let r = o6(&mut self.rng);
        let total = r.total + skill + ms.off + mods;
        let need = dr + aware;
        self.line("You shoot", &r, format!("{} + {} {:+} {:+} = {}  vs DR {}", r.total, skill, ms.off, mods, total, need), TEXT);
        self.p.ammo -= 1;
        self.foes[i].awake = true;
        self.audio.play(&self.s.miss, 0.7);
        let (sx, sy) = self.screen_of(self.foes[i].x, self.foes[i].y);
        if r.fumble || (total < need && !r.crit) {
            self.float(sx, sy - 10, "miss", DIM);
            self.say(&format!("You miss ({} against DR {} {}).", total, dr, if aware > 0 { format!("+ {} aware", aware) } else { "unaware".into() }), DIM);
        } else {
            self.mark(ms.skill, 1);
            let d = o6(&mut self.rng);
            let dmg = (ms.dam + d.total - k.ap).max(0);
            self.line("Damage", &d, format!("{} + {} - AP {} = {}", d.total, ms.dam, k.ap, dmg), GOLD);
            self.foes[i].bp -= dmg;
            self.foes[i].flash = 0.15;
            self.float(sx, sy - 10, &format!("{}", dmg), GOLD);
            self.audio.play(&self.s.hit, 1.0);
            self.say(&format!("Your shot hits the {} for {} ({} against DR {}).", k.name, dmg, total, need), TEXT);
            // Half the shots can be picked up again.
            if m != 3 && self.rng.chance(0.5) { let (x, y) = (self.foes[i].x, self.foes[i].y); self.loot.push(Loot { x, y, item: Item::Ammo(1) }); }
        }
        if m == 3 { self.p.ammo += 1; }
        self.end_turn();
    }

    /// p: a healing potion, 2d6 BP.
    fn drink(&mut self) {
        if self.p.potions == 0 { self.say("You have no potion.", DIM); return; }
        self.p.potions -= 1;
        let (a, b) = (d6(&mut self.rng), d6(&mut self.rng));
        let m = self.p.sheet.bp_max();
        let before = self.p.bp;
        self.p.bp = (self.p.bp + a + b).min(m);
        self.tray.clear();
        self.tray_head = "A HEALING POTION".into();
        self.tray.push(Line { who: "2d6".into(), dice: vec![a as u8, b as u8], sum: format!("{} + {} = {} BP", a, b, a + b), color: GREEN });
        self.say(&format!("You drink: +{} BP.", self.p.bp - before), GREEN);
        self.audio.play(&self.s.drink, 1.0);
        self.end_turn();
    }

    fn foes_near(&self) -> bool {
        self.foes.iter().any(|f| f.bp > 0 && f.awake && self.vis[idx(f.x, f.y)])
    }

    /// m: Medical Lore against DR 8 stops bleeding, mends a strain and
    /// gives back a BP. Takes a minute, ten rounds.
    fn bandage(&mut self) {
        if self.foes_near() { self.say("Not with a foe in sight.", RED); return; }
        let r = o6(&mut self.rng);
        let skill = self.p.sheet.total("Medical Lore");
        let total = r.total + skill;
        self.tray.clear();
        self.tray_head = "BANDAGING".into();
        self.line("Medical Lore", &r, format!("{} + {} = {}  vs DR 8", r.total, skill, total), GREEN);
        if total >= 8 && !r.fumble {
            self.p.bleed = 0;
            self.p.strain = 0;
            if !self.p.bandaged && self.p.bp < self.p.sheet.bp_max() { self.p.bp += 1; }
            self.p.bandaged = true;
            self.say(&format!("You bind your wounds (Medical Lore {} against 8).", total), GREEN);
            self.mark("Medical Lore", 1);
        } else {
            self.say(&format!("Your bandage slips (Medical Lore {} against 8).", total), RED);
        }
        for _ in 0..10 {
            self.end_turn();
            if self.foes_near() || !matches!(self.mode, Mode::Play) { break; }
        }
    }

    /// r: rest; a BP back every hundred rounds, until healed or disturbed.
    /// The light burns while you rest, unless you douse it first.
    fn rest(&mut self) {
        if self.foes_near() { self.say("Not with a foe in sight.", RED); return; }
        self.say("You rest...", DIM);
        for n in 1..=600 {
            self.end_turn();
            if !matches!(self.mode, Mode::Play) { return; }
            if n % 100 == 0 && self.p.bp < self.p.sheet.bp_max() { self.p.bp += 1; }
            // Other contenders walk the maze too.
            if n % 50 == 0 && self.rng.chance(0.12) { self.wanderer(); }
            if self.foes_near() { self.say("Something comes. You stop resting.", RED); return; }
            if self.p.bp >= self.p.sheet.bp_max() { self.say("You are rested.", GREEN); return; }
        }
        self.say("You rest an hour.", DIM);
    }

    fn wanderer(&mut self) {
        let menu = LEVEL_FOES[self.level];
        let k = menu[self.rng.below(menu.len() as u32) as usize];
        for _ in 0..200 {
            let (x, y) = (self.rng.below(MW as u32) as i32, self.rng.below(MH as u32) as i32);
            if self.map[idx(x, y)] == T::Floor && !self.vis[idx(x, y)] && self.foe_at(x, y).is_none() && self.dist[idx(x, y)] < 40 {
                let mut f = Foe::new(k, x, y);
                f.awake = true;
                self.foes.push(f);
                return;
            }
        }
    }

    fn climb(&mut self) {
        match self.map[idx(self.p.x, self.p.y)] {
            T::Up => {
                self.audio.play(&self.s.climb, 1.0);
                let n = self.level + 1;
                self.build(n);
                self.say(&format!("You climb to level {}: {}.", n + 1, LEVEL_NAMES[n]), GOLD);
                self.banner = Some((LEVEL_NAMES[n].to_string(), 2.5));
            }
            T::Gate => {
                if self.foes.iter().any(|f| f.k == DEMON && f.bp > 0) {
                    self.say("The Raven Demon guards the gate. It must fall first.", RED);
                    return;
                }
                self.mode = Mode::Won(0.0);
                let score = self.score();
                if funkey::store::record_score(GAME, score) { self.high = score; }
                self.audio.stop(1);
                self.audio.play(&self.s.won, 1.0);
            }
            _ => self.say("There is no way up here.", DIM),
        }
    }

    fn kill(&mut self, i: usize) {
        let f = self.foes.remove(i);
        let k = f.kind();
        let (sx, sy) = self.screen_of(f.x, f.y);
        self.particles.burst(sx as f32, sy as f32, if k.big { 40 } else { 16 }, 70.0, 0.7, 0x902020);
        self.audio.play(&self.s.kill, 1.0);
        self.say(&format!("The {} falls.", k.name), GOLD);
        if k.sp > 0 {
            let n = (1..=k.sp / 2 + 1).map(|_| d6(&mut self.rng)).sum::<i32>() / 2 + 1;
            self.loot.push(Loot { x: f.x, y: f.y, item: Item::Silver(n) });
        }
        match f.k {
            RIVAL => {
                let item = if self.rng.chance(0.5) { Item::Weapon(1 + self.rng.below(WEAPONS.len() as u32 - 1) as usize) } else { Item::Armour(1 + self.rng.below(4) as usize) };
                self.loot.push(Loot { x: f.x, y: f.y, item });
            }
            TROLL => self.loot.push(Loot { x: f.x, y: f.y, item: Item::Gold(2 + d6(&mut self.rng) / 2) }),
            DEMON => {
                self.loot.push(Loot { x: f.x, y: f.y, item: Item::Gold(10 + d6(&mut self.rng)) });
                self.banner = Some(("THE RAVEN DEMON FALLS".into(), 3.0));
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------- drawing

    fn cam(&self) -> (i32, i32) {
        ((self.p.x - VW / 2).clamp(0, MW - VW), (self.p.y - VH / 2).clamp(0, MH - VH))
    }

    fn screen_of(&self, x: i32, y: i32) -> (i32, i32) {
        let (cx, cy) = self.cam();
        ((x - cx) * TILE + TILE / 2, (y - cy) * TILE + TILE / 2)
    }

    fn draw_map(&self, f: &mut Frame) {
        let (cx, cy) = self.cam();
        let r = self.radius() as f32;
        let flick = 0.93 + 0.07 * ((self.time * 9.0).sin() * 0.6 + (self.time * 23.0).sin() * 0.4);
        let w = W as usize;
        for ty in 0..VH {
            for tx in 0..VW {
                let (mx, my) = (cx + tx, cy + ty);
                let i = idx(mx, my);
                let (x0, y0) = (tx * TILE, ty * TILE);
                if !self.seen[i] {
                    f.rect(x0, y0, TILE, TILE, 0x000000);
                    continue;
                }
                let vis = self.vis[i];
                let b = if vis {
                    let d = (((mx - self.p.x).pow(2) + (my - self.p.y).pow(2)) as f32).sqrt();
                    ((1.0 - (d / (r + 0.8)).powi(2) * 0.8) * flick).clamp(0.15, 1.0)
                } else { 0.2 };
                let t = self.map[i];
                let v = (hash(mx, my) * 4.0) as usize % 4;
                let tex = match t {
                    T::Wall => {
                        let below = inside(mx, my + 1) && self.map[idx(mx, my + 1)] != T::Wall;
                        &self.tex[if below { 4 + v } else { 8 + v }]
                    }
                    _ => &self.tex[v],
                };
                for py in 0..TILE {
                    let row = (y0 + py) as usize * w;
                    for px in 0..TILE {
                        let c = tex[(py * TILE + px) as usize];
                        f.px[row + (x0 + px) as usize] = if vis { warm(c, b) } else { cold(c) };
                    }
                }
                match t {
                    T::Up => draw_stairs(f, x0, y0, vis),
                    T::Gate => draw_gate(f, x0, y0, vis),
                    _ => {}
                }
            }
        }
        // Traps you have found, and what lies about.
        for t in &self.traps {
            if t.found && self.seen[idx(t.x, t.y)] {
                let (sx, sy) = self.screen_of(t.x, t.y);
                if on_view(sx, sy) { draw_spikes(f, sx, sy, self.vis[idx(t.x, t.y)]); }
            }
        }
        for l in &self.loot {
            if !self.vis[idx(l.x, l.y)] { continue; }
            let (sx, sy) = self.screen_of(l.x, l.y);
            if !on_view(sx, sy) { continue; }
            let a = match l.item {
                Item::Silver(_) => 12,
                Item::Gold(_) => 13,
                Item::Potion => 14,
                Item::Torches(_) => 15,
                Item::Weapon(_) => 16,
                Item::Armour(_) => 17,
                Item::Missile(..) | Item::Ammo(_) => 18,
                Item::Lantern => 19,
                Item::Oil => 20,
            };
            put(f, &self.art[a], sx, sy);
        }
        for foe in &self.foes {
            if foe.bp <= 0 || !self.vis[idx(foe.x, foe.y)] { continue; }
            let (sx, sy) = self.screen_of(foe.x, foe.y);
            if !on_view(sx, sy) { continue; }
            let k = foe.kind();
            let bob = if foe.awake { ((self.time * 6.0 + foe.x as f32).sin() * 1.0) as i32 } else { 0 };
            let s = &self.art[k.art];
            let y = if k.big { sy - 6 } else { sy } + bob;
            if foe.flash > 0.0 { put_white(f, s, sx, y); } else { put(f, s, sx, y); }
            if !foe.awake { f.text(sx + 5, sy - 11, "Z", 0x9090c0); }
            if foe.down { f.text(sx - 6, sy - 11, "DOWN", DIM); }
            if foe.flee > 0 { f.text(sx - 6, sy - 11, "FLEES", GREEN); }
            if foe.bp < foe.max {
                let wbar = (foe.bp.max(0) * 12 / foe.max.max(1)).max(1);
                f.rect(sx - 6, sy + 8, 12, 2, 0x401010);
                f.rect(sx - 6, sy + 8, wbar, 2, 0xe04030);
            }
        }
        if !matches!(self.mode, Mode::Dead(_)) {
            let (sx, sy) = self.screen_of(self.p.x, self.p.y);
            put(f, &self.art[self.p.art], sx, sy);
            if self.p.lit && self.radius() > 1 {
                let c = if (self.time * 12.0) as i32 % 2 == 0 { 0xffd060 } else { 0xff9030 };
                f.rect(sx + 6, sy - 7, 2, 2, c);
            }
        }
    }

    fn draw_panel(&self, f: &mut Frame) {
        let x = PANEL;
        f.rect(x - 2, 0, W - x + 2, H, 0x0c0a08);
        f.vline(x - 2, 0, H, 0x3a3020);
        let p = &self.p;
        let s = &p.sheet;
        f.text_big(x + 4, 3, s.name, GOLD);
        f.text(x + 4 + Frame::text_width(s.name, true, 1) + 6, 5, &format!("SIZE {}", s.size), DIM);
        f.text(x + 4, 13, &format!("BODY {}  MIND {}  SPIRIT {}", s.chars[0], s.chars[1], s.chars[2]), TEXT);
        // Body Points.
        let m = s.bp_max();
        f.text(x + 4, 22, &format!("BP {}/{}", p.bp.max(0), m), TEXT);
        let bw = 120;
        f.rect(x + 60, 22, bw, 5, 0x301010);
        let fill = (p.bp.max(0) * bw / m.max(1)).clamp(0, bw);
        let bc = if p.bp * 4 <= m { 0xe03020 } else if p.bp * 2 <= m { 0xe0a020 } else { 0x40c040 };
        f.rect(x + 60, 22, fill, 5, bc);
        // Status.
        let mut st = Vec::new();
        match self.wound() { -4 => st.push("HEAVILY WOUNDED -4".to_string()), -2 => st.push("WOUNDED -2".to_string()), _ => {} }
        let sp: i32 = p.status.iter().map(|s| s.0).sum();
        if sp != 0 { st.push(format!("STATUS {}", sp)); }
        if p.strain != 0 { st.push("STRAINED -3".into()); }
        if p.bleed > 0 { st.push("BLEEDING".into()); }
        if p.panic > 0 { st.push(format!("PANIC {}", p.panic)); }
        if p.frozen > 0 { st.push("FROZEN".into()); }
        if p.down { st.push("DOWN -5".into()); }
        if p.dropped { st.push("UNARMED".into()); }
        let line = if st.is_empty() { "UNHURT".to_string() } else { st.join("  ") };
        f.text(x + 4, 31, &line, if st.is_empty() { GREEN } else { RED });
        // Stance.
        let stn = STANCES[p.stance];
        f.text(x + 4, 41, "STANCE", DIM);
        f.text(x + 32, 41, &format!("{} {}", p.stance + 1, stn.0), GOLD);
        f.text(x + 32, 48, stn.3, DIM);
        // The weapon and its numbers now.
        let (ini, off, def, dam, mods) = self.numbers();
        let w = self.held();
        let (skill, pen, sname) = self.weapon_skill();
        f.text(x + 4, 58, w.name, TEXT);
        f.text(x + 4, 65, &format!("I {}  O {}  D {}  d {:+}", ini, off, def, dam), TEXT);
        let (lo, ld) = self.light();
        let train = if pen != 0 { format!(" as {} {:+}", sname, pen) } else { String::new() };
        f.text(x + 4, 72, &format!("skill {}{}  wpn O{:+} D{:+}", skill, train, w.off, w.def), DIM);
        f.text(x + 4, 79, &format!("light {}/{}  other {:+}  DB {}", lo, ld, mods, s.db()), DIM);
        f.text(x + 4, 88, &format!("{}  AP {}", ARMOURS[p.armour].0, ARMOURS[p.armour].1), TEXT);
        if let Some(mi) = p.missile {
            let ms = MISSILES[mi];
            f.text(x + 4, 95, &format!("{} {} {}  total {}", ms.name, if mi == 3 { "~".to_string() } else { p.ammo.to_string() }, ms.ammo, s.total(ms.skill) + ms.off), TEXT);
        }
        let light = if !p.lit { "LIGHT OUT (t)".to_string() }
            else if p.lantern && p.oil > 0 { format!("LANTERN oil {}", p.oil) }
            else if p.burn > 0 { format!("TORCH {}  spare {}", p.burn, p.torches) }
            else { "NO LIGHT".to_string() };
        f.text(x + 4, 104, &light, if self.radius() > 1 { 0xffc060 } else { RED });
        f.text(x + 4, 111, &format!("{} sp  {} gp  potions {}", p.sp, p.gp, p.potions), GOLD);
        // Skills, tier by tier.
        f.text(x + 4, 122, "SKILL          CHAR+ATTR+SKILL", DIM);
        let mut y = 130;
        let mut rows: Vec<&(&str, i32, i32)> = s.skills.iter().filter(|k| k.1 > 0 || k.2 > 0).collect();
        rows.sort_by_key(|k| -(s.total(k.0)));
        for k in rows.iter().take(14) {
            let (c, a, r) = s.parts(k.0);
            f.text(x + 4, y, k.0, TEXT);
            f.text(x + 80, y, &format!("{}+{}+{} = {}", c, a, r, c + a + r), TEXT);
            let need = 5 * (r + 1);
            let bw = 30;
            f.rect(x + 150, y + 1, bw, 3, 0x262018);
            f.rect(x + 150, y + 1, (k.2 * bw / need).min(bw), 3, 0xc0a040);
            y += 8;
        }
        let dy = y + 3;
        f.text(x + 4, dy, &format!("DODGE {} = DEF +{}", s.total("Dodge"), s.total("Dodge") / 5), DIM);
        f.text(x + 4, dy + 7, &format!("WIELD WEAPON {} = STR", s.total("Wield Weapon")), DIM);
        // Where you are.
        f.text(x + 4, H - 38, &format!("LEVEL {} OF {}  {}", self.level + 1, LEVELS, LEVEL_NAMES[self.level]), GOLD);
        f.text(x + 4, H - 30, &format!("ROUND {}  ({} MIN)", self.turn, self.turn / 10), DIM);
        f.text(x + 4, H - 20, "1-6 STANCE  F FIRE  P POTION  M BANDAGE", DIM);
        f.text(x + 4, H - 13, "R REST  T LIGHT  G TAKE  < CLIMB  ? RULES  Q QUIT", DIM);
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 6, VERSION, 0x4a4438);
    }

    fn draw_band(&self, f: &mut Frame) {
        f.rect(0, BAND, PANEL - 2, H - BAND, 0x0c0a08);
        f.hline(0, BAND, PANEL - 2, 0x3a3020);
        let x0 = 4;
        f.text(x0, BAND + 4, if self.tray_head.is_empty() { "THE DICE" } else { &self.tray_head }, GOLD);
        let mut y = BAND + 13;
        // A long round shows its last seven rolls.
        for l in self.tray.iter().skip(self.tray.len().saturating_sub(7)) {
            f.text(x0, y + 2, &l.who, l.color);
            let mut dx = x0 + 52;
            for (n, &d) in l.dice.iter().enumerate().take(6) {
                draw_die(f, dx, y, d, hot_die(&l.dice, n), 1);
                dx += 11;
            }
            f.text(dx + 2, y + 2, &l.sum, l.color);
            y += 11;
        }
        // The log.
        let lx = 262;
        f.vline(lx - 4, BAND + 2, H - BAND - 4, 0x2a2418);
        // The newest lines, after wrapping, so a long message never hides them.
        let lines: Vec<(String, Rgb)> = self.log.iter()
            .flat_map(|(s, c)| wrap(s, 38).into_iter().map(move |l| (l, *c)))
            .collect();
        let rows = ((H - 8 - BAND - 4) / 7 + 1) as usize;
        let mut y = BAND + 4;
        for (l, c) in lines.iter().skip(lines.len().saturating_sub(rows)) {
            f.text(lx, y, l, *c);
            y += 7;
        }
    }

    fn draw_title(&self, f: &mut Frame) {
        f.clear(0x080604);
        for y in 0..H {
            for x in (0..W).step_by(2) {
                let n = hash(x / 6, y / 6);
                if n > 0.93 { f.put(x, y, 0x1c140c); }
            }
        }
        fancy_text(f, W / 2, 14, "THE ELIMINATOR", 5, self.time);
        f.text_centered(W / 2, 56, "AN AMAR RPG DUNGEON  D6GAMING.ORG", DIM, true, 1);
        let lore = [
            "Beneath the royal castle in Amaron lies the Eliminator.",
            "Every year the King's heralds call for contenders.",
            "One in ten walks out, and the King makes them noble. The rest are never seen again.",
        ];
        for (i, l) in lore.iter().enumerate() { f.text_centered(W / 2, 72 + i as i32 * 10, l, TEXT, true, 1); }
        let cs = contenders();
        for (i, c) in cs.iter().enumerate() {
            let (x, y, w, h) = (26 + i as i32 * 200, 110, 188, 186);
            let sel = i == self.pick;
            f.rect(x, y, w, h, if sel { 0x3a2c14 } else { 0x16120c });
            f.rect(x, y, w, 1, if sel { GOLD } else { 0x3a3020 });
            f.rect(x, y + h - 1, w, 1, if sel { GOLD } else { 0x3a3020 });
            f.blit_scaled(&self.art[c.art], x + 8, y + 10, 3, false);
            f.text_big(x + 60, y + 10, &format!("{} {}", i + 1, c.sheet.name), if sel { GOLD } else { TEXT });
            for (n, l) in wrap(c.sheet.blurb, 30).iter().enumerate() { f.text(x + 60, y + 22 + n as i32 * 7, l, DIM); }
            let s = &c.sheet;
            let w0 = &WEAPONS[c.weapon];
            let mut rows: Vec<(String, Rgb)> = vec![
                (format!("BP {}  DB {}  SIZE {}", s.bp_max(), s.db(), s.size), TEXT),
                (format!("{}  AP {}", ARMOURS[c.armour].0, ARMOURS[c.armour].1), TEXT),
                (format!("{}: {}", w0.name, s.total(w0.name)), TEXT),
            ];
            if let Some(m) = c.missile { rows.push((format!("{}: {}", MISSILES[m].name, s.total(MISSILES[m].skill)), TEXT)); }
            rows.push(("SKILLS BY THE CREATION RULES".into(), DIM));
            for &(n, r) in c.base.iter() {
                let bonus = c.bonus.iter().filter(|b| **b == n).count() as i32;
                let (ch, at, _) = s.parts(n);
                let t = format!("{} {}{}  = {}", n, r, if bonus > 0 { "+1".to_string() } else { String::new() }, ch + at + r + bonus);
                rows.push((t, if bonus > 0 { GOLD } else { DIM }));
            }
            for (n, (t, col)) in rows.iter().enumerate() { f.text(x + 8, y + 56 + n as i32 * 8, t, *col); }
        }
        f.text_centered(W / 2, 302, "ARROWS OR 1-3 CHOOSE  ENTER OR SPACE STARTS  ? RULES  Q QUITS", GOLD, true, 1);
        let pulse = 0.5 + 0.5 * (self.time * 3.0).sin();
        f.text_centered(W / 2, 313, "NEW TO AMAR? PRESS I: THE THREE TIERS, THE O6 AND A BLOW, IN THREE PAGES", mix(TEXT, GOLD, pulse), false, 1);
        f.text_centered(W / 2, 324, &format!("HIGH SCORE {}", self.high), TEXT, true, 1);
        f.text_centered(W / 2, 340, "ALL NEW ART AND MUSIC. AMAR RPG BY GEIR ISENE, D6GAMING.ORG", DIM, false, 1);
        f.text(W - 4 - Frame::text_width(VERSION, false, 1), H - 7, VERSION, 0x4a4438);
    }

    fn draw_help(&self, f: &mut Frame) {
        f.clear(0x000000);
        f.rect(20, 14, W - 40, H - 28, 0x0e0b08);
        f.rect(20, 14, W - 40, 1, GOLD);
        f.rect(20, H - 15, W - 40, 1, GOLD);
        f.text_centered(W / 2, 20, "THE RULES OF AMAR, AS THIS GAME PLAYS THEM", GOLD, true, 1);
        let lines = [
            ("O6", "Roll a d6. A 6 rolls on: +1 for every 4-6, stopping on 1-3. A 1 rolls on: -1 for every 1-3, stopping on 4-6."),
            ("", "Two 6s in a row: CRITICAL, always a success. Two 1s in a row: FUMBLE, always a failure."),
            ("TOTAL", "Characteristic + Attribute + Skill. A sellsword's Longsword is BODY 1 + Melee Combat 3 + Longsword 4 = 8."),
            ("ATTACK", "Your O6 + Off against the foe's O6 + Def. The higher total hits."),
            ("", "Off = weapon OFF + skill.  Def = weapon DEF + skill + Dodge/5."),
            ("DAMAGE", "O6 + weapon DAM + DB, less the armour's AP.  DB = (SIZE + Wield Weapon) / 3."),
            ("BP", "SIZE x 2 + Fortitude / 3. At half your BP you are wounded, -2 to all rolls; at a quarter -4. At 0 you fall."),
            ("STANCES", "1 Normal. 2 Offensive +3 Off -5 Def. 3 Defensive -5 Off +3 Def. 4 Only defend +5 Def, no attack."),
            ("", "5 Power hit: -5 Off, DB added twice. 6 Double attack: two attacks, each at -5 Off."),
            ("ROUND", "Six seconds. Initiative each round: weapon INI + Reaction Speed + O6. The winner strikes first."),
            ("LIGHT", "Torch or lantern: -1 Off, -2 Def. Darkness: -5 Off, -10 Def. A torch burns an hour, 600 rounds."),
            ("SURPRISE", "A sleeping foe: Def -10. A fleeing foe shows its back: Def -7. A foe on the ground: -5."),
            ("MISSILES", "O6 + total against DR 5 inside half range, 10 inside range. An aware foe adds +5, or O6 + Dodge - 5."),
            ("STEALTH", "Each round a sleeping foe near you rolls O6 + Alertness against your O6 + Move Quietly."),
            ("FEAR", "O6 + Courage against the monster's fear DR. Miss by 1-3: -1 a round. By 4: frozen. By 5 or more: you run."),
            ("MARKS", "Every success gives the skill a mark. At 5 x (rank + 1) marks a d6 of 2 or more raises it."),
            ("", "A critical adds a mark, a fumble takes one away. Skills lift attributes, attributes lift characteristics."),
            ("WEAPONS", "Untrained: a trained weapon of the same kind -1, the same group -3, another -5."),
            ("", "A weapon's STR above your Wield Weapon total: -1, -3, -5 to Off and Def; 4 above, you cannot wield it."),
            ("HEALING", "A potion heals 2d6 BP. Bandaging, Medical Lore against DR 8, stops bleeding and gives back 1 BP."),
            ("", "Resting gives back a BP every hundred rounds. The light burns while you rest; t douses it."),
            ("PHOOKA", "It mends a BP every round, until a weapon of iron cuts it. A club or a staff will not do."),
        ];
        let mut y = 34;
        for (h, t) in lines {
            if !h.is_empty() { f.text(30, y, h, GOLD); }
            for (n, chunk) in wrap(t, 128).iter().enumerate() { f.text(80, y + n as i32 * 7, chunk, TEXT); y += if n > 0 { 7 } else { 0 }; }
            y += 13;
        }
        f.text_centered(W / 2, H - 24, "ANY KEY GOES BACK", DIM, true, 1);
    }

    fn draw_intro(&self, f: &mut Frame) {
        f.clear(0x080604);
        f.rect(12, 8, W - 24, H - 16, 0x0e0b08);
        f.rect(12, 8, W - 24, 1, GOLD);
        f.rect(12, H - 9, W - 24, 1, GOLD);
        let heads = ["THREE TIERS", "THE O6", "A BLOW"];
        f.text_centered(W / 2, 14, heads[self.intro.page], GOLD, true, 2);
        f.text(W - 44, 18, &format!("{} / {}", self.intro.page + 1, INTRO_PAGES), DIM);
        let c = contenders().remove(self.pick);
        let keys = match self.intro.page {
            0 => { self.intro_tiers(f, &c); "UP AND DOWN PICK A SKILL    RIGHT OR ENTER: NEXT PAGE    Q: TITLE".to_string() }
            1 => { self.intro_o6(f); "SPACE ROLLS ONE    R ROLLS A HUNDRED    LEFT AND RIGHT: PAGES    Q: TITLE".to_string() }
            _ => {
                self.intro_blow_page(f, &c);
                format!("SPACE STRIKES    ENTER: INTO THE MAZE AS THE {}    LEFT: BACK    Q: TITLE", c.sheet.name)
            }
        };
        f.text_centered(W / 2, H - 22, &keys, GOLD, false, 1);
    }

    /// Page one: the sheet as a tree, characteristic to attribute to skill,
    /// with one skill's three tiers lit and summed.
    fn intro_tiers(&self, f: &mut Frame, c: &Contender) {
        let s = &c.sheet;
        let skills = tree_skills(s);
        let lit = skills[self.intro.skill.min(skills.len() - 1)];
        let (la, faint, line) = (attr_of(lit), 0x5a5448, 0x3a3020);
        let lc = ATTRS[la].1;
        f.text_centered(W / 2, 34, &format!("The {}'s sheet. Every skill sits under an attribute, every attribute under a characteristic.", s.name), DIM, false, 1);
        let (cx, ax, kx, bw) = (40, 214, 400, 140);
        let row = |i: usize| 58 + i as i32 * 14;
        let char_y = |c: usize| {
            let rows: Vec<usize> = (0..ATTRS.len()).filter(|&a| ATTRS[a].1 == c).collect();
            match (rows.first(), rows.last()) {
                (Some(&a), Some(&b)) => (row(a) + row(b)) / 2,
                _ => row(ATTRS.len()) + 4,
            }
        };
        f.text(cx, 48, "CHARACTERISTIC", DIM);
        f.text(ax, 48, "ATTRIBUTE", DIM);
        f.text(kx, 48, "SKILL", DIM);
        // The lines first, the lit path last so nothing covers it.
        for on in [false, true] {
            let col = if on { GOLD } else { line };
            for a in 0..ATTRS.len() {
                if (a == la) == on { f.line(cx + bw, char_y(ATTRS[a].1) + 5, ax, row(a) + 5, col); }
            }
            for (i, &k) in skills.iter().enumerate() {
                if (k == lit) == on { f.line(ax + bw, row(attr_of(k)) + 5, kx, row(i) + 5, col); }
            }
        }
        let boxed = |f: &mut Frame, x: i32, y: i32, name: &str, v: i32, on: bool, dim: bool| {
            f.rect(x, y, bw, 11, if on { 0x3a2c14 } else { 0x16120c });
            if on { f.rect(x, y, 2, 11, GOLD); }
            let col = if on { GOLD } else if dim { faint } else { TEXT };
            f.text(x + 5, y + 3, name, col);
            let v = v.to_string();
            f.text(x + bw - 5 - Frame::text_width(&v, false, 1), y + 3, &v, col);
        };
        for (i, name) in CHARS.iter().enumerate() { boxed(f, cx, char_y(i), name, s.chars[i], i == lc, s.chars[i] == 0); }
        f.text(cx + bw + 6, char_y(2) + 3, "MAGIC. NOT USED IN THE MAZE.", faint);
        for (a, (name, _)) in ATTRS.iter().enumerate() {
            let used = skills.iter().any(|k| attr_of(k) == a);
            boxed(f, ax, row(a), name, s.attrs[a], a == la, !used && s.attrs[a] == 0);
        }
        for (i, &k) in skills.iter().enumerate() { boxed(f, kx, row(i), k, s.rank(k), k == lit, false); }
        let (ch, at, rk) = s.parts(lit);
        f.text_centered(W / 2, 204, &format!("{} = {} {} + {} {} + {} {} = {}", lit, CHARS[lc], ch, ATTRS[la].0, at, lit, rk, ch + at + rk), GOLD, true, 1);
        let lines = [
            "A roll adds all three tiers. That sum is the skill's total, the number you roll with.",
            "Melee Combat helps every weapon you swing, and BODY helps everything your body does.",
            "An untrained weapon uses your best one: -1 if it is the same kind, -3 the same group, -5 any other.",
            "Use a skill well and it earns marks. Enough marks, and a d6 of 2 or more raises it by one.",
            "Each rise gives the attribute above a mark. Each rise of an attribute gives its characteristic one.",
            "So you grow from the bottom up: a skill first, then a talent, then the whole body or mind.",
        ];
        for (n, l) in lines.iter().enumerate() { f.text_centered(W / 2, 222 + n as i32 * 11, l, TEXT, false, 1); }
    }

    /// Page two: the O6, rolled by hand, with a count of every total.
    fn intro_o6(&self, f: &mut Frame) {
        let it = &self.intro;
        let rules = [
            "Every roll in Amar is the O6: a d6 that can keep on rolling.",
            "A 2, 3, 4 or 5 is what you get.",
            "A 6 rolls on: +1 for every 4, 5 or 6, until a 1, 2 or 3 stops it.",
            "A 1 rolls on: -1 for every 1, 2 or 3, until a 4, 5 or 6 stops it.",
            "Two 6s in a row: CRITICAL, a success whatever the totals. Two 1s in a row: FUMBLE, a failure.",
            "Most rolls land on 2 to 5, so every +1 on your sheet counts. But anything can happen.",
        ];
        for (n, l) in rules.iter().enumerate() { f.text(40, 36 + n as i32 * 9, l, if n == 0 { GOLD } else { TEXT }); }
        let y = 96;
        match &it.roll {
            None => {
                let pulse = 0.5 + 0.5 * (self.time * 4.0).sin();
                f.text(40, y + 10, "PRESS SPACE TO ROLL", mix(DIM, GOLD, pulse));
            }
            Some(r) => {
                let mut x = 40;
                let shown = r.dice.len().min(12);
                for (n, &d) in r.dice.iter().enumerate().take(shown) {
                    if it.age < n as f32 * 0.18 { break; }
                    draw_die(f, x, y, d, hot_die(&r.dice, n), 3);
                    let tag = match (n, r.dice[0], d) {
                        (0, 2..=5, _) => "DONE",
                        (0, _, _) => "ROLL ON",
                        (_, 6, 4..=6) => "+1",
                        (_, 1, 1..=3) => "-1",
                        _ => "STOP",
                    };
                    f.text_centered(x + 13, y + 31, tag, if tag == "DONE" || tag == "STOP" { DIM } else { TEXT }, false, 1);
                    x += 34;
                }
                if it.age >= (shown - 1) as f32 * 0.18 + 0.1 {
                    f.text_scaled(x + 6, y + 6, &format!("= {}", r.total), GOLD, true, 2);
                    let (v, col) = if r.crit { ("CRITICAL: TWO 6S IN A ROW", GOLD) } else if r.fumble { ("FUMBLE: TWO 1S IN A ROW", RED) } else { ("", TEXT) };
                    f.text(x + 6, y + 24, v, col);
                }
            }
        }
        // The count, one bar per total.
        let rolls: u32 = it.tally.iter().sum();
        f.text(40, 150, "EVERY ROLL YOU MAKE, COUNTED BY ITS TOTAL", DIM);
        let top = it.tally.iter().copied().max().unwrap_or(0).max(1);
        let (x0, base) = (48, 286);
        for (i, &n) in it.tally.iter().enumerate() {
            let t = i as i32 - 3;
            let x = x0 + i as i32 * 34;
            let h = (n as i64 * 104 / top as i64) as i32;
            let col = if t >= 6 { GOLD } else if t <= 1 { RED } else { 0x8a7a5a };
            f.rect(x, base - h, 26, h, col);
            if n > 0 { f.text_centered(x + 13, base - h - 8, &n.to_string(), DIM, false, 1); }
            let label = if i == 15 { "12+".to_string() } else { t.to_string() };
            f.text_centered(x + 13, base + 4, &label, TEXT, false, 1);
        }
        if rolls > 0 {
            let s = format!("ROLLS {}    AVERAGE {:.2}    CRITICALS {}    FUMBLES {}    EACH SHOULD COME ABOUT 1 IN 36",
                rolls, it.sum as f64 / rolls as f64, it.crits, it.fumbles);
            f.text_centered(W / 2, 302, &s, TEXT, false, 1);
        }
    }

    /// Page three: one blow of the picked contender against an Araxi, the
    /// sums spelled out.
    fn intro_blow_page(&self, f: &mut Frame, c: &Contender) {
        let it = &self.intro;
        let s = &c.sheet;
        let w = &WEAPONS[c.weapon];
        let k = &KINDS[ARAXI];
        let skill = s.total(w.name);
        let off = w.off + skill;
        // The two of them, face to face.
        let (me, foe) = (&self.art[c.art], &self.art[k.art]);
        let (mx, fx, sy) = (40, W - 40 - foe.w * 6, 44);
        f.blit_scaled(me, mx, sy, 6, false);
        f.blit_scaled(foe, fx, sy, 6, true);
        let under = sy + me.h * 6 + 6;
        f.text_centered(mx + me.w * 3, under, s.name, GOLD, false, 1);
        f.text_centered(mx + me.w * 3, under + 9, &format!("BP {}", s.bp_max()), TEXT, false, 1);
        f.text_centered(fx + foe.w * 3, under, "ARAXI", RED, false, 1);
        let left = it.araxi.max(0);
        f.rect(fx, under + 9, foe.w * 6, 4, 0x3a1010);
        f.rect(fx, under + 9, foe.w * 6 * left / k.bp, 4, RED);
        f.text_centered(fx + foe.w * 3, under + 16, &format!("BP {} OF {}", left, k.bp), TEXT, false, 1);
        // The sums.
        let cx = W / 2;
        f.text_centered(cx, 40, &format!("Your Off = {} Off {} + {} total {} = {}", w.name, w.off, w.name, skill, off), TEXT, false, 1);
        f.text_centered(cx, 50, &format!("The Araxi's Def = {}. Its armour stops {} of any blow: AP {}.", k.def, k.ap, k.ap), TEXT, false, 1);
        f.text_centered(cx, 60, &format!("Your damage = O6 + {} Dam {} + DB {} - its AP {}", w.name, w.dam, s.db(), k.ap), TEXT, false, 1);
        f.text_centered(cx, 70, "Your O6 + Off against its O6 + Def. The higher total hits.", GOLD, false, 1);
        match &it.blow {
            None => {
                let pulse = 0.5 + 0.5 * (self.time * 4.0).sin();
                f.text_centered(cx, 110, "PRESS SPACE TO STRIKE", mix(DIM, GOLD, pulse), true, 1);
            }
            Some(b) => {
                let x = 150;
                let (n, a_down) = dice_row(f, x, 84, "YOU", &b.a, &format!("{} + Off {} = {}", b.a.total, off, b.at), TEXT, 0, it.age);
                let (n, d_down) = dice_row(f, x, 106, "ARAXI", &b.d, &format!("{} + Def {} = {}", b.d.total, k.def, b.dt), DIM, n, it.age);
                if a_down && d_down {
                    let (v, col) = if b.a.fumble { ("FUMBLE: YOUR BLOW GOES WILD".to_string(), RED) }
                        else if b.a.crit && b.hit { ("CRITICAL: IT LANDS WHATEVER THE TOTALS".to_string(), GOLD) }
                        else if b.d.crit && !b.hit { ("ITS CRITICAL: IT PARRIES WHATEVER THE TOTALS".to_string(), DIM) }
                        else if b.d.fumble && b.hit { ("ITS FUMBLE: YOUR BLOW GETS THROUGH".to_string(), GOLD) }
                        else if b.hit { (format!("HIT: {} BEATS {}", b.at, b.dt), GOLD) }
                        else { (format!("MISS: {} DOES NOT BEAT {}", b.at, b.dt), DIM) };
                    f.text_centered(cx, 132, &v, col, true, 1);
                }
                if let Some((r, dmg)) = &b.dam {
                    let sum = format!("{} + Dam {} + DB {} - AP {} = {}", r.total, w.dam, s.db(), k.ap, dmg);
                    let (_, down) = dice_row(f, x, 146, "DAMAGE", r, &sum, GOLD, n + 3, it.age);
                    if down && it.araxi <= 0 { f.text_centered(cx, 172, "THE ARAXI FALLS. SPACE FOR ANOTHER.", GOLD, true, 1); }
                }
            }
        }
        let lines = [
            format!("Body Points: SIZE x 2 + Fortitude / 3. The {}: {} x 2 + {} / 3 = {} BP.", s.name, s.size, s.total("Fortitude"), s.bp_max()),
            "At half your BP you are wounded: -2 to every roll. At a quarter: -4. At 0 you fall.".to_string(),
            format!("DB, the damage bonus: (SIZE {} + Wield Weapon {}) / 3 = {}.", s.size, s.total("Wield Weapon"), s.db()),
            "In the maze, keys 1 to 6 trade Off for Def: offensive +3/-5, defensive -5/+3, only defend +5.".to_string(),
            "A power hit adds DB twice at -5 Off. A double attack strikes twice, each at -5 Off.".to_string(),
            "Your torch costs -1 Off and -2 Def. The dark costs -5 and -10. Carry spare torches.".to_string(),
        ];
        for (n, l) in lines.iter().enumerate() { f.text_centered(W / 2, 200 + n as i32 * 12, l, TEXT, false, 1); }
    }
}

// ------------------------------------------------------------------ helpers

const DIRS: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

fn tick(v: &mut Vec<(i32, i32)>) {
    for s in v.iter_mut() { s.1 -= 1; }
    v.retain(|s| s.1 > 0);
}

fn on_view(x: i32, y: i32) -> bool { x >= 0 && y >= 0 && x < VW * TILE && y < VH * TILE }

fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0xff_ffff) as f32 / 16_777_216.0
}

fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in s.split(' ') {
        if !line.is_empty() && line.len() + 1 + word.len() > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() { line.push(' '); }
        line.push_str(word);
    }
    if !line.is_empty() { out.push(line); }
    out
}

fn tint(c: Rgb, k: f32) -> Rgb {
    let (r, g, b) = parts(c);
    let s = |v: u8| (v as f32 * k).round().clamp(0.0, 255.0) as u8;
    rgb(s(r), s(g), s(b))
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let ((ar, ag, ab), (br, bg, bb)) = (parts(a), parts(b));
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    rgb(l(ar, br), l(ag, bg), l(ab, bb))
}

/// Torchlight: warm, falling off with distance.
fn warm(c: Rgb, b: f32) -> Rgb {
    let (r, g, bl) = parts(c);
    rgb((r as f32 * b * 1.55).min(255.0) as u8, (g as f32 * b * 1.35).min(255.0) as u8, (bl as f32 * b * 1.05).min(255.0) as u8)
}

/// Remembered, out of the light: grey-blue and dim.
fn cold(c: Rgb) -> Rgb {
    let (r, g, b) = parts(c);
    let l = (r as f32 * 0.3 + g as f32 * 0.5 + b as f32 * 0.2) * 0.22;
    rgb((l * 0.8) as u8, (l * 0.9) as u8, (l * 1.2).min(255.0) as u8)
}

/// Floor, wall face and wall top textures for a level, four of each.
fn textures(level: usize) -> Vec<Vec<Rgb>> {
    let pal: [(Rgb, Rgb, Rgb); LEVELS] = [
        (0x5a4632, 0x7a6450, 0x2e241c),
        (0x4c4c54, 0x70707c, 0x26262c),
        (0x3c4c44, 0x547060, 0x1c2a24),
        (0x4c3c5a, 0x705a88, 0x281e36),
        (0x523434, 0x7a4c4e, 0x2c1618),
    ];
    let (floor, wall, top) = pal[level];
    let mut out = Vec::new();
    for v in 0..4 {
        out.push((0..TILE * TILE).map(|i| {
            let (x, y) = (i % TILE, i / TILE);
            let grout = x == 0 || y == 0 || (x == 7 && (y + v) % 14 < 7) || (y == 7 && v % 2 == 0);
            let n = hash(x + v * 31, y + v * 17);
            if grout { tint(floor, 0.55) } else { tint(floor, 0.85 + 0.3 * n) }
        }).collect());
    }
    for v in 0..4 {
        out.push((0..TILE * TILE).map(|i| {
            let (x, y) = (i % TILE, i / TILE);
            if y < 3 { return tint(top, 1.3 + 0.2 * hash(x + v, y)); }
            let row = (y - 3) / 4;
            let off = if row % 2 == 0 { 0 } else { 4 };
            let mortar = (y - 3) % 4 == 0 || (x + off + v) % 8 == 0;
            let n = hash(x + v * 7, y + v * 3);
            if mortar { tint(wall, 0.5) } else { tint(wall, (0.8 + 0.3 * n) * (1.0 - (y - 3) as f32 * 0.02)) }
        }).collect());
    }
    for v in 0..4 {
        out.push((0..TILE * TILE).map(|i| {
            let (x, y) = (i % TILE, i / TILE);
            tint(top, 0.8 + 0.4 * hash(x + v * 5, y + v * 11))
        }).collect());
    }
    out
}

fn draw_stairs(f: &mut Frame, x: i32, y: i32, vis: bool) {
    for s in 0..4 {
        let c = if vis { tint(0xc0a878, 1.0 - s as f32 * 0.15) } else { 0x303038 };
        f.rect(x + 2 + s, y + 2 + s * 3, TILE - 4 - s * 2, 2, c);
    }
}

fn draw_gate(f: &mut Frame, x: i32, y: i32, vis: bool) {
    let c = if vis { 0xb0a080 } else { 0x303038 };
    f.rect(x, y, TILE, TILE, if vis { 0xfff0c0 } else { 0x202028 });
    for k in 0..4 { f.rect(x + 1 + k * 4, y, 1, TILE, c); }
    f.rect(x, y + 4, TILE, 1, c);
    f.rect(x, y + 9, TILE, 1, c);
}

fn draw_spikes(f: &mut Frame, x: i32, y: i32, vis: bool) {
    let c = if vis { 0xd0d0d8 } else { 0x505060 };
    for k in -1..=1 {
        let bx = x + k * 4;
        f.put(bx, y - 2, c);
        f.rect(bx - 1, y - 1, 3, 1, c);
        f.rect(bx - 1, y, 3, 2, tint(c, 0.6));
    }
}

/// A die 9 pixels square, times `s`.
fn draw_die(f: &mut Frame, x: i32, y: i32, v: u8, hot: bool, s: i32) {
    let face = if hot { 0xffd040 } else { 0xf0ece0 };
    f.rect(x + s, y, 7 * s, 9 * s, face);
    f.rect(x, y + s, 9 * s, 7 * s, face);
    let pip = |f: &mut Frame, px: i32, py: i32| f.rect(x + px * s, y + py * s, 2 * s, 2 * s, if v == 1 || v == 6 { 0xb02020 } else { 0x181410 });
    let (l, m, r, t, c, b) = (1, 4, 6, 1, 4, 6);
    match v {
        1 => pip(f, m - 0, c - 0),
        2 => { pip(f, l, t); pip(f, r, b); }
        3 => { pip(f, l, t); pip(f, m, c); pip(f, r, b); }
        4 => { pip(f, l, t); pip(f, r, t); pip(f, l, b); pip(f, r, b); }
        5 => { pip(f, l, t); pip(f, r, t); pip(f, m, c); pip(f, l, b); pip(f, r, b); }
        _ => { pip(f, l, t); pip(f, r, t); pip(f, l, c); pip(f, r, c); pip(f, l, b); pip(f, r, b); }
    }
}

/// Is this die the second of two 6s, or of two 1s, in a row?
fn hot_die(dice: &[u8], n: usize) -> bool {
    n > 0 && dice[n - 1] == dice[n] && (dice[0] == 6 && dice[n] == 6 || dice[0] == 1 && dice[n] == 1)
}

/// A labelled row of dice for the intro, landing one by one: die `start`
/// of the page lands `start` x 0.12 seconds after the roll. The sum shows
/// once the last is down. Returns the dice counted so far and whether
/// this row is all down.
fn dice_row(f: &mut Frame, x: i32, y: i32, who: &str, r: &Roll, sum: &str, c: Rgb, start: usize, age: f32) -> (usize, bool) {
    let end = start + r.dice.len();
    f.text(x, y + 7, who, c);
    let mut dx = x + 44;
    for (n, &d) in r.dice.iter().enumerate().take(8) {
        if age < (start + n) as f32 * 0.12 { return (end, false); }
        draw_die(f, dx, y, d, hot_die(&r.dice, n), 2);
        dx += 21;
    }
    f.text(dx + 4, y + 7, sum, c);
    (end, true)
}

fn put(f: &mut Frame, s: &Sprite, x: i32, y: i32) { f.blit(s, x - s.w / 2, y - s.h / 2); }

fn put_white(f: &mut Frame, s: &Sprite, x: i32, y: i32) {
    let (x0, y0) = (x - s.w / 2, y - s.h / 2);
    for yy in 0..s.h {
        for xx in 0..s.w {
            if s.px[(yy * s.w + xx) as usize] >> 24 != 0 { f.put(x0 + xx, y0 + yy, WHITE); }
        }
    }
}

/// Twice the size, with the diagonal steps smoothed (the Scale2x rule).
fn scale2x(s: &Sprite) -> Sprite {
    let (w, h) = (s.w, s.h);
    let at = |x: i32, y: i32| if x < 0 || y < 0 || x >= w || y >= h { 0 } else { s.px[(y * w + x) as usize] };
    let mut px = vec![0u32; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let (p, a, b, c, d) = (at(x, y), at(x, y - 1), at(x + 1, y), at(x - 1, y), at(x, y + 1));
            let e0 = if c == a && c != d && a != b { a } else { p };
            let e1 = if a == b && a != c && b != d { b } else { p };
            let e2 = if d == c && d != b && c != a { c } else { p };
            let e3 = if b == d && b != a && d != c { d } else { p };
            let (i, w2) = ((y * 2 * w * 2 + x * 2) as usize, (w * 2) as usize);
            px[i] = e0;
            px[i + 1] = e1;
            px[i + w2] = e2;
            px[i + w2 + 1] = e3;
        }
    }
    Sprite { w: w * 2, h: h * 2, px }
}

fn shade(s: &Sprite) -> Sprite {
    let on = |x: i32, y: i32| x >= 0 && y >= 0 && x < s.w && y < s.h && s.px[(y * s.w + x) as usize] >> 24 != 0;
    let mut out = s.clone();
    for y in 0..s.h {
        for x in 0..s.w {
            let i = (y * s.w + x) as usize;
            if s.px[i] >> 24 == 0 { continue; }
            let k = if !on(x, y - 1) || !on(x - 1, y) { 1.25 } else if !on(x, y + 1) || !on(x + 1, y) { 0.72 } else { 1.0 };
            out.px[i] = 0xff00_0000 | tint(s.px[i] & 0xff_ffff, k);
        }
    }
    out
}

fn outline(s: &Sprite, c: Rgb) -> Sprite {
    let (w, h) = (s.w + 2, s.h + 2);
    let on = |x: i32, y: i32| x >= 1 && y >= 1 && x <= s.w && y <= s.h && s.px[((y - 1) * s.w + x - 1) as usize] >> 24 != 0;
    let mut px = vec![0u32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            px[(y * w + x) as usize] = if on(x, y) { s.px[((y - 1) * s.w + x - 1) as usize] }
                else if on(x - 1, y) || on(x + 1, y) || on(x, y - 1) || on(x, y + 1) { 0xff00_0000 | c } else { 0 };
        }
    }
    Sprite { w, h, px }
}

fn fancy(rows: &[&str], palette: &[(char, Rgb)]) -> Sprite {
    outline(&shade(&scale2x(&Sprite::from_rows(rows, palette))), 0x0c0806)
}

/// Every picture: the three contenders, the foes, the things lying about.
fn art() -> Vec<Sprite> {
    let man = |b: Rgb, h: Rgb, w: Rgb| fancy(&["..hhh..", "..sss..", ".bbbbb.", "s.bbb.w", "..bbb.w", "..l.l..", ".ll.ll."],
        &[('h', h), ('s', 0xe0b090), ('b', b), ('l', 0x4a3420), ('w', w)]);
    vec![
        man(0x3a6ad0, 0x8a8e98, 0xd8e0e8),
        man(0x4a8a3a, 0x6a4020, 0x8a6a40),
        man(0xa0502a, 0x302018, 0x9a9aa8),
        fancy(&[".......", ".......", "....rr.", ".rrrrer", "rrrrrr.", "t..r.r.", "......."], &[('r', 0x8a7a6a), ('e', 0xff4040), ('t', 0xd09080)]),
        fancy(&["l.....l", ".l.s.l.", "..sss..", "llsesll", "..sss..", ".l...l.", "l.....l"], &[('l', 0x2a2630), ('s', 0x4a3a58), ('e', 0xff3030)]),
        fancy(&["..kkk..", ".kekek.", "..kkk.c", ".kkkkkc", "k.kkk.c", "..k.k..", ".kk.kk."], &[('k', 0x3a3036), ('e', 0xff2020), ('c', 0x8a6a40)]),
        fancy(&[".......", "g....gg", "gg..gge", ".ggggg.", ".ggggg.", ".g.g.g.", "......."], &[('g', 0x7a7a84), ('e', 0xffd040)]),
        fancy(&["..zzz..", "..eze..", "zzzzzzz", "..zzz..", "..zzz..", "..z.z..", ".zz.zz."], &[('z', 0x6a8a5a), ('e', 0xd0ff60)]),
        man(0xb03030, 0x9aa0a8, 0xd8e0e8),
        fancy(&["h.....h", ".hgggh.", "..eee..", "..ggg..", ".ggggg.", "..g.g..", ".gg.gg."], &[('h', 0xd0c8a0), ('g', 0x8a7060), ('e', 0xa0e0ff)]),
        fancy(&["...tttt...", "..tettet..", "..tttttt..", ".tttttttt.", "tt.tttt.tt", "t..tttt..c", "...tttt..c", "...t..t..c", "..tt..tt..", ".........."],
            &[('t', 0x5a7a4a), ('e', 0xffe040), ('c', 0x6a4a2a)]),
        fancy(&["w.......w.", "ww..kk..ww", "wwwkekkwww", ".wwkkkkww.", "..wkkkkw..", "...kkkk...", "...kyyk...", "...k..k...", "..kk..kk..", ".........."],
            &[('w', 0x2a2034), ('k', 0x3c3450), ('e', 0xff3020), ('y', 0xffa020)]),
        fancy(&[".ss..", "swss.", ".ss.s", "..sws", "...s."], &[('s', 0xb8bcc8), ('w', 0xffffff)]),
        fancy(&[".ss..", "swss.", ".ss.s", "..sws", "...s."], &[('s', 0xe8c040), ('w', 0xfff8c0)]),
        fancy(&["..c..", "..g..", ".rrr.", "rrwrr", ".rrr."], &[('c', 0x8a6a40), ('g', 0xc0e0ff), ('r', 0xd03040), ('w', 0xffffff)]),
        fancy(&["..y..", ".yoy.", "..o..", "..b..", "..b.."], &[('y', 0xffe060), ('o', 0xff8020), ('b', 0x6a4a2a)]),
        fancy(&["....w", "...w.", "..w..", "hh...", "h...."], &[('w', 0xd0d8e0), ('h', 0x8a6a40)]),
        fancy(&["a...a", "aaaaa", ".aaa.", ".aaa.", ".a.a."], &[('a', 0x9a9aa8)]),
        fancy(&["..f..", ".fff.", "..s..", "..s..", ".s.s."], &[('f', 0xd0d0d8), ('s', 0x8a6a40)]),
        fancy(&[".hhh.", "h...h", "hyyyh", "hyyyh", ".hhh."], &[('h', 0x8a8070), ('y', 0xffd060)]),
        fancy(&["..c..", ".ooo.", "ooooo", "ooooo", ".ooo."], &[('c', 0x8a6a40), ('o', 0xa08040)]),
    ]
}

fn fancy_text(f: &mut Frame, cx: i32, y: i32, s: &str, scale: i32, time: f32) {
    let (w, h) = (Frame::text_width(s, true, scale), 7 * scale);
    let mut m = Frame::new(w + 1, h + 1);
    m.text_scaled(0, 0, s, WHITE, true, scale);
    let x0 = cx - w / 2;
    let lit = |xx: i32, yy: i32| m.get(xx, yy) != BLACK;
    for yy in 0..h { for xx in 0..w { if lit(xx, yy) { f.put(x0 + xx + scale / 2 + 1, y + yy + scale / 2 + 1, 0x000000); } } }
    let band = ((time * 0.4).fract() * (w + h) as f32 * 1.6) as i32 - h;
    for yy in 0..h {
        for xx in 0..w {
            if !lit(xx, yy) { continue; }
            let mut c = mix(0xffe8a0, 0xb04010, yy as f32 / h as f32);
            let d = (xx + yy - band).abs();
            if d < scale * 2 { c = mix(c, WHITE, 1.0 - d as f32 / (scale * 2) as f32); }
            f.put(x0 + xx, y + yy, c);
        }
    }
}

fn duet(tune: &str, bass: &str, vol: f32) -> Sample {
    let a = Tune::parse(tune, Wave::Triangle, 0.2 * vol).render();
    let b = Tune::parse(bass, Wave::Triangle, 0.3 * vol).render();
    let at = |s: &Sample, i: usize| *s.data.get(i).unwrap_or(&0) as i32;
    let n = a.data.len().max(b.data.len());
    Sample::from_i16((0..n).map(|i| (at(&a, i) + at(&b, i)).clamp(-32768, 32767) as i16).collect())
}

fn sounds() -> Sounds {
    let mut rattle = Vec::new();
    for k in 0..4 {
        rattle.extend_from_slice(&Sample::noise(0.012, 0.25 - k as f32 * 0.04).data);
        rattle.resize(rattle.len() + 900 + k * 300, 0);
    }
    Sounds {
        dice: Sample::from_i16(rattle),
        hit: Sample::noise(0.08, 0.35).then(&Sample::sweep(Wave::Square, 260.0, 110.0, 0.09, 0.22)),
        miss: Sample::sweep(Wave::Sine, 1300.0, 380.0, 0.12, 0.14),
        crit: Tune::parse("420 c6/16 e6/16 g6/16 c7/8", Wave::Square, 0.3).render(),
        fumble: Sample::sweep(Wave::Saw, 420.0, 90.0, 0.35, 0.28),
        coin: Tune::parse("520 e6/32 b6/16", Wave::Square, 0.25).render(),
        drink: Sample::sweep(Wave::Sine, 300.0, 900.0, 0.3, 0.3),
        climb: Tune::parse("200 g4/8 c5/8 e5/8 g5/4", Wave::Triangle, 0.4).render(),
        rise: Tune::parse("300 g5/16 b5/16 d6/16 g6/8", Wave::Triangle, 0.45).render(),
        die: Sample::sweep(Wave::Saw, 500.0, 50.0, 1.2, 0.4),
        fear: Sample::sweep(Wave::Saw, 140.0, 55.0, 0.8, 0.35),
        kill: Sample::noise(0.25, 0.3).then(&Sample::sweep(Wave::Triangle, 220.0, 60.0, 0.3, 0.3)),
        won: Tune::parse("160 c5/8 e5/8 g5/8 c6/4 g5/8 c6/2", Wave::Triangle, 0.5).render(),
        music: duet("70 a3/2 c4/4 e4/4 d4/2 c4/2 b3/2 g3/2 a3/1", "70 a2/1 f2/1 g2/1 a2/1", 0.8),
        title: duet("100 d4/4 f4/4 a4/2 g4/4 f4/4 e4/2 d4/4 e4/4 f4/4 a4/4 d4/1", "100 d3/1 c3/1 bb2/1 d3/1", 1.0),
    }
}

impl Game for Eliminator {
    fn update(&mut self, input: &Input, dt: f32) -> Flow {
        self.time += dt;
        self.particles.update(dt);
        for fl in &mut self.floats { fl.4 += dt; fl.1 -= 18.0 * dt; }
        self.floats.retain(|fl| fl.4 < 1.1);
        for foe in &mut self.foes { foe.flash -= dt; }
        if let Some((_, t)) = self.banner.as_mut() { *t -= dt; if *t <= 0.0 { self.banner = None; } }
        let pressed = |c: char| input.pressed(Key::Char(c));
        // q does what Esc does: some browsers keep Esc for themselves.
        let quit = input.pressed(Key::Escape) || pressed('q');
        match self.mode {
            Mode::Title => {
                if quit { return Flow::Quit; }
                if input.pressed(Key::Left) || pressed('h') { self.pick = (self.pick + 2) % 3; }
                if input.pressed(Key::Right) || pressed('l') { self.pick = (self.pick + 1) % 3; }
                for (i, c) in ['1', '2', '3'].iter().enumerate() { if pressed(*c) { self.pick = i; self.start(i); return Flow::Continue; } }
                if pressed('?') { self.mode = Mode::Help; self.help_page = 0; }
                if pressed('i') { self.open_intro(); return Flow::Continue; }
                if input.pressed(Key::Enter) || input.pressed(Key::Space) { let p = self.pick; self.start(p); }
            }
            Mode::Intro => {
                self.intro.age += dt;
                if quit { self.mode = Mode::Title; return Flow::Continue; }
                let last = self.intro.page + 1 == INTRO_PAGES;
                if last && input.pressed(Key::Enter) { let p = self.pick; self.start(p); return Flow::Continue; }
                if input.pressed(Key::Left) || pressed('h') { self.intro.page = self.intro.page.saturating_sub(1); }
                else if !last && (input.pressed(Key::Right) || pressed('l') || input.pressed(Key::Enter)) { self.intro.page += 1; }
                else {
                    match self.intro.page {
                        0 => {
                            let n = contenders()[self.pick].sheet.skills.len();
                            if input.pressed(Key::Down) || pressed('j') { self.intro.skill = (self.intro.skill + 1) % n; }
                            if input.pressed(Key::Up) || pressed('k') { self.intro.skill = (self.intro.skill + n - 1) % n; }
                        }
                        1 => {
                            if input.pressed(Key::Space) { self.intro_roll(1); }
                            if pressed('r') { self.intro_roll(100); }
                        }
                        _ => if input.pressed(Key::Space) { self.intro_blow(); },
                    }
                }
            }
            Mode::Help => {
                if input.any_pressed() { self.mode = if self.p.bp > 0 && self.turn > 0 || !self.tray.is_empty() || !self.log.is_empty() { Mode::Play } else { Mode::Title }; }
            }
            Mode::Dead(t) | Mode::Won(t) => {
                let t = t + dt;
                self.mode = if matches!(self.mode, Mode::Dead(_)) { Mode::Dead(t) } else { Mode::Won(t) };
                if t > 1.5 && input.any_pressed() {
                    if quit { return Flow::Quit; }
                    self.mode = Mode::Title;
                    self.build(0);
                    self.audio.play_loop(1, &self.s.title, 0.7);
                }
            }
            Mode::Play => {
                if quit { return Flow::Quit; }
                let moves: [(Key, char, (i32, i32)); 4] = [(Key::Up, 'k', (0, -1)), (Key::Down, 'j', (0, 1)), (Key::Left, 'h', (-1, 0)), (Key::Right, 'l', (1, 0))];
                let diag: [(char, (i32, i32)); 4] = [('y', (-1, -1)), ('u', (1, -1)), ('b', (-1, 1)), ('n', (1, 1))];
                let mut dir = None;
                let mut fresh = false;
                for (k, c, d) in moves {
                    if input.pressed(k) || pressed(c) { dir = Some(d); fresh = true; }
                    else if dir.is_none() && (input.held(k) || input.held(Key::Char(c))) { dir = Some(d); }
                }
                for (c, d) in diag {
                    if pressed(c) { dir = Some(d); fresh = true; }
                    else if dir.is_none() && input.held(Key::Char(c)) { dir = Some(d); }
                }
                self.hold -= dt;
                if let Some((dx, dy)) = dir {
                    if fresh { self.hold = 0.28; self.step(dx, dy); }
                    else if self.hold <= 0.0 { self.hold = 0.11; self.step(dx, dy); }
                    return Flow::Continue;
                }
                for n in 0..6 {
                    if pressed(char::from(b'1' + n as u8)) {
                        self.p.stance = n;
                        self.say(&format!("Stance: {} ({}).", STANCES[n].0, STANCES[n].3), BLUE);
                    }
                }
                if input.pressed(Key::Tab) {
                    self.p.stance = (self.p.stance + 1) % 6;
                    let s = STANCES[self.p.stance];
                    self.say(&format!("Stance: {} ({}).", s.0, s.3), BLUE);
                }
                if pressed('f') { self.fire(); }
                if pressed('p') { self.drink(); }
                if pressed('m') { self.bandage(); }
                if pressed('r') { self.rest(); }
                if pressed('g') { self.take(); }
                if pressed('s') || pressed('.') { self.end_turn(); }
                if pressed('<') || input.pressed(Key::Enter) { self.climb(); }
                if pressed('?') { self.mode = Mode::Help; }
                if pressed('t') {
                    self.p.lit = !self.p.lit;
                    self.say(if self.p.lit { "You light your light." } else { "You douse your light. Darkness: -5 Off, -10 Def." }, DIM);
                    self.fov();
                }
            }
        }
        Flow::Continue
    }

    fn draw(&mut self, f: &mut Frame) {
        match self.mode {
            Mode::Title => { self.draw_title(f); return; }
            Mode::Intro => { self.draw_intro(f); return; }
            Mode::Help if self.turn == 0 && self.log.is_empty() => { self.draw_title(f); self.draw_help(f); return; }
            _ => {}
        }
        f.clear(0x000000);
        self.draw_map(f);
        self.particles.draw(f, 0, 0);
        for (x, y, s, c, t) in &self.floats {
            let c = if *t > 0.7 { mix(*c, 0x000000, (*t - 0.7) / 0.4) } else { *c };
            f.text_centered(*x as i32, *y as i32, s, c, true, 1);
        }
        if let Some((s, t)) = &self.banner {
            let c = if *t < 0.5 { mix(GOLD, 0x000000, 1.0 - *t / 0.5) } else { GOLD };
            f.text_centered(VW * TILE / 2, 40, s, c, true, 2);
        }
        self.draw_band(f);
        self.draw_panel(f);
        match self.mode {
            Mode::Help => self.draw_help(f),
            Mode::Dead(_) => {
                f.rect(40, 90, VW * TILE - 80, 100, 0x100404);
                f.text_centered(VW * TILE / 2, 100, "YOU FALL IN THE ELIMINATOR", RED, true, 2);
                f.text_centered(VW * TILE / 2, 128, &format!("LEVEL {}  {} SP  {} GP", self.level + 1, self.p.sp, self.p.gp), TEXT, true, 1);
                f.text_centered(VW * TILE / 2, 142, &format!("SCORE {}   HIGH {}", self.score(), self.high), GOLD, true, 1);
                f.text_centered(VW * TILE / 2, 164, "ANY KEY FOR A NEW CONTENDER  Q QUITS", DIM, true, 1);
            }
            Mode::Won(_) => {
                f.rect(30, 70, VW * TILE - 60, 130, 0x14100a);
                fancy_text(f, VW * TILE / 2, 80, "YOU WALK OUT", 3, self.time);
                f.text_centered(VW * TILE / 2, 112, "The gate grinds open. Daylight.", TEXT, true, 1);
                f.text_centered(VW * TILE / 2, 124, "By the King's word you rise a noble of Amar.", TEXT, true, 1);
                f.text_centered(VW * TILE / 2, 146, &format!("SCORE {}   HIGH {}", self.score(), self.high), GOLD, true, 1);
                f.text_centered(VW * TILE / 2, 176, "ANY KEY FOR A NEW CONTENDER  Q QUITS", DIM, true, 1);
            }
            _ => {}
        }
    }
}

/// The game with its sound on and the title tune playing.
fn eliminator() -> Eliminator {
    let mut game = Eliminator::new();
    game.audio = Audio::open();
    game.audio.play_loop(1, &game.s.title, 0.7);
    game
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    run(&mut eliminator(), Config { width: W, height: H, fps: 30 });
}

// In a web page: see web/ in this repo.
#[cfg(target_arch = "wasm32")]
funkey::web!(eliminator(), Config { width: W, height: H, fps: 30 });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_o6_rolls_on_and_flags_criticals_and_fumbles() {
        let mut rng = Rng::new(3);
        let (mut sum, mut crits, mut fumbles, n) = (0i64, 0, 0, 200_000);
        for _ in 0..n {
            let r = o6(&mut rng);
            sum += r.total as i64;
            if r.crit { crits += 1; }
            if r.fumble { fumbles += 1; }
            // A cascade ends on the right kind of die.
            let last = *r.dice.last().unwrap();
            if r.dice[0] == 6 { assert!(last <= 3 || r.dice.len() == 1); }
            if r.dice[0] == 1 { assert!(last >= 4 || r.dice.len() == 1); }
        }
        let mean = sum as f64 / n as f64;
        assert!((mean - 3.5).abs() < 0.05, "mean {}", mean);
        // 6 then 6: one in 36, and likewise for 1 then 1.
        assert!((crits as f64 / n as f64 - 1.0 / 36.0).abs() < 0.004);
        assert!((fumbles as f64 / n as f64 - 1.0 / 36.0).abs() < 0.004);
    }

    #[test]
    fn every_contender_follows_the_creation_rules() {
        for c in contenders() {
            let s = &c.sheet;
            let mut ch = s.chars.to_vec();
            ch.sort();
            assert_eq!(ch, vec![0, 1, 1], "{}", s.name);
            let mut at: Vec<i32> = s.attrs.iter().copied().filter(|&v| v > 0).collect();
            at.sort();
            assert_eq!(at, vec![1, 1, 1, 2, 2, 3], "{}", s.name);
            // No attribute under the zero characteristic.
            for (i, &(_, c0)) in ATTRS.iter().enumerate() { if s.chars[c0] == 0 { assert_eq!(s.attrs[i], 0); } }
            let mut sk: Vec<i32> = c.base.iter().map(|b| b.1).collect();
            sk.sort();
            assert_eq!(sk, vec![1, 1, 1, 1, 1, 2, 2, 2, 3], "{}", s.name);
            let total: i32 = s.skills.iter().map(|k| k.1).sum();
            assert_eq!(total, 14 + 2, "{}: the two human bonus points", s.name);
        }
    }

    #[test]
    fn derived_numbers_match_the_amar_formulas() {
        let cs = contenders();
        // Sellsword: SIZE 3.5, Fortitude 1+2+3 = 6, Wield Weapon 1+1+2 = 4.
        assert_eq!(cs[0].sheet.total("Longsword"), 8);
        assert_eq!(cs[0].sheet.bp_max(), 7 + 2);
        assert_eq!(cs[0].sheet.db(), 2);
        // Scout: SIZE 3, Fortitude 3, Wield Weapon 2.
        assert_eq!(cs[1].sheet.bp_max(), 6 + 1);
        assert_eq!(cs[1].sheet.db(), 1);
        // Brute: SIZE 4, Fortitude 6, Wield Weapon 6.
        assert_eq!(cs[2].sheet.bp_max(), 8 + 2);
        assert_eq!(cs[2].sheet.db(), 3);
    }

    #[test]
    fn marks_raise_a_skill_and_cascade_up() {
        let mut s = contenders().remove(0).sheet;
        let mut rng = Rng::new(9);
        let before = s.rank("Dodge");
        // Five times (rank + 1) marks, and a few more for the d6 to pass.
        for _ in 0..(5 * (before + 1) + 10) { s.mark("Dodge", 1, &mut rng); if s.rank("Dodge") > before { break; } }
        assert_eq!(s.rank("Dodge"), before + 1);
        assert_eq!(s.attr_marks[2], 1, "Athletics gets a mark");
    }

    #[test]
    fn untrained_weapons_cost_by_familiarity() {
        let mut g = Eliminator::new();
        g.start(0);
        g.p.weapon = 3;
        assert_eq!(g.weapon_skill(), (8, 0, "Longsword"));
        g.p.weapon = 4; // longsword and buckler: the same kind
        assert_eq!(g.weapon_skill().0, 7);
        g.p.weapon = 6; // bastard sword: the same group
        assert_eq!(g.weapon_skill().0, 5);
        g.p.weapon = 9; // broad axe: untrained, BODY + Melee Combat = 4 beats 8 - 5 = 3
        assert_eq!(g.weapon_skill().0, 4);
    }

    #[test]
    fn every_level_leads_up_and_out() {
        for seed in 1..6 {
            let mut g = Eliminator::new();
            g.seed = seed;
            for n in 0..LEVELS {
                g.build(n);
                let exit = (0..(MW * MH) as usize).find(|&i| matches!(g.map[i], T::Up | T::Gate)).expect("a way up");
                assert!(g.dist[exit] < i32::MAX, "seed {} level {}: the way up is walled off", seed, n + 1);
                assert!(g.foes.len() >= 8, "seed {} level {}: {} foes", seed, n + 1, g.foes.len());
                if n == LEVELS - 1 { assert!(g.foes.iter().any(|f| f.k == DEMON)); }
            }
        }
    }

    /// Stand a contender next to a foe, awake, and fight until one falls.
    fn duel(pick: usize, k: usize, seed: u64) -> bool {
        let mut g = Eliminator::new();
        g.rng = Rng::new(seed);
        g.start(pick);
        g.foes.clear();
        let (x, y) = (g.p.x, g.p.y);
        let (fx, fy) = if g.map[idx(x + 1, y)] != T::Wall { (x + 1, y) } else { (x - 1, y) };
        let mut f = Foe::new(k, fx, fy);
        f.awake = true;
        f.seen = true;
        g.foes.push(f);
        for _ in 0..300 {
            if matches!(g.mode, Mode::Dead(_)) { return false; }
            if g.foes.is_empty() { return true; }
            let (dx, dy) = (g.foes[0].x - g.p.x, g.foes[0].y - g.p.y);
            g.step(dx.signum(), dy.signum());
        }
        false
    }

    #[test]
    fn a_fresh_contender_wins_most_first_level_fights() {
        for (pick, k, floor) in [(0, ARAXI, 0.8), (2, ARAXI, 0.8), (0, RAT, 0.95), (1, RAT, 0.65)] {
            let wins = (0..200).filter(|&s| duel(pick, k, s)).count();
            println!("{} against {}: {} of 200", contenders()[pick].sheet.name, KINDS[k].name, wins);
            assert!(wins as f32 / 200.0 >= floor, "{} against {}: {} of 200", contenders()[pick].sheet.name, KINDS[k].name, wins);
        }
    }

    #[test]
    fn a_troll_is_a_real_threat_to_a_fresh_contender() {
        let wins = (0..200).filter(|&s| duel(0, TROLL, s)).count();
        println!("Sellsword against a troll: {} of 200", wins);
        assert!(wins < 120, "{} of 200", wins);
    }

    #[test]
    fn better_armour_replaces_yours_and_worse_stays_on_the_floor() {
        let mut g = Eliminator::new();
        g.start(0);
        assert_eq!(g.p.armour, 2, "the sellsword starts in heavy leather, AP 2");
        let (x, y) = (g.p.x, g.p.y);
        let here = |g: &Eliminator| g.loot.iter().filter(|l| (l.x, l.y) == (x, y)).count();
        g.loot.retain(|l| (l.x, l.y) != (x, y));
        g.loot.push(Loot { x, y, item: Item::Armour(1) });
        g.take();
        assert_eq!(g.p.armour, 2, "light leather, AP 1, is no better");
        assert_eq!(here(&g), 1);
        g.loot.retain(|l| (l.x, l.y) != (x, y));
        g.loot.push(Loot { x, y, item: Item::Armour(5) });
        g.take();
        assert_eq!(g.p.armour, 5, "chainmail, AP 4, goes on");
        assert_eq!(here(&g), 0, "and nothing is left on the floor");
    }

    #[test]
    fn q_quits_like_escape() {
        let mut g = Eliminator::new();
        let mut input = Input::new();
        input.inject(Key::Char('q'));
        assert_eq!(g.update(&input, 1.0 / 30.0), Flow::Quit);
    }

    #[test]
    fn the_intro_walks_through_and_starts_the_game() {
        for pick in 0..3 {
            let mut g = Eliminator::new();
            g.pick = pick;
            let mut f = Frame::new(W, H);
            let mut press = |g: &mut Eliminator, k: Key| {
                let mut input = Input::new();
                input.inject(k);
                g.update(&input, 1.0 / 30.0);
                // Let every die land before the frame is drawn.
                g.intro.age += 9.0;
                g.draw(&mut f);
            };
            press(&mut g, Key::Char('i'));
            assert!(matches!(g.mode, Mode::Intro));
            for _ in 0..12 { press(&mut g, Key::Down); }
            press(&mut g, Key::Right);
            press(&mut g, Key::Char('r'));
            press(&mut g, Key::Space);
            assert_eq!(g.intro.tally.iter().sum::<u32>(), 101);
            press(&mut g, Key::Right);
            assert_eq!(g.intro.page, 2);
            // Strike until the Araxi falls, then once more to raise it.
            for _ in 0..300 {
                press(&mut g, Key::Space);
                if g.intro.araxi <= 0 { break; }
            }
            assert!(g.intro.araxi <= 0);
            press(&mut g, Key::Space);
            assert_eq!(g.intro.araxi, KINDS[ARAXI].bp);
            press(&mut g, Key::Enter);
            assert!(matches!(g.mode, Mode::Play));
        }
    }

    #[test]
    fn a_wandering_bot_plays_every_level_without_a_crash() {
        for n in 0..LEVELS {
            let mut g = Eliminator::new();
            g.seed = 11;
            g.start(0);
            g.build(n);
            let mut rng = Rng::new(n as u64);
            for _ in 0..1500 {
                if !matches!(g.mode, Mode::Play) { break; }
                let (dx, dy) = DIRS[rng.below(8) as usize];
                g.p.bp = g.p.sheet.bp_max();
                g.step(dx, dy);
                if rng.chance(0.02) { g.fire(); }
                if rng.chance(0.01) { g.p.stance = rng.below(6) as usize; }
            }
            let mut f = Frame::new(W, H);
            g.draw(&mut f);
        }
    }
}
