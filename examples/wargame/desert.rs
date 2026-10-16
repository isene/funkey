//! North Africa 1942: Rommel's drive from the Gazala line to the gates of
//! Egypt, and the British answer at El Alamein. Fourteen turns of two
//! weeks each, from 26 May to the end of November. A hex is about 20 km
//! across, and a unit is a division or a brigade. In the desert supply
//! runs no further than ten hexes from a side's edge or a port it holds.

use super::scenario::*;

const DE: usize = 0;
const IT: usize = 1;
const UK: usize = 2;
const FR: usize = 3;

const NATIONS: &[Nation] = &[
    Nation { name: "GERMAN", side: 0, paper: 0x9aa6a8, ink: 0x10161a },
    Nation { name: "ITALIAN", side: 0, paper: 0x8fbf8a, ink: 0x10240c },
    Nation { name: "COMMONWEALTH", side: 1, paper: 0xd29a5e, ink: 0x24140a },
    Nation { name: "FREE FRENCH", side: 1, paper: 0x86a4dc, ink: 0x0a1430 },
];

const GP: Kit = Kit { nat: DE, kind: Kind::Arm, a: 8, d: 5, m: 8, what: "PANZER DIVISION", size: 2 };
const GL: Kit = Kit { nat: DE, kind: Kind::Mot, a: 5, d: 4, m: 7, what: "LIGHT DIVISION", size: 2 };
const GF: Kit = Kit { nat: DE, kind: Kind::Inf, a: 4, d: 4, m: 4, what: "LIGHT DIVISION", size: 2 };
const GB: Kit = Kit { nat: DE, kind: Kind::Inf, a: 3, d: 4, m: 4, what: "PARACHUTE BRIGADE", size: 1 };
const IA: Kit = Kit { nat: IT, kind: Kind::Arm, a: 4, d: 3, m: 7, what: "ARMOURED DIVISION", size: 2 };
const IM: Kit = Kit { nat: IT, kind: Kind::Mot, a: 3, d: 3, m: 7, what: "MOTORISED DIVISION", size: 2 };
const II: Kit = Kit { nat: IT, kind: Kind::Inf, a: 2, d: 3, m: 4, what: "INFANTRY DIVISION", size: 2 };
const IP: Kit = Kit { nat: IT, kind: Kind::Inf, a: 3, d: 4, m: 4, what: "PARACHUTE DIVISION", size: 2 };
const BA: Kit = Kit { nat: UK, kind: Kind::Arm, a: 6, d: 4, m: 8, what: "ARMOURED DIVISION", size: 2 };
const BI: Kit = Kit { nat: UK, kind: Kind::Inf, a: 3, d: 4, m: 5, what: "INFANTRY DIVISION", size: 2 };
const BV: Kit = Kit { nat: UK, kind: Kind::Inf, a: 5, d: 5, m: 5, what: "INFANTRY DIVISION", size: 2 };
const BT: Kit = Kit { nat: UK, kind: Kind::Arm, a: 3, d: 4, m: 5, what: "ARMY TANK BRIGADE", size: 1 };
const BM: Kit = Kit { nat: UK, kind: Kind::Mot, a: 2, d: 3, m: 7, what: "MOTOR BRIGADE", size: 1 };
const BB: Kit = Kit { nat: UK, kind: Kind::Inf, a: 2, d: 3, m: 5, what: "INFANTRY BRIGADE", size: 1 };
const FB: Kit = Kit { nat: FR, kind: Kind::Inf, a: 2, d: 4, m: 5, what: "BRIGADE", size: 1 };

