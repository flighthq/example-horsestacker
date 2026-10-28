# Horse Stacker: Rust + winit

This is the native counterpart to `../typescript-web/`. It uses the same Flight GL API through
`flight-sdk`, while `winit` and `glutin` provide the native window and OpenGL context that a browser
normally supplies.

The current foundation:

- opens a native window and owns its lifecycle through `winit`;
- creates a `glow` context and a Flight `GlRenderState`;
- clears and presents the window through a Flight GL render pass;
- resolves models and sounds from the repository's shared `../assets/` tree; and
- ports the round phase machine, both modes, queued-piece cadence and scoring rules;
- ports the stack profiles and collider geometry from TypeScript; and
- runs placed pieces through Flight's Rust 2D physics with the same materials, collision shapes,
  stack assistance, contacts, pasture bounds, and supported/settled-height rules as the web game.

The window currently exercises the round and physics rules with a deterministic piece sequence.
Scene rendering, model loading, aimed placement, audio, and UI remain to be ported. The Rust source
intentionally follows the TypeScript source file-for-file and declaration-for-declaration where a
counterpart exists. Names only change to Rust's conventional `snake_case`; enums, explicit
ownership, and `HorseStackWorld` provide the small amount of host-language adaptation needed by
Flight's Rust API.

| TypeScript web | Rust + winit |
| --- | --- |
| `src/main.ts` | `src/main.rs` |
| `src/game/game.ts` | `src/game/game.rs` |
| `src/game/gameConfig.ts` | `src/game/game_config.rs` |
| `src/game/gameMode.ts` | `src/game/game_mode.rs` |
| `src/game/gamePhase.ts` | `src/game/game_phase.rs` |
| `src/physics/colliderGeometry.ts` | `src/physics/collider_geometry.rs` |
| `src/physics/pasture.ts` | `src/physics/pasture.rs` |
| `src/physics/stackBodyRegistry.ts` | `src/physics/stack_body_registry.rs` |
| `src/physics/stackObjectKind.ts` | `src/physics/stack_object_kind.rs` |
| `src/physics/stackObjectProfile.ts` | `src/physics/stack_object_profile.rs` |
| `src/physics/stackPhysics.ts` | `src/physics/stack_physics.rs` |

## Run

Use the current stable Rust toolchain. The Flight dependency is pinned to revision
`9e8325c93cc9bd0aeef2924ad047a8eb93020e32` while its parity work is in progress.

```bash
cargo run --manifest-path rust-winit/Cargo.toml
```

Press <kbd>Enter</kbd> to start Time Challenge or <kbd>S</kbd> to start Steady Hands. Press
<kbd>Space</kbd> or left-click to place the current piece, <kbd>Backspace</kbd> to leave a round,
and <kbd>Escape</kbd> to exit. The current and next piece, clock, placement count, and live stack
height are shown in the window title; the Flight clear colour marks the round phase until the scene
renderer lands.

## Check

```bash
cargo fmt --manifest-path rust-winit/Cargo.toml --check
cargo test --manifest-path rust-winit/Cargo.toml --lib --no-default-features
cargo test --manifest-path rust-winit/Cargo.toml --lib --no-default-features --features flight-physics
cargo check --manifest-path rust-winit/Cargo.toml --all-targets
```

The lightweight test command deliberately omits the native feature so iteration on rules does not
compile the GL/window stack. The default `native` feature enables Flight GL, Flight physics, winit,
and glutin; `flight-physics` can also be enabled independently for headless solver validation.
