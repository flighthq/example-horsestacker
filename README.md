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

The native application will live in `rust-winit/`. Its source tree follows the same conceptual
boundaries as the TypeScript application while using idiomatic Rust ownership and winit lifecycle
handling.

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
