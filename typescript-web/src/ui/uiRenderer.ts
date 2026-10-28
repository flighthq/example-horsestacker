// Getting the 2D UI onto the same canvas as the 3D scene. src/ui/UiRenderer.hx in the Haxe
// sibling — though that one still carries the hand-rolled GLSL compositor this no longer
// needs.
//
// One canvas. The UI is drawn into its own offscreen render target using a SECOND render
// state over the SAME GL context, then presented onto the finished 3D frame as a single
// blended quad. All public SDK: beginGlRenderPass/endGlRenderPass own the target, its clear
// and the save/restore of enclosing state; presentGlRenderTarget lays the finished target
// over the canvas; destroyGlTextureRenderTarget hands its GL objects back.
//
// present reads the target's DECLARED colour space, so an 'srgb' target — which a 2D scene
// is, because Flight's 2D tower composites in the encoded domain by policy
// (render/SCENE2D_WORKING_COLOR_SPACE) — is copied straight through with premultiplied
// blending rather than gamma-encoded a second time. That seam is why the alternatives failed,
// and it is worth recording:
//   - rendering 2D straight at the canvas after the pipeline presents draws nothing;
//   - rendering it into the pipeline's own target, nested pass or not, destroys the 3D;
//   - handing the UI target to the pipeline as a DestinationOver CompositeEffect backdrop
//     works and is public API too — but the pipeline composites in LINEAR, and while our own
//     colours can be converted on the way in, sRGB texture content cannot, so the emoji tally
//     came out washed.
//
// Two more things make this work, each of which broke an earlier attempt:
//   - the second state comes from createGlRenderState with the 2D preset, so its renderer/
//     material registrations and per-frame batch state are its own. Sharing one state with the 3D
//     pipeline corrupts the 3D — props lose their textures.
//   - present runs immediately after the 2D pass closes, which is where renderGlScene2D has
//     already left blending on and depth/cull off — the state the blend needs. It also owns
//     its own VAO and unbinds what it sampled, so nothing leaks into the next frame's 3D.
import type { DisplayObject, GlRenderState, GlTextureRenderTarget } from '@flighthq/sdk';
import {
  beginGlRenderPass,
  createGlRenderState,
  createGlTextureRenderTarget,
  createMatrix,
  destroyGlTextureRenderTarget,
  endGlRenderPass,
  glScene2DRenderPreset,
  prepareScene2DRender,
  presentGlRenderTarget,
  renderGlScene2D,
} from '@flighthq/sdk';
import { webHostCanvas, webHostImage } from '@flighthq/host-web';

export interface UiRenderer {
  /** The offscreen state the UI's nodes are prepared and drawn against. */
  state: GlRenderState;
  /** Reallocate the target for a new size. Call before the next render. */
  resize: (width: number, height: number, pixelRatio: number) => void;
  /** Draw `root` into the target and lay it over whatever is already on the canvas. */
  render: (root: DisplayObject) => void;
}

export function createUiRenderer(
  screenState: GlRenderState,
  pixelRatio: number,
): UiRenderer {
  const state: GlRenderState = createGlRenderState(screenState.gl, {
    ...glScene2DRenderPreset,
    canvasHost: webHostCanvas,
    imageHost: webHostImage,
    pixelRatio,
  });
  let target: GlTextureRenderTarget | null = null;
  let deviceTransform = createMatrix(pixelRatio, 0, 0, pixelRatio, 0, 0);

  return {
    state,

    resize(width, height, nextPixelRatio) {
      state.pixelRatio = nextPixelRatio;
      deviceTransform = createMatrix(nextPixelRatio, 0, 0, nextPixelRatio, 0, 0);
      // The target matches the drawing buffer exactly, so UI text lands on whole device
      // pixels and stays crisp rather than being resampled by the composite. Resizing
      // allocates a new one, so the old one has to go back or every resize leaks a
      // screen-sized texture.
      if (target !== null) destroyGlTextureRenderTarget(target);
      target = createGlTextureRenderTarget(state, {
        width: Math.max(1, Math.round(width * nextPixelRatio)),
        height: Math.max(1, Math.round(height * nextPixelRatio)),
        depth: 'none',
        colorSpace: 'srgb',
      });
    },

    render(root) {
      const surface = target;
      if (surface === null) return;
      if (!prepareScene2DRender(state, root)) return;

      // The draw call receives the logical-to-device transform explicitly. The render target
      // is device-sized, while layout and hit-testing remain in logical pixels.
      const pass = beginGlRenderPass(state, surface, { color: [0, 0, 0, 0] });
      renderGlScene2D(pass, root, deviceTransform);
      endGlRenderPass(pass);

      // Lay it over that frame. See the colour-space note at the top of the file.
      presentGlRenderTarget(state, surface);
    },
  };
}
