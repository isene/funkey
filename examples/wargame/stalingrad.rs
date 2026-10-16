//! Stalingrad 1942: the German summer drive to the Volga and the
//! Caucasus oil, and the Soviet winter answer. Fourteen turns of two
//! weeks each, from 28 June to the end of December. A hex is about 40 km
//! across; a German or allied unit is a corps, a Soviet one an army.

use super::scenario::*;

const DE: usize = 0;
const RO: usize = 1;
const HU: usize = 2;
const IT: usize = 3;
const SU: usize = 4;

const NATIONS: &[Nation] = &[
    Nation { name: "GERMAN", side: 0, paper: 0x9aa6a8, ink: 0x10161a },
    Nation { name: "ROMANIAN", side: 0, paper: 0xd8c47c, ink: 0x2a2010 },
    Nation { name: "HUNGARIAN", side: 0, paper: 0xb4c48c, ink: 0x1a2410 },
    Nation { name: "ITALIAN", side: 0, paper: 0x8fbf8a, ink: 0x10240c },
    Nation { name: "SOVIET", side: 1, paper: 0xc9573f, ink: 0x1c0a06 },
];

const PZ: Kit = Kit { nat: DE, kind: Kind::Arm, a: 9, d: 6, m: 8, what: "PANZER CORPS", size: 3 };
const GI: Kit = Kit { nat: DE, kind: Kind::Inf, a: 6, d: 6, m: 4, what: "INFANTRY CORPS", size: 3 };
const PD: Kit = Kit { nat: DE, kind: Kind::Arm, a: 6, d: 4, m: 8, what: "PANZER DIVISION", size: 2 };
const GM: Kit = Kit { nat: DE, kind: Kind::Mtn, a: 5, d: 6, m: 4, what: "MOUNTAIN CORPS", size: 3 };
const RI: Kit = Kit { nat: RO, kind: Kind::Inf, a: 3, d: 4, m: 4, what: "ARMY CORPS", size: 3 };
const RC: Kit = Kit { nat: RO, kind: Kind::Cav, a: 2, d: 3, m: 6, what: "CAVALRY CORPS", size: 3 };
const HI: Kit = Kit { nat: HU, kind: Kind::Inf, a: 3, d: 4, m: 4, what: "ARMY CORPS", size: 3 };
const II: Kit = Kit { nat: IT, kind: Kind::Inf, a: 3, d: 4, m: 4, what: "ARMY CORPS", size: 3 };
const IA: Kit = Kit { nat: IT, kind: Kind::Mtn, a: 3, d: 5, m: 4, what: "ALPINE CORPS", size: 3 };
const SR: Kit = Kit { nat: SU, kind: Kind::Inf, a: 4, d: 5, m: 4, what: "RIFLE ARMY", size: 4 };
const SG: Kit = Kit { nat: SU, kind: Kind::Inf, a: 5, d: 6, m: 4, what: "GUARDS ARMY", size: 4 };
const ST: Kit = Kit { nat: SU, kind: Kind::Arm, a: 7, d: 4, m: 7, what: "TANK ARMY", size: 4 };
const SC: Kit = Kit { nat: SU, kind: Kind::Cav, a: 3, d: 3, m: 6, what: "CAVALRY CORPS", size: 3 };
const SM: Kit = Kit { nat: SU, kind: Kind::Mot, a: 6, d: 4, m: 7, what: "MECHANISED CORPS", size: 3 };

