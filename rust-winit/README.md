# Horse Stacker: Rust + winit

This is the native counterpart to `../typescript-web/`. It uses the same Flight GL API through
`flight-sdk`, while `winit` and `glutin` provide the native window and OpenGL context that a browser
normally supplies.

The current foundation:

- opens a native window and owns its lifecycle through `winit`;
- creates a `glow` context and a Flight `GlRenderState`;
- clears and presents the window through a Flight GL render pass;
- resolves models and sounds from the repository's shared `../assets/` tree; and
- ports the round phase machine, both modes, queued-piece cadence and scoring rules; and
- ports the stack profiles and collider geometry from TypeScript.

The window currently exercises the round rules with a deterministic piece sequence. The scene,
model loading, Flight physics world, aimed placement, audio, and UI remain to be ported. The pure
modules are intentionally organized like the TypeScript app, but use Rust enums and ownership
instead of copying TypeScript shapes literally.

## Run

Use the current stable Rust toolchain. The Flight dependency is pinned to revision
`9e8325c93cc9bd0aeef2924ad047a8eb93020e32` while its parity work is in progress.

```bash
cargo run --manifest-path rust-winit/Cargo.toml
```

Press <kbd>Enter</kbd> to start Time Challenge or <kbd>S</kbd> to start Steady Hands. Press
<kbd>Space</kbd> or left-click to place the current piece, <kbd>Backspace</kbd> to leave a round,
and <kbd>Escape</kbd> to exit. Until the Flight physics/scene slice lands, the current and next
piece, clock, and placement count are shown in the window title and the Flight clear colour marks
the round phase.

## Check

```bash
cargo fmt --manifest-path rust-winit/Cargo.toml --check
cargo test --manifest-path rust-winit/Cargo.toml --lib --no-default-features
cargo check --manifest-path rust-winit/Cargo.toml --all-targets
```

The lightweight test command deliberately omits the native feature so iteration on rules does not
compile the GL/window stack. The default `native` feature enables Flight GL, Flight physics, winit,
and glutin; `flight-physics` can also be enabled independently for future headless validation.
