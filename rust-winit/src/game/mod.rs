// Keep this module name parallel with TypeScript's `game/game.ts`. The public items are
// re-exported below, so callers still use the idiomatic `crate::game::Game` path.
#[allow(clippy::module_inception)]
mod game;
mod game_config;
mod game_mode;
mod game_phase;

pub use game::*;
pub use game_config::*;
pub use game_mode::GameMode;
pub use game_phase::GamePhase;