const fn f(name: &'static str, kit: Kit, col: i32, row: i32, turn: usize) -> Force { Force { name, kit, col, row, turn } }

const FORCES: &[Force] = &[
    // The German line on 28 June, from east of Kursk to Taganrog.
    f("LV", GI, 3, 0, 1),
    f("VII", GI, 3, 1, 1),
    f("XIII", GI, 3, 2, 1),
    f("HUN III", HI, 4, 3, 1),
    f("HUN IV", HI, 4, 4, 1),
    f("XXIX", GI, 4, 5, 1),
    f("XVII", GI, 4, 6, 1),
    f("VIII", GI, 4, 7, 1),
    f("LI", GI, 4, 8, 1),
    f("XLIV", GI, 5, 9, 1),
    f("LII", GI, 6, 10, 1),
    f("IV", GI, 7, 11, 1),
    f("ITA XXXV", II, 7, 12, 1),
    f("V", GI, 8, 13, 1),
    f("XLIX MTN", GM, 8, 14, 1),
    // The panzers behind it.
    f("XXIV PZ", PZ, 2, 1, 1),
    f("XLVIII PZ", PZ, 2, 2, 1),
    f("XL PZ", PZ, 3, 4, 1),
    f("III PZ", PZ, 3, 6, 1),
    f("XIV PZ", PZ, 3, 8, 1),
    f("LVII PZ", PZ, 6, 12, 1),
    // Germany's allies come up through the summer.
    f("ITA II", II, 0, 8, 2),
    f("ITA ALPINI", IA, 0, 9, 3),
    f("ROM I", RI, 0, 12, 3),
    f("ROM II", RI, 0, 13, 3),
    f("ROM IV", RI, 0, 11, 4),
    f("ROM V", RI, 0, 12, 4),
    f("ROM VI", RI, 0, 12, 5),
    f("ROM VII", RI, 0, 13, 5),
    f("ROM CAV", RC, 0, 10, 5),
    f("6 PZ DIV", PD, 0, 12, 12),
    // The Soviet line facing it.
    f("48", SR, 4, 0, 1),
    f("13", SR, 4, 1, 1),
    f("40", SR, 4, 2, 1),
    f("21", SR, 5, 3, 1),
    f("28", SR, 5, 4, 1),
    f("38", SR, 5, 5, 1),
    f("9", SR, 5, 6, 1),
    f("57", SR, 5, 7, 1),
    f("37", SR, 5, 8, 1),
    f("12", SR, 6, 9, 1),
    f("18", SR, 7, 10, 1),
    f("56", SR, 8, 11, 1),
    f("24", SR, 8, 12, 1),
    f("60", SR, 9, 13, 1),
    f("6", SR, 9, 14, 1),
    f("3 TANK", ST, 6, 2, 1),
    f("3 GDS CAV", SC, 9, 11, 1),
    // Far behind it, in the Caucasus.
    f("47", SR, 8, 20, 1),
    f("51", SR, 14, 16, 1),
    f("44", SR, 22, 24, 1),
    // What the Stavka sends.
    f("62", SR, 16, 0, 2),
    f("63", SR, 12, 0, 2),
    f("64", SR, 29, 8, 2),
    f("1 TANK", ST, 20, 0, 3),
    f("4 TANK", ST, 22, 0, 3),
    f("1 GDS", SG, 18, 0, 4),
    f("66", SR, 14, 0, 4),
    f("58", SR, 29, 14, 5),
    f("65", SR, 16, 0, 6),
    f("4 CAV", SC, 29, 12, 7),
    f("46", SR, 24, 25, 8),
    // Operation Uranus.
    f("5 TANK", ST, 15, 0, 11),
    f("4 MECH", SM, 29, 9, 11),
    f("13 MECH", SM, 29, 11, 11),
    f("8 CAV", SC, 17, 0, 11),
    f("2 GDS", SG, 29, 10, 12),
    f("5 SHOCK", SG, 16, 0, 12),
    f("3 GDS", SG, 13, 0, 13),
];

const fn c(name: &'static str, col: i32, row: i32, size: u8, vp: i32, owner: usize) -> City { City { name, col, row, size, vp, owner, port: false } }

const CITIES: &[City] = &[
    c("STALINGRAD", 21, 9, 2, 4, 1),
    c("ROSTOV", 10, 14, 2, 2, 1),
    c("VORONEZH", 9, 1, 2, 2, 1),
    c("KHARKOV", 2, 6, 2, 2, 0),
    c("KURSK", 2, 1, 1, 1, 0),
    c("MAYKOP", 11, 21, 1, 2, 1),
    c("GROZNY", 23, 25, 1, 3, 1),
    c("ASTRAKHAN", 28, 16, 1, 2, 1),
    c("KRASNODAR", 8, 20, 1, 1, 1),
    c("NOVOROSSIYSK", 6, 21, 1, 1, 1),
    c("STAVROPOL", 15, 20, 0, 1, 1),
    c("ELISTA", 20, 17, 0, 1, 1),
    c("STALINO", 6, 12, 1, 0, 0),
    c("VOROSHILOVGRAD", 9, 10, 1, 0, 1),
    c("TAGANROG", 8, 14, 0, 0, 0),
    c("BELGOROD", 3, 4, 0, 0, 0),
    c("KUPYANSK", 5, 6, 0, 0, 1),
    c("IZYUM", 4, 8, 0, 0, 0),
    c("OSTROGOZHSK", 8, 4, 0, 0, 1),
    c("MILLEROVO", 11, 9, 0, 0, 1),
    c("KALACH", 18, 10, 0, 0, 1),
    c("SERAFIMOVICH", 17, 7, 0, 0, 1),
    c("KOTELNIKOVO", 17, 12, 0, 0, 1),
    c("SALSK", 14, 16, 0, 0, 1),
    c("TIKHORETSK", 11, 17, 0, 0, 1),
    c("ARMAVIR", 13, 20, 0, 0, 1),
    c("PYATIGORSK", 17, 23, 0, 0, 1),
    c("MOZDOK", 21, 23, 0, 0, 1),
    c("ORDZHONIKIDZE", 21, 25, 0, 0, 1),
    c("TUAPSE", 8, 23, 0, 0, 1),
    c("KAMYSHIN", 22, 6, 0, 0, 1),
];

const RIVERS: &[River] = &[
    River { name: "DON", points: &[(38.95, 52.4), (39.0, 51.9), (39.0, 51.5), (39.3, 51.1), (39.55, 50.9), (40.1, 50.45), (40.4, 50.2), (40.9, 49.95), (41.3, 49.8), (41.8, 49.62), (42.7, 49.55), (43.1, 49.3), (43.45, 49.05), (43.55, 48.7), (43.2, 48.3), (42.7, 47.9), (42.1, 47.6), (41.1, 47.55), (40.6, 47.45), (39.9, 47.1), (39.4, 47.05), (39.15, 46.95)] },
    River { name: "VOLGA", points: &[(46.2, 52.4), (46.05, 51.6), (45.75, 51.0), (45.45, 50.2), (45.15, 49.6), (45.0, 49.1), (45.05, 48.6), (45.4, 48.3), (46.0, 48.0), (46.8, 47.6), (47.4, 47.15), (47.6, 46.6), (47.8, 46.15), (48.1, 45.85)] },
    River { name: "DONETS", points: &[(36.6, 51.0), (36.7, 50.55), (36.95, 50.25), (36.7, 49.85), (36.85, 49.45), (37.25, 49.15), (37.9, 49.0), (38.4, 48.9), (39.0, 48.75), (39.6, 48.55), (40.3, 48.3), (40.8, 48.15), (41.0, 47.8), (41.1, 47.58)] },
    River { name: "OSKOL", points: &[(37.85, 51.6), (37.84, 51.3), (37.88, 50.76), (38.05, 50.2), (37.7, 49.7), (37.45, 49.25), (37.4, 49.15)] },
    River { name: "MIUS", points: &[(38.9, 48.25), (38.95, 47.75), (39.0, 47.45), (39.1, 47.25), (39.05, 46.95)] },
    River { name: "CHIR", points: &[(41.4, 49.55), (41.9, 49.1), (42.5, 48.65), (43.1, 48.4)] },
    River { name: "MANYCH", points: &[(40.4, 47.3), (41.0, 47.0), (41.7, 46.7), (42.5, 46.4), (43.3, 46.15), (44.2, 45.95)] },
    River { name: "KUBAN", points: &[(41.9, 43.9), (41.7, 44.5), (41.2, 45.0), (40.6, 45.4), (39.8, 45.15), (39.05, 44.95), (38.4, 45.1), (37.6, 45.25)] },
    River { name: "TEREK", points: &[(44.6, 43.0), (44.65, 43.7), (45.4, 43.75), (46.1, 43.75), (46.7, 43.85), (47.2, 43.65)] },
];

fn supply(side: usize, col: i32, row: i32) -> bool {
    match side {
        0 => col == 0 || row == 0 && col <= 3,
        _ => col == 29 || row == 0 && col >= 7 || row == 25 && col >= 10,
    }
}

pub const SCENARIO: Scenario = Scenario {
    name: "STALINGRAD 1942",
    about: &[
        "Summer 1942. The German army in the south",
        "drives for the Volga and the oil of the",
        "Caucasus. In November the Soviet answer",
        "comes. Fourteen turns of two weeks each.",
    ],
    sides: ["AXIS", "SOVIET"],
    nations: NATIONS,
    land: Land::Steppe,
    ground: &[
        "........fff............ff.....",
        "........f.f...................",
        "...............f..............",
        "..............................",
        "..............................",
        "..............................",
        "..............................",
        ".....f........................",
        "....ff........................",
        "....f.........................",
        ".......r.r....................",
        "......rrrrr...................",
        "......r.r.r.........r.........",
        "....................r.........",
        ".....~~~............r.........",
        ".....~~~~...........r......m.m",
        ".~~~~~~.........mmm.r......mmm",
        "~~~~~~~...........m.r......mmm",
        "~~~~~~m.....................m~",
        "~.~.~mm.......rrrrr........~~~",
        ".~.~..........rrrrr........~~~",
        "~~~~~~^fffff..rrrrr......m.~~~",
        "~~~~~~~~^^f^.......rr....mm~~~",
        "~~~~~~~~~^^^^^.^...rrr...mm.~~",
        "~~~~~~~~~~^^^^^^^^........m.~~",
        "~~~~~~~~~~~~^^^^^^^^.^......~~",
    ],
    geo: (35.0, 52.3, 0.01667, 0.01126),
    rivers: RIVERS,
    cities: CITIES,
    dates: &["JUN 28", "JUL 12", "JUL 26", "AUG 9", "AUG 23", "SEP 6", "SEP 20", "OCT 4", "OCT 18", "NOV 1", "NOV 15", "NOV 29", "DEC 13", "DEC 27"],
    weather: "ddddddddmmssss",
    cold: 0,
    posture: ["aaaaaaaaaadddd", "ddddddddddaaaa"],
    supply,
    range: 0,
    forces: FORCES,
    levels: &[(16, "AXIS DECISIVE VICTORY"), (12, "AXIS VICTORY"), (9, "DRAW"), (6, "SOVIET VICTORY"), (0, "SOVIET DECISIVE VICTORY")],
};
