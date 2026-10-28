# Horse Stacker: Rust + winit

This is the native counterpart to `../typescript-web/`. It uses the same Flight GL API through
`flight-sdk`, while `winit` and `glutin` provide the native window and OpenGL context that a browser
normally supplies.

The current foundation:

- opens a native window and owns its lifecycle through `winit`;
- creates a `glow` context and a Flight `GlRenderState`;
- clears and presents the window through a Flight GL render pass;
- resolves models and sounds from the repository's shared `../assets/` tree; and
- ports the first pure game rules, stack profiles, and collider geometry from TypeScript.

The scene, model loading, physics world, input, audio, and UI remain to be ported. The pure modules
are intentionally organized like the TypeScript app, but use Rust enums and ownership instead of
copying TypeScript shapes literally.

## Run

Use the current stable Rust toolchain. The Flight dependency is pinned to revision
`9e8325c93cc9bd0aeef2924ad047a8eb93020e32` while its parity work is in progress.

```bash
cargo run --manifest-path rust-winit/Cargo.toml
```

Press <kbd>Escape</kbd> or close the window to exit.

## Check

```bash
cargo fmt --manifest-path rust-winit/Cargo.toml --check
cargo test --manifest-path rust-winit/Cargo.toml
```
