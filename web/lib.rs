//! The funkey games for a web page, one per build, picked by feature. Each
//! game is the example itself: its `funkey::web!` line gives the exports.

#[cfg(feature = "climb")]
#[path = "../examples/climb.rs"]
#[allow(dead_code)]
mod climb;

#[cfg(feature = "drive")]
#[path = "../examples/drive.rs"]
#[allow(dead_code)]
mod drive;

#[cfg(feature = "eliminator")]
#[path = "../examples/eliminator.rs"]
#[allow(dead_code)]
mod eliminator;

#[cfg(feature = "gems")]
#[path = "../examples/gems.rs"]
#[allow(dead_code)]
mod gems;

#[cfg(feature = "invaders")]
#[path = "../examples/invaders.rs"]
#[allow(dead_code)]
mod invaders;

#[cfg(feature = "jumpman")]
#[path = "../examples/jumpman.rs"]
#[allow(dead_code)]
mod jumpman;

#[cfg(feature = "salvo")]
#[path = "../examples/salvo.rs"]
#[allow(dead_code)]
mod salvo;

#[cfg(feature = "stack")]
#[path = "../examples/stack.rs"]
#[allow(dead_code)]
mod stack;

#[cfg(feature = "vector")]
#[path = "../examples/vector.rs"]
#[allow(dead_code)]
mod vector;

#[cfg(feature = "marble")]
#[path = "../examples/marble.rs"]
#[allow(dead_code)]
mod marble;

#[cfg(feature = "again")]
#[path = "../examples/again.rs"]
#[allow(dead_code)]
mod again;

#[cfg(feature = "chomp")]
#[path = "../examples/chomp.rs"]
#[allow(dead_code)]
mod chomp;

#[cfg(feature = "kart")]
#[path = "../examples/kart.rs"]
#[allow(dead_code)]
mod kart;

#[cfg(feature = "raid")]
#[path = "../examples/raid.rs"]
#[allow(dead_code)]
mod raid;

#[cfg(feature = "soar")]
#[path = "../examples/soar.rs"]
#[allow(dead_code)]
mod soar;

#[cfg(feature = "castle")]
#[path = "../examples/castle.rs"]
#[allow(dead_code)]
mod castle;

#[cfg(feature = "blocks")]
#[path = "../examples/blocks.rs"]
#[allow(dead_code)]
mod blocks;

#[cfg(feature = "wargame")]
#[path = "../examples/wargame/main.rs"]
#[allow(dead_code)]
mod wargame;
