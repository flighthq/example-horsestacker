# TypeScript to Rust parity ledger

This ledger defines source-translation completion for the native showcase. A source module is
mechanically translated when its
declarations, state, helper functions, and observable behavior have a recognizable Rust
counterpart. Browser host operations are translated to the native substitution named here; they
are not silently omitted.

Every row below passed that source inventory on 2026-10-27. The complete native target now passes
`cargo check --all-targets`, `cargo build --all-targets`, and its 20 library tests. The lightweight
no-feature lane passes 10 tests and the headless `flight-physics` lane passes 20 tests.

| TypeScript source | Rust counterpart | Translation status | Native substitution |
| --- | --- | --- | --- |
| `audio/audioManager.ts` | `audio/audio_manager.rs` | translated and compiled | Native audio channel/resource host and preference store |
| `data/farmPropGeometry.ts` | `data/farm_prop_geometry.rs` | translated | None |
| `game/game.ts` | `game/game.rs` | translated and compiled | Native preference store for records; winit input clock |
| `game/gameConfig.ts` | `game/game_config.rs` | translated | None |
| `game/gameMode.ts` | `game/game_mode.rs` | translated | Rust enum |
| `game/gamePhase.ts` | `game/game_phase.rs` | translated | Rust enum |
| `main.ts` | `main.rs` | translated and compiled | winit/glutin lifecycle and native GL surface |
| `physics/colliderGeometry.ts` | `physics/collider_geometry.rs` | translated | None |
| `physics/pasture.ts` | `physics/pasture.rs` | translated | None |
| `physics/stackBodyRegistry.ts` | `physics/stack_body_registry.rs` | translated | Explicit registry owned by `HorseStackWorld` |
| `physics/stackObjectKind.ts` | `physics/stack_object_kind.rs` | translated | Caller-supplied random draw |
| `physics/stackObjectProfile.ts` | `physics/stack_object_profile.rs` | translated | Rust enum-indexed profile table |
| `physics/stackPhysics.ts` | `physics/stack_physics.rs` | translated | `HorseStackWorld` owns world and registries |
| `reducedMotion.ts` | `reduced_motion.rs` | translated and compiled | Mutable native accessibility preference |
| `scene/cameraRig.ts` | `scene/camera_rig.rs` | translated | None |
| `scene/edgeProbe.ts` | `scene/edge_probe.rs` | translated and compiled | `glow` framebuffer readback and stderr diagnostics |
| `scene/landingIndicator.ts` | `scene/landing_indicator.rs` | translated and compiled | None |
| `scene/modelLoader.ts` | `scene/model_loader.rs` | translated | Filesystem glTF loading |
| `scene/particleEffects.ts` | `scene/particle_effects.rs` | translated and compiled | None |
| `scene/playPlane.ts` | `scene/play_plane.rs` | translated and compiled | winit cursor coordinates converted to NDC |
| `scene/sceneGraph.ts` | `scene/scene_graph.rs` | translated | None |
| `scene/sceneRenderer.ts` | `scene/scene_renderer.rs` | translated | Native GL surface instead of canvas |
| `scene/stackObjectVisual.ts` | `scene/stack_object_visual.rs` | translated | Rust reference handles |
| `scene/terrainProfile.ts` | `scene/terrain_profile.rs` | translated and compiled | Explicit Flight node revision state |
| `scene/windmill.ts` | `scene/windmill.rs` | translated and compiled | Native reduced-motion preference |
| `ui/gameUi.ts` | `ui/game_ui.rs` | translated and compiled | Flight 2D nodes over the native GL surface |
| `ui/uiElements.ts` | `ui/ui_elements.rs` | translated and compiled | Flight text metrics rather than browser canvas measurement |
| `ui/uiRenderer.ts` | `ui/ui_renderer.rs` | translated and compiled | Native Flight GL state without web image/canvas hosts |

Host-only `styles.css` becomes native window/Flight-2D presentation and therefore has no literal
Rust file. `vite-env.d.ts` only augments the TypeScript build and has no runtime counterpart.
