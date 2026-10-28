# TypeScript to Rust parity ledger

This ledger defines source-translation completion for the native showcase. A source module is
mechanically translated when its
declarations, state, helper functions, and observable behavior have a recognizable Rust
counterpart. Browser host operations are translated to the native substitution named here; they
are not silently omitted.

Every row below passed that source inventory on 2026-10-27. `translated (unvalidated)` records the
intentional boundary between the completed mechanical pass and the subsequent compiler-driven
repair pass; it is not a claim that the Rust compiler has accepted the code yet.

| TypeScript source | Rust counterpart | Translation status | Native substitution |
| --- | --- | --- | --- |
| `audio/audioManager.ts` | `audio/audio_manager.rs` | translated (unvalidated) | Native audio channel/resource host and preference store |
| `data/farmPropGeometry.ts` | `data/farm_prop_geometry.rs` | translated | None |
| `game/game.ts` | `game/game.rs` | translated (unvalidated) | Native preference store for records; winit input clock |
| `game/gameConfig.ts` | `game/game_config.rs` | translated | None |
| `game/gameMode.ts` | `game/game_mode.rs` | translated | Rust enum |
| `game/gamePhase.ts` | `game/game_phase.rs` | translated | Rust enum |
| `main.ts` | `main.rs` | translated (unvalidated) | winit/glutin lifecycle and native GL surface |
| `physics/colliderGeometry.ts` | `physics/collider_geometry.rs` | translated | None |
| `physics/pasture.ts` | `physics/pasture.rs` | translated | None |
| `physics/stackBodyRegistry.ts` | `physics/stack_body_registry.rs` | translated | Explicit registry owned by `HorseStackWorld` |
| `physics/stackObjectKind.ts` | `physics/stack_object_kind.rs` | translated | Caller-supplied random draw |
| `physics/stackObjectProfile.ts` | `physics/stack_object_profile.rs` | translated | Rust enum-indexed profile table |
| `physics/stackPhysics.ts` | `physics/stack_physics.rs` | translated | `HorseStackWorld` owns world and registries |
| `reducedMotion.ts` | `reduced_motion.rs` | translated (unvalidated) | Mutable native accessibility preference |
| `scene/cameraRig.ts` | `scene/camera_rig.rs` | translated | None |
| `scene/edgeProbe.ts` | `scene/edge_probe.rs` | translated (unvalidated) | `glow` framebuffer readback and stderr diagnostics |
| `scene/landingIndicator.ts` | `scene/landing_indicator.rs` | translated (unvalidated) | None |
| `scene/modelLoader.ts` | `scene/model_loader.rs` | translated | Filesystem glTF loading |
| `scene/particleEffects.ts` | `scene/particle_effects.rs` | translated (unvalidated) | None |
| `scene/playPlane.ts` | `scene/play_plane.rs` | translated (unvalidated) | winit cursor coordinates converted to NDC |
| `scene/sceneGraph.ts` | `scene/scene_graph.rs` | translated | None |
| `scene/sceneRenderer.ts` | `scene/scene_renderer.rs` | translated | Native GL surface instead of canvas |
| `scene/stackObjectVisual.ts` | `scene/stack_object_visual.rs` | translated | Rust reference handles |
| `scene/terrainProfile.ts` | `scene/terrain_profile.rs` | translated (unvalidated) | Explicit Flight node revision state |
| `scene/windmill.ts` | `scene/windmill.rs` | translated (unvalidated) | Native reduced-motion preference |
| `ui/gameUi.ts` | `ui/game_ui.rs` | translated (unvalidated) | Flight 2D nodes over the native GL surface |
| `ui/uiElements.ts` | `ui/ui_elements.rs` | translated (unvalidated) | Flight text metrics rather than browser canvas measurement |
| `ui/uiRenderer.ts` | `ui/ui_renderer.rs` | translated (unvalidated) | Native Flight GL state without web image/canvas hosts |

Host-only `styles.css` becomes native window/Flight-2D presentation and therefore has no literal
Rust file. `vite-env.d.ts` only augments the TypeScript build and has no runtime counterpart.