const fn f(name: &'static str, kit: Kit, col: i32, row: i32, turn: usize) -> Force { Force { name, kit, col, row, turn } }

const FORCES: &[Force] = &[
    // Panzerarmee Afrika on 26 May: the Italian infantry faces the line,
    // the armour gathers in the south to go round Bir Hakeim.
    f("SABRATHA", II, 3, 3, 1),
    f("TRENTO", II, 3, 4, 1),
    f("BRESCIA", II, 3, 5, 1),
    f("PAVIA", II, 3, 6, 1),
    f("15 PZ", GP, 2, 6, 1),
    f("21 PZ", GP, 2, 7, 1),
    f("90 LIGHT", GL, 3, 7, 1),
    f("ARIETE", IA, 2, 8, 1),
    f("TRIESTE", IM, 3, 8, 1),
    // What comes over the sea after it.
    f("BOLOGNA", II, 0, 4, 2),
    f("LITTORIO", IA, 0, 5, 3),
    f("164 LIGHT", GF, 0, 4, 4),
    f("FOLGORE", IP, 0, 5, 5),
    f("RAMCKE", GB, 0, 4, 6),
    f("PISTOIA", II, 0, 5, 9),
    // The Eighth Army in the Gazala line, its boxes and its armour.
    f("1 SA", BI, 5, 3, 1),
    f("50", BI, 5, 4, 1),
    f("1 ARMY TK", BT, 5, 5, 1),
    f("1 FF", FB, 6, 6, 1),
    f("1 ARMD", BA, 6, 4, 1),
    f("201 GDS", BM, 7, 4, 1),
    f("32 ARMY TK", BT, 7, 5, 1),
    f("7 ARMD", BA, 7, 6, 1),
    f("3 IND MOT", BM, 6, 7, 1),
    f("2 SA", BI, 8, 4, 1),
    f("5 IND", BI, 8, 5, 1),
    f("29 IND", BB, 8, 6, 1),
    // Help from the rest of the Empire.
    f("10 IND", BI, 23, 7, 2),
    f("2 NZ", BV, 23, 7, 3),
    f("9 AUS", BV, 35, 7, 4),
    f("44", BI, 35, 7, 6),
    f("8 ARMD", BA, 41, 13, 7),
    f("51 HD", BI, 35, 7, 7),
    f("10 ARMD", BA, 41, 13, 8),
    f("4 IND", BI, 35, 7, 9),
];

const fn c(name: &'static str, col: i32, row: i32, size: u8, vp: i32, owner: usize) -> City { City { name, col, row, size, vp, owner, port: false } }
const fn p(name: &'static str, col: i32, row: i32, size: u8, vp: i32, owner: usize) -> City { City { name, col, row, size, vp, owner, port: true } }

const CITIES: &[City] = &[
    p("DERNA", 2, 1, 0, 1, 0),
    p("TOBRUK", 8, 4, 1, 3, 1),
    p("BARDIA", 13, 5, 0, 1, 1),
    c("SIDI BARRANI", 17, 6, 0, 1, 1),
    p("MATRUH", 23, 7, 1, 2, 1),
    c("EL ALAMEIN", 31, 9, 0, 1, 1),
    p("ALEXANDRIA", 35, 7, 2, 4, 1),
    c("CAIRO", 41, 13, 2, 4, 1),
    c("MECHILI", 1, 3, 0, 0, 0),
    c("GAZALA", 5, 3, 0, 0, 1),
    c("BIR HAKEIM", 6, 6, 1, 0, 1),
    c("KNIGHTSBRIDGE", 7, 4, 0, 0, 1),
    c("EL ADEM", 8, 5, 0, 0, 1),
    c("GAMBUT", 11, 4, 0, 0, 1),
    c("SOLLUM", 14, 6, 0, 0, 1),
    c("JARABUB", 11, 14, 0, 0, 1),
    c("FUKA", 26, 8, 0, 0, 1),
    c("EL DABA", 29, 8, 0, 0, 1),
    c("DAMANHUR", 38, 9, 0, 0, 1),
];

const RIVERS: &[River] = &[River { name: "NILE", points: &[(31.24, 30.05), (31.13, 30.19), (31.0, 30.45), (30.85, 30.75), (30.65, 31.1), (30.42, 31.45)] }];

fn supply(side: usize, col: i32, _row: i32) -> bool { if side == 0 { col == 0 } else { col == 43 } }

pub const SCENARIO: Scenario = Scenario {
    name: "NORTH AFRICA 1942",
    about: &[
        "May 1942. Rommel strikes at the Gazala line,",
        "bound for Tobruk, Alexandria and the Nile.",
        "In the desert supply runs ten hexes from the",
        "edge or a port. Fourteen turns of two weeks.",
    ],
    sides: ["AXIS", "ALLIES"],
    nations: NATIONS,
    land: Land::Desert,
    ground: &[
        "rr~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
        "rr..~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
        ".....~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
        "......~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
        ".........r..~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
        ".............r~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
        "..............rr....~~~~~~~~~~~~~~~~~~~~~~~~",
        "................r.......~~~~~~~~~~~.~fffffff",
        "............................~.~~~~~m..ffffff",
        "................................~.....ffffff",
        "..............................r.r.......ffff",
        ".....................................m..ffff",
        "...........................mmm............ff",
        ".......................mmmmmmmmm..........f.",
        ".....................mmmmmmmmmmmm...........",
        ".........^.^.^.^...mmmmmmmmmmmmmm...........",
        ".......^^^^^^^^^^^.mmmmmmmmmmmm.............",
    ],
    geo: (22.0, 33.0, 0.22 / 27.0, 0.218 / 31.176915),
    rivers: RIVERS,
    cities: CITIES,
    dates: &["MAY 26", "JUN 9", "JUN 23", "JUL 7", "JUL 21", "AUG 4", "AUG 18", "SEP 1", "SEP 15", "SEP 29", "OCT 13", "OCT 27", "NOV 10", "NOV 24"],
    weather: "dddddddddddddd",
    cold: 2,
    posture: ["aaaaaaaadddddd", "dddddddddaaaaa"],
    supply,
    range: 10,
    forces: FORCES,
    levels: &[(13, "AXIS DECISIVE VICTORY"), (8, "AXIS VICTORY"), (5, "DRAW"), (1, "ALLIED VICTORY"), (0, "ALLIED DECISIVE VICTORY")],
};
