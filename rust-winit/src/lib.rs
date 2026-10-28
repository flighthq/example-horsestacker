//! Native Horse Stacker game code shared by the executable and its tests.

pub mod assets;
pub mod audio;
pub mod data;
pub mod game;
pub mod physics;
pub mod reduced_motion;
pub mod native_storage;
#[cfg(feature = "flight-physics")]
pub mod scene;
#[cfg(feature = "native")]
pub mod ui;
