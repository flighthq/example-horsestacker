/// The pressure applied to a Horse Stacker round.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameMode {
    /// Stack for thirty seconds, then score the settled pile.
    Time,
    /// Stack until a horse leaves the pasture, then score the peak height.
    Steady,
}
