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
