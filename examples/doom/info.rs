//! The tables the game runs on: what every monster, missile, pickup,
//! decoration and weapon looks like and does, frame by frame, with the
//! timings Doom uses. A frame is (letter, tics, action); a lower-case
//! letter means the frame is drawn at full brightness.

#![allow(dead_code)]

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Act {
    None,
    /// Idle: look for the player.
    Look,
    /// Walk toward the player and decide whether to attack.
    Chase,
    Face,
    /// The monster's ranged attack, from its table entry.
    Attack,
    /// Its melee attack.
    Melee,
    /// Melee when close enough, else the ranged attack.
    Combo,
    Pain,
    Scream,
    XScream,
    /// Stop blocking: the corpse is on the floor.
    Fall,
    /// Splash damage around a barrel or rocket.
    Explode,
    /// Keep shooting while the player stays in sight, else back to the chase.
    Refire,
    SkullAttack,
    PainAttack,
    PainDie,
    /// A revenant rocket turning toward the player.
    Tracer,
    Whoosh,
    FatRaise, Fat1, Fat2, Fat3,
    VileStart, VileTarget, VileAttack,
    Hoof, Metal, BabyMetal,
    BossDeath,
    KeenDie,
    /// The end of an effect: take it out of the world.
    Remove,
    // The weapon in the player's hands.
    Ready, Punch, Saw, FirePistol, FireShotgun, FireShotgun2, FireCGun, FireMissile, FirePlasma, FireBFG,
    BFGSound, GunFlash, ReFire, CheckReload, OpenShotgun2, LoadShotgun2, CloseShotgun2, Light0, Light1, Light2,
    /// The BFG ball's spray of rays, on its explosion.
    BFGSpray,
}
use Act::*;

pub type Fr = (u8, i16, Act);

/// The frame letter as the WAD names it.
pub fn letter(f: u8) -> u8 { f.to_ascii_uppercase() }
/// Full bright frames are written in lower case.
pub fn bright(f: u8) -> bool { f.is_ascii_lowercase() }

#[derive(Clone, Copy, Debug)]
pub enum Attack {
    None,
    /// Bullets, this many at once.
    Hitscan(u8),
    Missile(&'static Proj),
}

pub struct Mon {
    pub sprite: [u8; 4],
    pub health: i32,
    pub radius: f32,
    pub height: f32,
    /// Units moved per Chase call.
    pub speed: f32,
    /// Chance in 256 that a hit interrupts the monster.
    pub pain: u8,
    pub mass: f32,
    pub float: bool,
    pub spawn: &'static [Fr],
    pub see: &'static [Fr],
    pub melee: &'static [Fr],
    pub missile: &'static [Fr],
    pub pain_st: &'static [Fr],
    pub death: &'static [Fr],
    pub xdeath: &'static [Fr],
    pub attack: Attack,
    /// Melee damage as (dice, multiplier): (rand % dice + 1) * multiplier.
    pub melee_dmg: (i32, i32),
    pub s_sight: &'static [&'static str],
    pub s_pain: &'static str,
    pub s_death: &'static [&'static str],
    pub s_active: &'static str,
    pub s_attack: &'static str,
    /// The thing kind dropped on death, 0 for nothing.
    pub drop: u16,
}

