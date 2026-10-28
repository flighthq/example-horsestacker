/// The round state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GamePhase {
    Loading,
    Ready,
    Playing,
    Settling,
    Finished,
}
