# Writing scenes.js

`scenes.js` is plain browser JavaScript, loaded after `engine/lib.js`. It assigns one function per narration scene. Everything below is a global from `lib.js`. Read `engine/lib.js` for the exact code of any primitive.

```js
SCENES.s3 = (t, c, dur) => {
  const p = (s, d = 0.6) => P(t, s, d);   // local progress helper, used in every example scene
  box(360, 540, 340, 120, { p: p(c("save")), label: "save intent", size: 38 });
};
```

- `t` is seconds since the scene started. `dur` is the scene length.
- `c("name")` is the time the narrator says the word after `{name}`. A cue missing from `narration.json` throws, and `check` reports it.
- The engine draws the black background, the scene header (number plus title), the fade in and out at scene edges, and the progress bar. The scene draws only its own content.

## Layout

- The canvas is 1920x1080. The header sits at y 70 on the left. Keep content between y 130 and y 1000, with x 80 or more from each edge.
- Coordinates are centers for `text` and `pill`, and top-left corners for `box`.
- Type sizes: 40 to 54 for the main claim, 30 to 38 for box labels, 24 to 28 for small labels. Nothing under 22.
- Colors are in `COL`: `fg` white, `dim` gray, `faint` dark gray, `blue` and `amber` accents. Blue means the good path, the answer, or the thing that works. Amber means the cost, the risk, or the thing being replaced. Keep it to those two accents.

## Timing

| Helper | Returns |
|---|---|
| `P(t, start, d = 0.6)` | Eased progress 0 to 1 from `start` over `d` seconds. Feed it to `p`, `a`, or `alpha`. |
| `keyed(t, [[time, value], ...], d = 0.7)` | A value that eases from one keyframe to the next. Use it to move an object step by step. |
| `lerp(a, b, k)`, `clamp(x, lo, hi)`, `easeIO(k)` | Plain math. |
| `A(alpha, () => { ... })` | Draws the callback at that opacity. Use it to fade a group or to crossfade phases. |

Phase change inside one scene: wrap phase one in `A(1 - P(t, c("next"), 0.6), ...)` and phase two in `A(P(t, c("next") + 0.3, 0.6), ...)`. Add `"next"` to the scene's `stills` so the review sees the change.

## Primitives

| Call | Options and notes |
|---|---|
| `text(s, x, y, o)` | `size`, `weight`, `color`, `align` (default center), `alpha`. `\n` splits lines. `check` records every string. |
| `box(x, y, w, h, o)` | `p` draws the outline, then fades in the label. `label`, `sub`, `size`, `subSize`, `subGap`, `weight`, `labelColor`, `subColor`, `color`, `fill` with `fillAlpha`, `lw`, `r`, `dash`, `glow`. |
| `arrow(x1, y1, x2, y2, o)` | `p` grows it, `bend` curves it, `both` adds a back head, `head: false`, `color`, `lw`, `hs` (head size), `dash`, `alpha`. Returns `pt(k)`, a point at 0 to 1 along the curve. |
| `dot(x, y, r, color, a)` | Pair it with `arrow`'s `pt` for a dot that travels along a connection. |
| `pill(label, x, y, o)` | A rounded tag: `size`, `a`, `color`, `fill`, `textColor`. Returns its width. |
| `measure(s, size, weight)` | Text width, for laying out rows of pills. |
| `check`, `cross(x, y, s, color, p, lw)` | Strokes that draw in with `p`. |
| `person`, `phone`, `tablet`, `laptop(x, y, s, color, a)` | Line icons. |
| `clock(x, y, r, angle, color, a)`, `bolt`, `gate`, `pin`, `retry`, `bubble`, `decisionCard` | More icons. See `lib.js` for the arguments. |

A new icon goes in the project's `scenes.js`, built from `ctx` calls inside `A(...)`. Move it to `lib.js` only when a second video needs it, and only if the worked example still renders byte for byte afterward.

## What makes it read like 3Blue1Brown

- Build the picture in front of the viewer. Each element appears on the cue for the word that names it.
- Show flow as motion: a dot travels along an arrow, a token moves between steps, a counter fills up.
- Show change as change: a box dims and gets a cross, a value counts up, two options split at a fork.
- End each scene on a full picture that makes sense with the sound off. That end state is the still the review judges.
- Use a few words on screen. A label names a thing, and the narrator explains it.