#[derive(Debug)]
pub struct Proj {
    pub sprite: [u8; 4],
    pub death_sprite: [u8; 4],
    pub speed: f32,
    pub radius: f32,
    pub height: f32,
    /// Damage as this many rolls of 1..8.
    pub damage: i32,
    /// Splash radius, 0 for none.
    pub splash: f32,
    pub spawn: &'static [Fr],
    pub death: &'static [Fr],
    pub s_fire: &'static str,
    pub s_hit: &'static str,
    pub seeker: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Deco {
    pub sprite: [u8; 4],
    pub frames: &'static [Fr],
    pub solid: bool,
    /// Hangs from the ceiling.
    pub hang: bool,
    pub radius: f32,
    pub height: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AmmoKind { Bullets = 0, Shells = 1, Cells = 2, Rockets = 3 }

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Effect {
    Health(i32, i32),
    Armor(i32, i32),
    ArmorBonus,
    Key(usize),
    Ammo(AmmoKind, i32),
    Backpack,
    Weapon(usize),
    Berserk, Invuln, Invis, Suit, Map, Visor, Mega,
}

#[derive(Clone, Copy, Debug)]
pub struct Pickup {
    pub sprite: [u8; 4],
    pub frames: &'static [Fr],
    pub effect: Effect,
    pub msg: &'static str,
    /// Counts toward the items percentage.
    pub count: bool,
    pub sound: &'static str,
}

pub struct Weapon {
    pub sprite: [u8; 4],
    pub flash_sprite: [u8; 4],
    pub ammo: Option<AmmoKind>,
    pub per_shot: i32,
    pub ready: &'static [Fr],
    pub attack: &'static [Fr],
    pub flash: &'static [Fr],
    /// The number key that picks it.
    pub slot: u8,
}

// ------------------------------------------------------------ monsters

const LOOK2: &[Fr] = &[(b'A', 10, Look), (b'B', 10, Look)];
const SEE4: &[Fr] = &[(b'A', 4, Chase), (b'A', 4, Chase), (b'B', 4, Chase), (b'B', 4, Chase), (b'C', 4, Chase), (b'C', 4, Chase), (b'D', 4, Chase), (b'D', 4, Chase)];
const SEE3: &[Fr] = &[(b'A', 3, Chase), (b'A', 3, Chase), (b'B', 3, Chase), (b'B', 3, Chase), (b'C', 3, Chase), (b'C', 3, Chase), (b'D', 3, Chase), (b'D', 3, Chase)];
const SEE2: &[Fr] = &[(b'A', 2, Chase), (b'A', 2, Chase), (b'B', 2, Chase), (b'B', 2, Chase), (b'C', 2, Chase), (b'C', 2, Chase), (b'D', 2, Chase), (b'D', 2, Chase)];
const SEE2_6: &[Fr] = &[(b'A', 2, Chase), (b'A', 2, Chase), (b'B', 2, Chase), (b'B', 2, Chase), (b'C', 2, Chase), (b'C', 2, Chase), (b'D', 2, Chase), (b'D', 2, Chase), (b'E', 2, Chase), (b'E', 2, Chase), (b'F', 2, Chase), (b'F', 2, Chase)];
const SEE4_6: &[Fr] = &[(b'A', 4, Chase), (b'A', 4, Chase), (b'B', 4, Chase), (b'B', 4, Chase), (b'C', 4, Chase), (b'C', 4, Chase), (b'D', 4, Chase), (b'D', 4, Chase), (b'E', 4, Chase), (b'E', 4, Chase), (b'F', 4, Chase), (b'F', 4, Chase)];
const POSS_XDEATH: &[Fr] = &[(b'M', 5, None), (b'N', 5, XScream), (b'O', 5, Fall), (b'P', 5, None), (b'Q', 5, None), (b'R', 5, None), (b'S', 5, None), (b'T', 5, None), (b'U', -1, None)];
const POSS_DEATH: &[Fr] = &[(b'H', 5, None), (b'I', 5, Scream), (b'J', 5, Fall), (b'K', 5, None), (b'L', -1, None)];
const POS_SIGHT: &[&str] = &["POSIT1", "POSIT2", "POSIT3"];
const POS_DEATH: &[&str] = &["PODTH1", "PODTH2", "PODTH3"];

pub static ZOMBIE: Mon = Mon {
    sprite: *b"POSS", health: 20, radius: 20.0, height: 56.0, speed: 8.0, pain: 200, mass: 100.0, float: false,
    spawn: LOOK2, see: SEE4, melee: &[],
    missile: &[(b'E', 10, Face), (b'F', 8, Attack), (b'E', 8, None)],
    pain_st: &[(b'G', 3, None), (b'G', 3, Pain)],
    death: POSS_DEATH, xdeath: POSS_XDEATH,
    attack: Attack::Hitscan(1), melee_dmg: (0, 0),
    s_sight: POS_SIGHT, s_pain: "POPAIN", s_death: POS_DEATH, s_active: "POSACT", s_attack: "PISTOL", drop: 2007,
};

pub static SHOTGUY: Mon = Mon {
    sprite: *b"SPOS", health: 30, radius: 20.0, height: 56.0, speed: 8.0, pain: 170, mass: 100.0, float: false,
    spawn: LOOK2, see: SEE4, melee: &[],
    missile: &[(b'E', 10, Face), (b'F', 10, Attack), (b'E', 10, None)],
    pain_st: &[(b'G', 3, None), (b'G', 3, Pain)],
    death: POSS_DEATH, xdeath: POSS_XDEATH,
    attack: Attack::Hitscan(3), melee_dmg: (0, 0),
    s_sight: POS_SIGHT, s_pain: "POPAIN", s_death: POS_DEATH, s_active: "POSACT", s_attack: "SHOTGN", drop: 2001,
};

pub static IMP: Mon = Mon {
    sprite: *b"TROO", health: 60, radius: 20.0, height: 56.0, speed: 8.0, pain: 200, mass: 100.0, float: false,
    spawn: LOOK2, see: SEE3,
    melee: &[(b'E', 8, Face), (b'F', 8, Face), (b'G', 6, Combo)],
    missile: &[(b'E', 8, Face), (b'F', 8, Face), (b'G', 6, Combo)],
    pain_st: &[(b'H', 2, None), (b'H', 2, Pain)],
    death: &[(b'I', 8, None), (b'J', 8, Scream), (b'K', 6, None), (b'L', 6, Fall), (b'M', -1, None)],
    xdeath: &[(b'N', 5, None), (b'O', 5, XScream), (b'P', 5, None), (b'Q', 5, Fall), (b'R', 5, None), (b'S', 5, None), (b'T', 5, None), (b'U', -1, None)],
    attack: Attack::Missile(&IMPBALL), melee_dmg: (8, 3),
    s_sight: &["BGSIT1", "BGSIT2"], s_pain: "POPAIN", s_death: &["BGDTH1", "BGDTH2"], s_active: "BGACT", s_attack: "CLAW", drop: 0,
};

pub static DEMON: Mon = Mon {
    sprite: *b"SARG", health: 150, radius: 30.0, height: 56.0, speed: 10.0, pain: 180, mass: 400.0, float: false,
    spawn: LOOK2, see: SEE2,
    melee: &[(b'E', 8, Face), (b'F', 8, Face), (b'G', 8, Melee)], missile: &[],
    pain_st: &[(b'H', 2, None), (b'H', 2, Pain)],
    death: &[(b'I', 8, None), (b'J', 8, Scream), (b'K', 4, None), (b'L', 4, Fall), (b'M', 4, None), (b'N', -1, None)], xdeath: &[],
    attack: Attack::None, melee_dmg: (10, 4),
    s_sight: &["SGTSIT"], s_pain: "DMPAIN", s_death: &["SGTDTH"], s_active: "DMACT", s_attack: "SGTATK", drop: 0,
};

pub static CACO: Mon = Mon {
    sprite: *b"HEAD", health: 400, radius: 31.0, height: 56.0, speed: 8.0, pain: 128, mass: 400.0, float: true,
    spawn: &[(b'A', 10, Look)], see: &[(b'A', 3, Chase)],
    melee: &[(b'B', 5, Face), (b'C', 5, Face), (b'D', 5, Combo)],
    missile: &[(b'B', 5, Face), (b'C', 5, Face), (b'D', 5, Combo)],
    pain_st: &[(b'E', 3, None), (b'E', 3, Pain), (b'F', 6, None)],
    death: &[(b'G', 8, None), (b'H', 8, Scream), (b'I', 8, None), (b'J', 8, None), (b'K', 8, Fall), (b'L', -1, None)], xdeath: &[],
    attack: Attack::Missile(&CACOBALL), melee_dmg: (6, 10),
    s_sight: &["CACSIT"], s_pain: "DMPAIN", s_death: &["CACDTH"], s_active: "DMACT", s_attack: "CLAW", drop: 0,
};

const BOSS_DEATH: &[Fr] = &[(b'I', 8, None), (b'J', 8, Scream), (b'K', 8, None), (b'L', 8, Fall), (b'M', 8, None), (b'N', 8, None), (b'O', -1, BossDeath)];
pub static BARON: Mon = Mon {
    sprite: *b"BOSS", health: 1000, radius: 24.0, height: 64.0, speed: 8.0, pain: 50, mass: 1000.0, float: false,
    spawn: LOOK2, see: SEE3,
    melee: &[(b'E', 8, Face), (b'F', 8, Face), (b'G', 8, Combo)],
    missile: &[(b'E', 8, Face), (b'F', 8, Face), (b'G', 8, Combo)],
    pain_st: &[(b'H', 2, None), (b'H', 2, Pain)],
    death: BOSS_DEATH, xdeath: &[],
    attack: Attack::Missile(&BARONBALL), melee_dmg: (8, 10),
    s_sight: &["BRSSIT"], s_pain: "DMPAIN", s_death: &["BRSDTH"], s_active: "DMACT", s_attack: "CLAW", drop: 0,
};

pub static KNIGHT: Mon = Mon {
    sprite: *b"BOS2", health: 500, radius: 24.0, height: 64.0, speed: 8.0, pain: 50, mass: 1000.0, float: false,
    spawn: LOOK2, see: SEE3,
    melee: &[(b'E', 8, Face), (b'F', 8, Face), (b'G', 8, Combo)],
    missile: &[(b'E', 8, Face), (b'F', 8, Face), (b'G', 8, Combo)],
    pain_st: &[(b'H', 2, None), (b'H', 2, Pain)],
    death: &[(b'I', 8, None), (b'J', 8, Scream), (b'K', 8, None), (b'L', 8, Fall), (b'M', 8, None), (b'N', 8, None), (b'O', -1, None)], xdeath: &[],
    attack: Attack::Missile(&BARONBALL), melee_dmg: (8, 10),
    s_sight: &["KNTSIT"], s_pain: "DMPAIN", s_death: &["KNTDTH"], s_active: "DMACT", s_attack: "CLAW", drop: 0,
};

pub static SKULL: Mon = Mon {
    sprite: *b"SKUL", health: 100, radius: 16.0, height: 56.0, speed: 8.0, pain: 255, mass: 50.0, float: true,
    spawn: &[(b'a', 10, Look), (b'b', 10, Look)], see: &[(b'a', 6, Chase), (b'b', 6, Chase)],
    melee: &[],
    missile: &[(b'c', 10, Face), (b'd', 4, SkullAttack), (b'c', 4, None), (b'd', 4, None)],
    pain_st: &[(b'e', 3, None), (b'e', 3, Pain)],
    death: &[(b'f', 6, None), (b'g', 6, Scream), (b'h', 6, None), (b'i', 6, Fall), (b'j', 6, None), (b'k', 6, None), (b'k', 0, Remove)], xdeath: &[],
    attack: Attack::None, melee_dmg: (8, 3),
    s_sight: &[], s_pain: "DMPAIN", s_death: &["FIRXPL"], s_active: "DMACT", s_attack: "SKLATK", drop: 0,
};

pub static CHAINGUY: Mon = Mon {
    sprite: *b"CPOS", health: 70, radius: 20.0, height: 56.0, speed: 8.0, pain: 170, mass: 100.0, float: false,
    spawn: LOOK2, see: SEE4, melee: &[],
    missile: &[(b'E', 10, Face), (b'F', 4, Attack), (b'E', 4, Attack), (b'F', 1, Refire)],
    pain_st: &[(b'G', 3, None), (b'G', 3, Pain)],
    death: &[(b'H', 5, None), (b'I', 5, Scream), (b'J', 5, Fall), (b'K', 5, None), (b'L', 5, None), (b'M', 5, None), (b'N', -1, None)],
    xdeath: &[(b'O', 5, None), (b'P', 5, XScream), (b'Q', 5, Fall), (b'R', 5, None), (b'S', 5, None), (b'T', -1, None)],
    attack: Attack::Hitscan(1), melee_dmg: (0, 0),
    s_sight: &["POSIT2"], s_pain: "POPAIN", s_death: &["PODTH2"], s_active: "POSACT", s_attack: "SHOTGN", drop: 2002,
};

pub static REVENANT: Mon = Mon {
    sprite: *b"SKEL", health: 300, radius: 20.0, height: 56.0, speed: 10.0, pain: 100, mass: 500.0, float: false,
    spawn: LOOK2, see: SEE2_6,
    melee: &[(b'G', 0, Face), (b'G', 6, Whoosh), (b'H', 6, Face), (b'I', 6, Melee)],
    missile: &[(b'j', 0, Face), (b'j', 10, Face), (b'K', 10, Attack), (b'K', 10, Face)],
    pain_st: &[(b'L', 5, None), (b'L', 5, Pain)],
    death: &[(b'L', 7, None), (b'M', 7, None), (b'N', 7, Scream), (b'O', 7, Fall), (b'P', 7, None), (b'Q', -1, None)], xdeath: &[],
    attack: Attack::Missile(&TRACER), melee_dmg: (10, 6),
    s_sight: &["SKESIT"], s_pain: "POPAIN", s_death: &["SKEDTH"], s_active: "SKEACT", s_attack: "SKEPCH", drop: 0,
};

pub static MANCUBUS: Mon = Mon {
    sprite: *b"FATT", health: 600, radius: 48.0, height: 64.0, speed: 8.0, pain: 80, mass: 1000.0, float: false,
    spawn: &[(b'A', 15, Look), (b'B', 15, Look)], see: SEE4_6, melee: &[],
    missile: &[(b'G', 20, FatRaise), (b'h', 10, Fat1), (b'I', 5, Face), (b'G', 5, None), (b'h', 10, Fat2), (b'I', 5, Face), (b'G', 5, None), (b'h', 10, Fat3), (b'I', 5, Face), (b'G', 5, Face)],
    pain_st: &[(b'J', 3, None), (b'J', 3, Pain)],
    death: &[(b'K', 6, None), (b'L', 6, Scream), (b'M', 6, Fall), (b'N', 6, None), (b'O', 6, None), (b'P', 6, None), (b'Q', 6, None), (b'R', 6, None), (b'S', 6, None), (b'T', -1, BossDeath)], xdeath: &[],
    attack: Attack::Missile(&FATSHOT), melee_dmg: (0, 0),
    s_sight: &["MANSIT"], s_pain: "MNPAIN", s_death: &["MANDTH"], s_active: "POSACT", s_attack: "MANATK", drop: 0,
};

pub static ARACHNOTRON: Mon = Mon {
    sprite: *b"BSPI", health: 500, radius: 64.0, height: 64.0, speed: 12.0, pain: 128, mass: 600.0, float: false,
    spawn: LOOK2,
    see: &[(b'A', 20, BabyMetal), (b'A', 3, Chase), (b'B', 3, Chase), (b'B', 3, Chase), (b'C', 3, Chase), (b'C', 3, Chase), (b'D', 3, Chase), (b'D', 3, Chase), (b'E', 3, Chase), (b'E', 3, Chase), (b'F', 3, Chase), (b'F', 3, Chase)],
    melee: &[],
    missile: &[(b'A', 20, Face), (b'g', 4, Attack), (b'h', 4, None), (b'h', 1, Refire)],
    pain_st: &[(b'I', 3, None), (b'I', 3, Pain)],
    death: &[(b'J', 20, Scream), (b'K', 7, Fall), (b'L', 7, None), (b'M', 7, None), (b'N', 7, None), (b'O', 7, None), (b'P', -1, BossDeath)], xdeath: &[],
    attack: Attack::Missile(&ARACHPLASMA), melee_dmg: (0, 0),
    s_sight: &["BSPSIT"], s_pain: "DMPAIN", s_death: &["BSPDTH"], s_active: "BSPACT", s_attack: "PLASMA", drop: 0,
};

pub static PAINELEMENTAL: Mon = Mon {
    sprite: *b"PAIN", health: 400, radius: 31.0, height: 56.0, speed: 8.0, pain: 128, mass: 400.0, float: true,
    spawn: &[(b'A', 10, Look)], see: &[(b'A', 3, Chase), (b'B', 3, Chase), (b'C', 3, Chase)], melee: &[],
    missile: &[(b'D', 5, Face), (b'e', 5, Face), (b'f', 5, Face), (b'f', 0, PainAttack)],
    pain_st: &[(b'G', 6, None), (b'G', 6, Pain)],
    death: &[(b'h', 8, None), (b'i', 8, Scream), (b'j', 8, None), (b'k', 8, PainDie), (b'l', 8, None), (b'm', 8, None), (b'm', 0, Remove)], xdeath: &[],
    attack: Attack::None, melee_dmg: (0, 0),
    s_sight: &["PESIT"], s_pain: "PEPAIN", s_death: &["PEDTH"], s_active: "DMACT", s_attack: "SKLATK", drop: 0,
};

pub static ARCHVILE: Mon = Mon {
    sprite: *b"VILE", health: 700, radius: 20.0, height: 56.0, speed: 15.0, pain: 10, mass: 500.0, float: false,
    spawn: LOOK2, see: SEE2_6, melee: &[],
    missile: &[(b'g', 0, VileStart), (b'g', 10, Face), (b'h', 8, VileTarget), (b'i', 8, Face), (b'j', 8, Face), (b'k', 8, Face), (b'l', 8, Face), (b'm', 8, Face), (b'n', 8, Face), (b'o', 8, Face), (b'p', 8, VileAttack), (b'p', 20, None)],
    pain_st: &[(b'Q', 5, None), (b'Q', 5, Pain)],
    death: &[(b'Q', 7, None), (b'R', 7, Scream), (b'S', 7, Fall), (b'T', 7, None), (b'U', 7, None), (b'V', 7, None), (b'W', 7, None), (b'X', 5, None), (b'Y', 5, None), (b'Z', -1, None)], xdeath: &[],
    attack: Attack::None, melee_dmg: (0, 0),
    s_sight: &["VILSIT"], s_pain: "VIPAIN", s_death: &["VILDTH"], s_active: "VILACT", s_attack: "VILATK", drop: 0,
};

pub static CYBERDEMON: Mon = Mon {
    sprite: *b"CYBR", health: 4000, radius: 40.0, height: 110.0, speed: 16.0, pain: 20, mass: 1000.0, float: false,
    spawn: LOOK2, see: &[(b'A', 3, Hoof), (b'B', 3, Chase), (b'C', 3, Chase), (b'D', 3, Metal)], melee: &[],
    missile: &[(b'E', 6, Face), (b'F', 12, Attack), (b'E', 12, Face), (b'F', 12, Attack), (b'E', 12, Face), (b'F', 12, Attack)],
    pain_st: &[(b'G', 10, Pain)],
    death: &[(b'H', 10, None), (b'I', 10, Scream), (b'J', 10, None), (b'K', 10, None), (b'L', 10, None), (b'M', 10, None), (b'N', 10, None), (b'O', 10, None), (b'P', 30, None), (b'P', -1, BossDeath)], xdeath: &[],
    attack: Attack::Missile(&ROCKET), melee_dmg: (0, 0),
    s_sight: &["CYBSIT"], s_pain: "DMPAIN", s_death: &["CYBDTH"], s_active: "DMACT", s_attack: "RLAUNC", drop: 0,
};

pub static SPIDER: Mon = Mon {
    sprite: *b"SPID", health: 3000, radius: 128.0, height: 100.0, speed: 12.0, pain: 40, mass: 1000.0, float: false,
    spawn: LOOK2,
    see: &[(b'A', 3, Metal), (b'A', 3, Chase), (b'B', 3, Chase), (b'B', 3, Chase), (b'C', 3, Chase), (b'C', 3, Chase), (b'D', 3, Chase), (b'D', 3, Chase), (b'E', 3, Chase), (b'E', 3, Chase), (b'F', 3, Chase), (b'F', 3, Chase)],
    melee: &[],
    missile: &[(b'A', 20, Face), (b'g', 4, Attack), (b'h', 4, Attack), (b'h', 1, Refire)],
    pain_st: &[(b'I', 3, None), (b'I', 3, Pain)],
    death: &[(b'J', 20, Scream), (b'K', 10, Fall), (b'L', 10, None), (b'M', 10, None), (b'N', 10, None), (b'O', 10, None), (b'P', 10, None), (b'Q', 10, None), (b'R', 10, None), (b'S', 30, None), (b'S', -1, BossDeath)], xdeath: &[],
    attack: Attack::Hitscan(3), melee_dmg: (0, 0),
    s_sight: &["SPISIT"], s_pain: "DMPAIN", s_death: &["SPIDTH"], s_active: "DMACT", s_attack: "SHOTGN", drop: 0,
};

pub static WOLFSS: Mon = Mon {
    sprite: *b"SSWV", health: 50, radius: 20.0, height: 56.0, speed: 8.0, pain: 170, mass: 100.0, float: false,
    spawn: LOOK2, see: SEE3, melee: &[],
    missile: &[(b'E', 10, Face), (b'F', 10, Face), (b'G', 4, Attack), (b'F', 6, Face), (b'G', 4, Attack), (b'F', 1, Refire)],
    pain_st: &[(b'H', 3, None), (b'H', 3, Pain)],
    death: &[(b'I', 5, None), (b'J', 5, Scream), (b'K', 5, Fall), (b'L', 5, None), (b'M', -1, None)],
    xdeath: &[(b'N', 5, None), (b'O', 5, XScream), (b'P', 5, Fall), (b'Q', 5, None), (b'R', 5, None), (b'S', 5, None), (b'T', 5, None), (b'U', 5, None), (b'V', -1, None)],
    attack: Attack::Hitscan(1), melee_dmg: (0, 0),
    s_sight: &["SSSIT"], s_pain: "POPAIN", s_death: &["SSDTH"], s_active: "POSACT", s_attack: "SHOTGN", drop: 2007,
};

pub static KEEN: Mon = Mon {
    sprite: *b"KEEN", health: 100, radius: 16.0, height: 72.0, speed: 0.0, pain: 255, mass: 10000000.0, float: true,
    spawn: &[(b'A', -1, None)], see: &[], melee: &[], missile: &[],
    pain_st: &[(b'M', 4, None), (b'M', 8, Pain)],
    death: &[(b'A', 6, None), (b'B', 6, None), (b'C', 6, None), (b'D', 6, Scream), (b'E', 6, None), (b'F', 6, None), (b'G', 6, None), (b'H', 6, None), (b'I', 6, None), (b'J', 6, None), (b'K', 6, KeenDie), (b'L', -1, None)], xdeath: &[],
    attack: Attack::None, melee_dmg: (0, 0),
    s_sight: &[], s_pain: "KEENPN", s_death: &["KEENDT"], s_active: "", s_attack: "", drop: 0,
};

pub static BARREL: Mon = Mon {
    sprite: *b"BAR1", health: 20, radius: 10.0, height: 42.0, speed: 0.0, pain: 0, mass: 100.0, float: false,
    spawn: &[(b'A', 6, None), (b'B', 6, None)], see: &[], melee: &[], missile: &[], pain_st: &[],
    death: &[(b'a', 5, None), (b'b', 5, Scream), (b'c', 5, None), (b'd', 10, Explode), (b'e', 10, None), (b'e', 0, Remove)], xdeath: &[],
    attack: Attack::None, melee_dmg: (0, 0),
    s_sight: &[], s_pain: "", s_death: &["BAREXP"], s_active: "", s_attack: "", drop: 0,
};

/// The monster for a thing kind. Spectres are demons drawn fuzzy.
pub fn monster(kind: u16) -> Option<&'static Mon> {
    Some(match kind {
        3004 => &ZOMBIE, 9 => &SHOTGUY, 3001 => &IMP, 3002 | 58 => &DEMON, 3005 => &CACO, 3003 => &BARON,
        3006 => &SKULL, 65 => &CHAINGUY, 66 => &REVENANT, 67 => &MANCUBUS, 68 => &ARACHNOTRON, 69 => &KNIGHT,
        71 => &PAINELEMENTAL, 64 => &ARCHVILE, 16 => &CYBERDEMON, 7 => &SPIDER, 84 => &WOLFSS, 72 => &KEEN,
        2035 => &BARREL,
        _ => return Option::None,
    })
}

// ------------------------------------------------------------ missiles

const BALL_SPAWN: &[Fr] = &[(b'a', 4, None), (b'b', 4, None)];
const BALL_DEATH: &[Fr] = &[(b'c', 6, None), (b'd', 6, None), (b'e', 6, None), (b'e', 0, Remove)];

pub static IMPBALL: Proj = Proj { sprite: *b"BAL1", death_sprite: *b"BAL1", speed: 10.0, radius: 6.0, height: 8.0, damage: 3, splash: 0.0,
    spawn: BALL_SPAWN, death: BALL_DEATH, s_fire: "FIRSHT", s_hit: "FIRXPL", seeker: false };
pub static CACOBALL: Proj = Proj { sprite: *b"BAL2", death_sprite: *b"BAL2", speed: 10.0, radius: 6.0, height: 8.0, damage: 5, splash: 0.0,
    spawn: BALL_SPAWN, death: BALL_DEATH, s_fire: "FIRSHT", s_hit: "FIRXPL", seeker: false };
pub static BARONBALL: Proj = Proj { sprite: *b"BAL7", death_sprite: *b"BAL7", speed: 15.0, radius: 6.0, height: 16.0, damage: 8, splash: 0.0,
    spawn: BALL_SPAWN, death: BALL_DEATH, s_fire: "FIRSHT", s_hit: "FIRXPL", seeker: false };
pub static ROCKET: Proj = Proj { sprite: *b"MISL", death_sprite: *b"MISL", speed: 20.0, radius: 11.0, height: 8.0, damage: 20, splash: 128.0,
    spawn: &[(b'a', 1, None)], death: &[(b'b', 8, Explode), (b'c', 6, None), (b'd', 4, None), (b'd', 0, Remove)], s_fire: "RLAUNC", s_hit: "BAREXP", seeker: false };
pub static PLASMA: Proj = Proj { sprite: *b"PLSS", death_sprite: *b"PLSE", speed: 25.0, radius: 13.0, height: 8.0, damage: 5, splash: 0.0,
    spawn: &[(b'a', 6, None), (b'b', 6, None)], death: &[(b'a', 4, None), (b'b', 4, None), (b'c', 4, None), (b'd', 4, None), (b'e', 4, None), (b'e', 0, Remove)], s_fire: "PLASMA", s_hit: "FIRXPL", seeker: false };
pub static BFGBALL: Proj = Proj { sprite: *b"BFS1", death_sprite: *b"BFE1", speed: 25.0, radius: 13.0, height: 8.0, damage: 100, splash: 0.0,
    spawn: BALL_SPAWN, death: &[(b'a', 8, None), (b'b', 8, None), (b'c', 8, BFGSpray), (b'd', 8, None), (b'e', 8, None), (b'f', 8, None), (b'f', 0, Remove)], s_fire: "BFG", s_hit: "RXPLOD", seeker: false };
pub static TRACER: Proj = Proj { sprite: *b"FATB", death_sprite: *b"FBXP", speed: 10.0, radius: 11.0, height: 8.0, damage: 10, splash: 0.0,
    spawn: &[(b'a', 2, Tracer), (b'b', 2, Tracer)], death: &[(b'a', 8, None), (b'b', 6, None), (b'c', 4, None), (b'c', 0, Remove)], s_fire: "SKEATK", s_hit: "BAREXP", seeker: true };
pub static FATSHOT: Proj = Proj { sprite: *b"MANF", death_sprite: *b"MISL", speed: 20.0, radius: 6.0, height: 8.0, damage: 8, splash: 0.0,
    spawn: BALL_SPAWN, death: &[(b'b', 8, None), (b'c', 6, None), (b'd', 4, None), (b'd', 0, Remove)], s_fire: "FIRSHT", s_hit: "FIRXPL", seeker: false };
pub static ARACHPLASMA: Proj = Proj { sprite: *b"APLS", death_sprite: *b"APBX", speed: 25.0, radius: 13.0, height: 8.0, damage: 5, splash: 0.0,
    spawn: &[(b'a', 5, None), (b'b', 5, None)], death: &[(b'a', 5, None), (b'b', 5, None), (b'c', 5, None), (b'd', 5, None), (b'e', 5, None), (b'e', 0, Remove)], s_fire: "PLASMA", s_hit: "FIRXPL", seeker: false };

// ------------------------------------------------------------- effects

pub const PUFF: &[Fr] = &[(b'a', 4, None), (b'b', 4, None), (b'C', 4, None), (b'D', 4, None), (b'D', 0, Remove)];
pub const BLOOD_BIG: &[Fr] = &[(b'C', 8, None), (b'B', 8, None), (b'A', 8, None), (b'A', 0, Remove)];
pub const BLOOD_MID: &[Fr] = &[(b'B', 8, None), (b'A', 8, None), (b'A', 0, Remove)];
pub const BLOOD_SMALL: &[Fr] = &[(b'A', 8, None), (b'A', 0, Remove)];
pub const TELEFOG: &[Fr] = &[(b'a', 6, None), (b'b', 6, None), (b'a', 6, None), (b'b', 6, None), (b'c', 6, None), (b'd', 6, None), (b'e', 6, None), (b'f', 6, None), (b'g', 6, None), (b'h', 6, None), (b'i', 6, None), (b'j', 6, None), (b'j', 0, Remove)];
pub const BFG_SPRAY: &[Fr] = &[(b'a', 8, None), (b'b', 8, None), (b'c', 8, None), (b'd', 8, None), (b'd', 0, Remove)];
pub const VILE_FIRE: &[Fr] = &[(b'a', 2, None), (b'b', 2, None), (b'a', 2, None), (b'b', 2, None), (b'c', 2, None), (b'b', 2, None), (b'c', 2, None), (b'b', 2, None), (b'c', 2, None), (b'd', 2, None), (b'c', 2, None), (b'd', 2, None), (b'c', 2, None), (b'd', 2, None), (b'e', 2, None), (b'd', 2, None), (b'e', 2, None), (b'd', 2, None), (b'e', 2, None), (b'f', 2, None), (b'e', 2, None), (b'f', 2, None), (b'e', 2, None), (b'f', 2, None), (b'g', 2, None), (b'h', 2, None), (b'h', 0, Remove)];
pub const GIBS: &[Fr] = &[(b'A', -1, None)];

// ------------------------------------------------------------- pickups

const ONE: &[Fr] = &[(b'A', -1, None)];
const ONE_BRIGHT: &[Fr] = &[(b'a', -1, None)];
const BONUS: &[Fr] = &[(b'A', 6, None), (b'B', 6, None), (b'C', 6, None), (b'D', 6, None), (b'E', 6, None), (b'F', 6, None)];
const SPHERE: &[Fr] = &[(b'a', 6, None), (b'b', 6, None), (b'c', 6, None), (b'd', 6, None)];
const SOUL: &[Fr] = &[(b'a', 6, None), (b'b', 6, None), (b'c', 6, None), (b'd', 6, None), (b'e', 6, None), (b'f', 6, None)];
const KEY: &[Fr] = &[(b'A', 10, None), (b'b', 10, None)];

pub const KEY_BLUE: usize = 0;
pub const KEY_YELLOW: usize = 1;
pub const KEY_RED: usize = 2;
pub const KEY_BLUE_SKULL: usize = 3;
pub const KEY_YELLOW_SKULL: usize = 4;
pub const KEY_RED_SKULL: usize = 5;

pub const W_FIST: usize = 0;
pub const W_PISTOL: usize = 1;
pub const W_SHOTGUN: usize = 2;
pub const W_CHAINGUN: usize = 3;
pub const W_ROCKET: usize = 4;
pub const W_PLASMA: usize = 5;
pub const W_BFG: usize = 6;
pub const W_CHAINSAW: usize = 7;
pub const W_SSG: usize = 8;

pub fn pickup(kind: u16) -> Option<Pickup> {
    use AmmoKind::*;
    let p = |sprite: &[u8; 4], frames: &'static [Fr], effect: Effect, msg: &'static str, count: bool, sound: &'static str|
        Pickup { sprite: *sprite, frames, effect, msg, count, sound };
    Some(match kind {
        2011 => p(b"STIM", ONE, Effect::Health(10, 100), "Picked up a stimpack.", false, "ITEMUP"),
        2012 => p(b"MEDI", ONE, Effect::Health(25, 100), "Picked up a medikit.", false, "ITEMUP"),
        2014 => p(b"BON1", BONUS, Effect::Health(1, 200), "Picked up a health bonus.", true, "ITEMUP"),
        2015 => p(b"BON2", BONUS, Effect::ArmorBonus, "Picked up an armor bonus.", true, "ITEMUP"),
        2013 => p(b"SOUL", SOUL, Effect::Health(100, 200), "Supercharge!", true, "GETPOW"),
        83 => p(b"MEGA", SPHERE, Effect::Mega, "MegaSphere!", true, "GETPOW"),
        2018 => p(b"ARM1", &[(b'A', 6, None), (b'b', 7, None)], Effect::Armor(100, 1), "Picked up the armor.", false, "ITEMUP"),
        2019 => p(b"ARM2", &[(b'A', 6, None), (b'b', 6, None)], Effect::Armor(200, 2), "Picked up the MegaArmor!", false, "ITEMUP"),
        2022 => p(b"PINV", SPHERE, Effect::Invuln, "Invulnerability!", true, "GETPOW"),
        2023 => p(b"PSTR", ONE_BRIGHT, Effect::Berserk, "Berserk!", true, "GETPOW"),
        2024 => p(b"PINS", SPHERE, Effect::Invis, "Partial Invisibility", true, "GETPOW"),
        2025 => p(b"SUIT", ONE_BRIGHT, Effect::Suit, "Radiation Shielding Suit", true, "GETPOW"),
        2026 => p(b"PMAP", SPHERE, Effect::Map, "Computer Area Map", true, "GETPOW"),
        2045 => p(b"PVIS", &[(b'a', 6, None), (b'b', 6, None)], Effect::Visor, "Light Amplification Visor", true, "GETPOW"),
        5 => p(b"BKEY", KEY, Effect::Key(KEY_BLUE), "Picked up a blue keycard.", false, "ITEMUP"),
        6 => p(b"YKEY", KEY, Effect::Key(KEY_YELLOW), "Picked up a yellow keycard.", false, "ITEMUP"),
        13 => p(b"RKEY", KEY, Effect::Key(KEY_RED), "Picked up a red keycard.", false, "ITEMUP"),
        40 => p(b"BSKU", KEY, Effect::Key(KEY_BLUE_SKULL), "Picked up a blue skull key.", false, "ITEMUP"),
        39 => p(b"YSKU", KEY, Effect::Key(KEY_YELLOW_SKULL), "Picked up a yellow skull key.", false, "ITEMUP"),
        38 => p(b"RSKU", KEY, Effect::Key(KEY_RED_SKULL), "Picked up a red skull key.", false, "ITEMUP"),
        2007 => p(b"CLIP", ONE, Effect::Ammo(Bullets, 10), "Picked up a clip.", false, "ITEMUP"),
        2048 => p(b"AMMO", ONE, Effect::Ammo(Bullets, 50), "Picked up a box of bullets.", false, "ITEMUP"),
        2008 => p(b"SHEL", ONE, Effect::Ammo(Shells, 4), "Picked up 4 shotgun shells.", false, "ITEMUP"),
        2049 => p(b"SBOX", ONE, Effect::Ammo(Shells, 20), "Picked up a box of shotgun shells.", false, "ITEMUP"),
        2010 => p(b"ROCK", ONE, Effect::Ammo(Rockets, 1), "Picked up a rocket.", false, "ITEMUP"),
        2046 => p(b"BROK", ONE, Effect::Ammo(Rockets, 5), "Picked up a box of rockets.", false, "ITEMUP"),
        2047 => p(b"CELL", ONE, Effect::Ammo(Cells, 20), "Picked up an energy cell.", false, "ITEMUP"),
        17 => p(b"CELP", ONE, Effect::Ammo(Cells, 100), "Picked up an energy cell pack.", false, "ITEMUP"),
        8 => p(b"BPAK", ONE, Effect::Backpack, "Picked up a backpack full of ammo!", false, "ITEMUP"),
        2001 => p(b"SHOT", ONE, Effect::Weapon(W_SHOTGUN), "You got the shotgun!", false, "WPNUP"),
        82 => p(b"SGN2", ONE, Effect::Weapon(W_SSG), "You got the super shotgun!", false, "WPNUP"),
        2002 => p(b"MGUN", ONE, Effect::Weapon(W_CHAINGUN), "You got the chaingun!", false, "WPNUP"),
        2003 => p(b"LAUN", ONE, Effect::Weapon(W_ROCKET), "You got the rocket launcher!", false, "WPNUP"),
        2004 => p(b"PLAS", ONE, Effect::Weapon(W_PLASMA), "You got the plasma gun!", false, "WPNUP"),
        2005 => p(b"CSAW", ONE, Effect::Weapon(W_CHAINSAW), "A chainsaw!  Find some meat!", false, "WPNUP"),
        2006 => p(b"BFUG", ONE, Effect::Weapon(W_BFG), "You got the BFG9000!  Oh, yes.", false, "WPNUP"),
        _ => return Option::None,
    })
}

// --------------------------------------------------------- decorations

const TORCH: &[Fr] = &[(b'a', 4, None), (b'b', 4, None), (b'c', 4, None), (b'd', 4, None)];

pub fn decoration(kind: u16) -> Option<Deco> {
    let d = |sprite: &[u8; 4], frames: &'static [Fr], solid: bool, hang: bool, radius: f32, height: f32|
        Deco { sprite: *sprite, frames, solid, hang, radius, height };
    Some(match kind {
        2028 => d(b"COLU", ONE_BRIGHT, true, false, 16.0, 48.0),
        30 => d(b"COL1", ONE, true, false, 16.0, 48.0), 31 => d(b"COL2", ONE, true, false, 16.0, 40.0),
        32 => d(b"COL3", ONE, true, false, 16.0, 48.0), 33 => d(b"COL4", ONE, true, false, 16.0, 40.0),
        36 => d(b"COL5", &[(b'A', 14, None), (b'B', 14, None)], true, false, 16.0, 40.0), 37 => d(b"COL6", ONE, true, false, 16.0, 40.0),
        41 => d(b"CEYE", &[(b'a', 6, None), (b'b', 6, None), (b'c', 6, None), (b'b', 6, None)], true, false, 16.0, 54.0),
        42 => d(b"FSKU", &[(b'a', 6, None), (b'b', 6, None), (b'c', 6, None)], true, true, 16.0, 26.0),
        43 => d(b"TRE1", ONE, true, false, 16.0, 56.0), 54 => d(b"TRE2", ONE, true, false, 32.0, 108.0),
        44 => d(b"TBLU", TORCH, true, false, 16.0, 68.0), 45 => d(b"TGRN", TORCH, true, false, 16.0, 68.0), 46 => d(b"TRED", TORCH, true, false, 16.0, 68.0),
        55 => d(b"SMBT", TORCH, true, false, 16.0, 37.0), 56 => d(b"SMGT", TORCH, true, false, 16.0, 37.0), 57 => d(b"SMRT", TORCH, true, false, 16.0, 37.0),
        47 => d(b"SMIT", ONE, true, false, 16.0, 40.0), 48 => d(b"ELEC", ONE, true, false, 16.0, 128.0),
        34 => d(b"CAND", ONE_BRIGHT, false, false, 20.0, 16.0), 35 => d(b"CBRA", ONE_BRIGHT, true, false, 16.0, 60.0),
        70 => d(b"FCAN", &[(b'a', 4, None), (b'b', 4, None), (b'c', 4, None)], true, false, 10.0, 16.0),
        85 => d(b"TLMP", TORCH, true, false, 16.0, 80.0), 86 => d(b"TLP2", TORCH, true, false, 16.0, 60.0),
        25 => d(b"POL1", ONE, true, false, 16.0, 16.0), 26 => d(b"POL6", &[(b'A', 6, None), (b'B', 8, None)], true, false, 16.0, 16.0),
        27 => d(b"POL4", ONE, true, false, 16.0, 16.0), 28 => d(b"POL2", ONE, true, false, 16.0, 16.0),
        29 => d(b"POL3", &[(b'a', 6, None), (b'b', 6, None)], true, false, 16.0, 16.0), 24 => d(b"POL5", ONE, false, false, 16.0, 16.0),
        10 | 12 => d(b"PLAY", &[(b'W', -1, None)], false, false, 16.0, 16.0), 15 => d(b"PLAY", &[(b'N', -1, None)], false, false, 16.0, 16.0),
        18 => d(b"POSS", &[(b'L', -1, None)], false, false, 20.0, 16.0), 19 => d(b"SPOS", &[(b'L', -1, None)], false, false, 20.0, 16.0),
        20 => d(b"TROO", &[(b'M', -1, None)], false, false, 20.0, 16.0), 21 => d(b"SARG", &[(b'N', -1, None)], false, false, 30.0, 16.0),
        22 => d(b"HEAD", &[(b'L', -1, None)], false, false, 31.0, 16.0), 23 => d(b"SKUL", &[(b'K', -1, None)], false, false, 16.0, 16.0),
        49 => d(b"GOR1", &[(b'A', 10, None), (b'B', 10, None), (b'C', 10, None), (b'B', 10, None)], true, true, 16.0, 68.0),
        50 => d(b"GOR2", ONE, true, true, 16.0, 84.0), 51 => d(b"GOR3", ONE, true, true, 16.0, 84.0),
        52 => d(b"GOR4", ONE, true, true, 16.0, 68.0), 53 => d(b"GOR5", ONE, true, true, 16.0, 52.0),
        59 => d(b"GOR2", ONE, false, true, 20.0, 84.0), 60 => d(b"GOR4", ONE, false, true, 20.0, 68.0),
        61 => d(b"GOR3", ONE, false, true, 20.0, 52.0), 62 => d(b"GOR5", ONE, false, true, 20.0, 52.0),
        63 => d(b"GOR1", &[(b'A', 10, None), (b'B', 10, None), (b'C', 10, None), (b'B', 10, None)], false, true, 20.0, 68.0),
        73 => d(b"HDB1", ONE, true, true, 16.0, 88.0), 74 => d(b"HDB2", ONE, true, true, 16.0, 88.0),
        75 => d(b"HDB3", ONE, true, true, 16.0, 64.0), 76 => d(b"HDB4", ONE, true, true, 16.0, 64.0),
        77 => d(b"HDB5", ONE, true, true, 16.0, 64.0), 78 => d(b"HDB6", ONE, true, true, 16.0, 64.0),
        79 => d(b"POB1", ONE, false, false, 16.0, 16.0), 80 => d(b"POB2", ONE, false, false, 16.0, 16.0),
        81 => d(b"BRS1", ONE, false, false, 16.0, 16.0),
        _ => return Option::None,
    })
}

// ------------------------------------------------------------- weapons

pub static WEAPONS: [Weapon; 9] = [
    Weapon { sprite: *b"PUNG", flash_sprite: *b"PUNG", ammo: Option::None, per_shot: 0, slot: 1,
        ready: &[(b'A', 1, Ready)], attack: &[(b'B', 4, None), (b'C', 4, Punch), (b'D', 5, None), (b'C', 4, None), (b'B', 5, ReFire)], flash: &[] },
    Weapon { sprite: *b"PISG", flash_sprite: *b"PISF", ammo: Some(AmmoKind::Bullets), per_shot: 1, slot: 2,
        ready: &[(b'A', 1, Ready)], attack: &[(b'A', 4, None), (b'B', 6, FirePistol), (b'C', 4, None), (b'B', 5, ReFire)], flash: &[(b'a', 7, Light1)] },
    Weapon { sprite: *b"SHTG", flash_sprite: *b"SHTF", ammo: Some(AmmoKind::Shells), per_shot: 1, slot: 3,
        ready: &[(b'A', 1, Ready)],
        attack: &[(b'A', 3, None), (b'A', 7, FireShotgun), (b'B', 5, None), (b'C', 5, None), (b'D', 4, None), (b'C', 5, None), (b'B', 5, None), (b'A', 3, None), (b'A', 7, ReFire)],
        flash: &[(b'a', 4, Light1), (b'b', 3, Light2)] },
    Weapon { sprite: *b"CHGG", flash_sprite: *b"CHGF", ammo: Some(AmmoKind::Bullets), per_shot: 1, slot: 4,
        ready: &[(b'A', 1, Ready)], attack: &[(b'A', 4, FireCGun), (b'B', 4, FireCGun), (b'B', 0, ReFire)], flash: &[(b'a', 5, Light1), (b'b', 5, Light2)] },
    Weapon { sprite: *b"MISG", flash_sprite: *b"MISF", ammo: Some(AmmoKind::Rockets), per_shot: 1, slot: 5,
        ready: &[(b'A', 1, Ready)], attack: &[(b'B', 8, GunFlash), (b'B', 12, FireMissile), (b'B', 0, ReFire)],
        flash: &[(b'a', 3, Light1), (b'b', 4, None), (b'c', 4, Light2), (b'd', 4, Light2)] },
    Weapon { sprite: *b"PLSG", flash_sprite: *b"PLSF", ammo: Some(AmmoKind::Cells), per_shot: 1, slot: 6,
        ready: &[(b'A', 1, Ready)], attack: &[(b'A', 3, FirePlasma), (b'B', 20, ReFire)], flash: &[(b'a', 4, Light1), (b'b', 4, Light1)] },
    Weapon { sprite: *b"BFGG", flash_sprite: *b"BFGF", ammo: Some(AmmoKind::Cells), per_shot: 40, slot: 7,
        ready: &[(b'A', 1, Ready)], attack: &[(b'A', 20, BFGSound), (b'B', 10, GunFlash), (b'B', 10, FireBFG), (b'B', 20, ReFire)],
        flash: &[(b'a', 11, Light1), (b'b', 6, Light2)] },
    Weapon { sprite: *b"SAWG", flash_sprite: *b"SAWG", ammo: Option::None, per_shot: 0, slot: 1,
        ready: &[(b'C', 4, Ready), (b'D', 4, Ready)], attack: &[(b'A', 4, Saw), (b'B', 4, Saw), (b'B', 0, ReFire)], flash: &[] },
    Weapon { sprite: *b"SHT2", flash_sprite: *b"SHT2", ammo: Some(AmmoKind::Shells), per_shot: 2, slot: 3,
        ready: &[(b'A', 1, Ready)],
        attack: &[(b'A', 3, None), (b'A', 7, FireShotgun2), (b'B', 7, None), (b'C', 7, CheckReload), (b'D', 7, OpenShotgun2), (b'E', 7, None), (b'F', 7, LoadShotgun2), (b'G', 6, None), (b'H', 6, CloseShotgun2), (b'A', 5, ReFire)],
        flash: &[(b'i', 5, Light1), (b'j', 4, Light2)] },
];

pub const MAX_AMMO: [i32; 4] = [200, 50, 300, 50];

/// The map the exit leads to, and the secret exit's map.
pub fn next_map(name: &str, secret: bool) -> Option<String> {
    let b = name.as_bytes();
    if b.len() == 4 && b[0] == b'E' {
        let (e, m) = ((b[1] - b'0') as u32, (b[3] - b'0') as u32);
        if secret { return Some(format!("E{}M9", e)); }
        let next = match (e, m) {
            (1, 9) => 4, (2, 9) => 6, (3, 9) => 7, (4, 9) => 3,
            (_, 8) => return Option::None,
            (_, m) => m + 1,
        };
        return Some(format!("E{}M{}", e, next));
    }
    let n: u32 = name.strip_prefix("MAP")?.parse().ok()?;
    let next = if secret { if n == 31 { 32 } else { 31 } } else {
        match n { 31 | 32 => 16, 30 => return Option::None, n => n + 1 }
    };
    Some(format!("MAP{:02}", next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exits_follow_dooms_order() {
        assert_eq!(next_map("E1M1", false).as_deref(), Some("E1M2"));
        assert_eq!(next_map("E1M3", true).as_deref(), Some("E1M9"));
        assert_eq!(next_map("E1M9", false).as_deref(), Some("E1M4"));
        assert_eq!(next_map("E1M8", false), Option::None);
        assert_eq!(next_map("MAP15", true).as_deref(), Some("MAP31"));
        assert_eq!(next_map("MAP31", true).as_deref(), Some("MAP32"));
        assert_eq!(next_map("MAP32", false).as_deref(), Some("MAP16"));
    }
}
