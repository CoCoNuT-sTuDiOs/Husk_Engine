# Husk decisions

Short record of what was decided and why. Paste into a new chat with ROADMAP.md.

## Product
- Husk is a desktop-only (Windows first) animation engine for animators and
  people with a little Blender. It is not a phone app.
- It makes short or full cartoons (fight scenes, walk cycles) in the YouTube
  and TikTok style, on a laptop that can carry it.
- The only output is an mp4 video. Cap: 90 s per scene; a whole movie has
  no cap.
- Benchmark for modelling and rigging: Blender 2.4 (armatures with vertex
  groups and envelopes, tree IK, mirrored bones, Edit Mode, Subdivision
  Surface and Mirror modifiers, preset armatures).
- Users can build characters from scratch in Husk or drop their own in.
- The Husk donut test (torus with icing and sprinkles, to video) and the
  character tests (mannequin, low-poly human, smooth body, textured
  character) decide when a stage is done.

## How we work
- Never design from muscle memory. Before any new system, look at how public
  projects do it (Blender manual, Bevy, other engines, the glTF spec), learn
  the known problems, and say what we take and why.
- Study Blender's docs and papers; never copy its GPL source.
- Small steps: Rust with tests, then Flutter UI, then run it live.
- The scene (Rust) is the source of truth; the renderer holds a GPU-side
  copy. Lists that must stay in step print "out of step" if they do not.

## Rendering
- Lights are objects: sun and point, inverse-square falloff, at most 8.
  With no lights, a light that follows the camera is used.
- The shader soft-clips values above 0.8 so lights add up without going flat.
- World: background colour, ambient strength and reflection strength.

## Animation
- The pose timeline (snapshot keys of bone rotations and root offset) stays.
- Everything else (position, rotation, scale, colour, roughness, metalness,
  camera field of view, light colour and intensity) lives in per-property
  channels inside each object: Step or Linear keys, rotations as quaternions
  with slerp (one segment never turns more than half a circle).
- Key records the pose and the object's channels at the same moment, so a
  repeat (set on the pose timeline) loops both.
- Seeking a time evaluates every object, then pushes the results to the
  renderer.

## Known problems
- Auto skin weights bleed between parts that are close but separate (fingers
  slightly apart, legs not joined but not far apart). Likely one cause.
  Research first; envelopes and rigid attach are candidate answers.
- Project file saves only the active object's skeleton, weights and
  animation; a scene save format is needed.
- The camera and light shapes are drawn like any mesh and would appear in an
  exported video; they need an editor-only flag before export.