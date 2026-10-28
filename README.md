# Horse Stacker

Horse Stacker is an end-to-end Flight showcase implemented twice:

- `typescript-web/` is the complete browser game built with TypeScript and Flight's web host.
- `rust-winit/` is its native Rust and winit counterpart, currently under construction.

Both applications use the canonical models and audio in `assets/`. Their implementations stay
independent so each one remains an idiomatic, self-contained example of its Flight API.

## TypeScript web application

```bash
cd typescript-web
npm install
npm run dev
```

Move the pointer or use <kbd>←</kbd>/<kbd>→</kbd> to position the yellow marker. Click or tap
anywhere on the page, or press <kbd>Space</kbd>, <kbd>Enter</kbd>, or <kbd>↓</kbd>, to reveal and
place the object.

### Checks

```bash
cd typescript-web
npm run build
npm run validate:assets
npm run validate:game
```

The gameplay validation runs 64-object level, balanced, and teetered placement scenarios using
the shipped timing and surface-placement rules. It also checks the weighted prop distribution,
both hen variants, finite body state, contacts, distinct 2D colliders, supported pile height,
zero-impulse physics activation, horse-height calibration, and farm-edge falloff.

## Rust winit application

The native application in `rust-winit/` opens a native GL window and renders the shared farm glTF
through a Flight scene graph, with the web version's sky, lights, shadows, camera, and render-target
setup. Its round core drives both modes, the timer and settling phases, queued-piece cadence,
placement counts, and the different scoring rules. Its Flight physics world uses the web version's
stack profiles, collider geometry, materials, stability assistance, contact tracking, and
supported/settled-height rules. It now extracts the farm's hay, cow, and hen meshes, loads the horse
model, and renders placed bodies through the same per-part Flight instancing design as the web game.
Input aiming and the landing preview are the next visible porting slice.

The Rust modules deliberately mirror the TypeScript file and declaration names, translated to
Rust's standard `snake_case`, so the two implementations can be read side-by-side as an API
showcase rather than merely producing the same visuals.

```bash
cargo run --manifest-path rust-winit/Cargo.toml
```

Flight is pinned to an exact source revision while its Rust/TypeScript parity work is in progress.
See [`rust-winit/README.md`](rust-winit/README.md) for the current port status and checks.

## Model credits

- [Low Poly Farm](https://sketchfab.com/3d-models/low-poly-farm-879d61d8dfc048548ee380cace6f79d3)
  by [EdwinRC](https://sketchfab.com/Edwin3D), licensed under
  [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
- [Horse](https://sketchfab.com/3d-models/horse-e9f1f7d5684c4e8881eb24a1d57e71b3)
  by [SleepyPineapple](https://sketchfab.com/SleepyPineapple), licensed under
  [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).

## Music credit

- “The Mountain's Happy Song” by Elijah_K, sourced from
  [Free Music Archive](https://freemusicarchive.org/) and licensed under CC BY.
- “Farm Early Morning (Loop),” “Bodyfall Wood Double Thud,” “Flashlight Turn On 4,”
  “Fanfare Arpeggio Resolution,” and “Ta Da Brass Fanfare 1” sourced from
  [Free Sound Effects](https://free-sound-effects.net/).
- “HORSE3” whinny compilation sourced from Free Sound Effects and cued as four short reaction
  clips in the game.
