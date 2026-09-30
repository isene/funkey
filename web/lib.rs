//! The funkey games for a web page, one per build, picked by feature. Each
//! game is the example itself: its `funkey::web!` line gives the exports.

#[cfg(feature = "eliminator")]
#[path = "../examples/eliminator.rs"]
#[allow(dead_code)]
mod eliminator;

#[cfg(feature = "stack")]
#[path = "../examples/stack.rs"]
#[allow(dead_code)]
mod stack;
