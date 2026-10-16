//! What a scenario is made of. The rules know nothing of any war; a
//! scenario brings the map, the rivers, the cities, the forces, the dates
//! and the weather, and the rules play it.

/// What a unit is, for the rules: tanks exploit a breakthrough, mountain
/// troops climb, cavalry is quick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind { Inf, Arm, Mot, Cav, Mtn }

impl Kind {
    /// Tanks and motorised troops move again after the battles.
    pub fn mech(self) -> bool { matches!(self, Kind::Arm | Kind::Mot) }
}

pub struct Nation {
    pub name: &'static str,
    /// 0 or 1: which of the two sides it fights for.
    pub side: usize,
    /// The counter's colour and the colour printed on it.
    pub paper: u32,
    pub ink: u32,
}

/// A kind of unit with its strength: attack, defence and movement at
/// full strength. A reduced unit has about half the attack and two
/// thirds of the defence.
#[derive(Clone, Copy)]
pub struct Kit {
    pub nat: usize,
    pub kind: Kind,
    pub a: i32,
    pub d: i32,
    pub m: i32,
    /// "PANZER CORPS", for the side panel.
    pub what: &'static str,
}

/// One unit of the order of battle. On turn 1 it starts at its hex; a
/// later turn brings it in there, or as near as it can get.
pub struct Force {
    pub name: &'static str,
    pub kit: Kit,
    pub col: i32,
    pub row: i32,
    pub turn: usize,
}

pub struct City {
    pub name: &'static str,
    pub col: i32,
    pub row: i32,
    /// 0 a town, 1 a city, 2 a great city: each is harder to take.
    pub size: u8,
    /// Victory points to the side that holds it at the end.
    pub vp: i32,
    /// The side that holds it at the start.
    pub owner: usize,
}

/// A river as points of longitude and latitude, from its source down.
pub struct River {
    pub name: &'static str,
    pub points: &'static [(f32, f32)],
}

pub struct Scenario {
    pub name: &'static str,
    /// A few lines for the title screen.
    pub about: &'static [&'static str],
    pub sides: [&'static str; 2],
    pub nations: &'static [Nation],
    /// One string per row of hexes, one character per hex:
    /// `.` clear, `f` forest, `r` rough, `m` marsh, `^` mountains, `~` sea.
    pub ground: &'static [&'static str],
    /// Where the map lies on the globe: the longitude at its left edge,
    /// the latitude at its top, and degrees of each per pixel.
    pub geo: (f32, f32, f32, f32),
    pub rivers: &'static [River],
    pub cities: &'static [City],
    /// The date of each turn.
    pub dates: &'static [&'static str],
    /// One letter per turn: `d` dry, `m` mud, `s` snow.
    pub weather: &'static str,
    /// The side that suffers in the snow.
    pub cold: usize,
    /// For the computer, one letter per turn and side: `a` attacks,
    /// `d` defends.
    pub posture: [&'static str; 2],
    /// True for a hex on a side's supply edge.
    pub supply: fn(side: usize, col: i32, row: i32) -> bool,
    pub forces: &'static [Force],
    /// The result at the end, by the victory points side 0 holds: the
    /// first line whose number it reaches.
    pub levels: &'static [(i32, &'static str)],
}
