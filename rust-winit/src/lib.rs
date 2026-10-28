//! Native Horse Stacker game code shared by the executable and its tests.

pub mod assets;
#[cfg(feature = "flight-physics")]
pub mod audio;
pub mod data;
#[cfg(feature = "flight-physics")]
pub mod game;
#[cfg(feature = "native")]
pub mod native_2d;
#[cfg(feature = "native")]
pub mod native_gl;
pub mod native_storage;
pub mod physics;
pub mod reduced_motion;
#[cfg(feature = "flight-physics")]
pub mod scene;
#[cfg(feature = "native")]
pub mod ui;
