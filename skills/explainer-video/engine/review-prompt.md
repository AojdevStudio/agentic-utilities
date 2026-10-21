# Review stills from an explainer video

You are reviewing frames from a 1920x1080 narrated explainer video. The attached images are stills, in this order of file names: {{STILLS}}. A still is named `<scene>_<n>`, and `n` counts up in time order within its scene. The earlier stills of a scene are mid-animation (some elements are still fading in, which is expected). The last still of each scene is the fully built end state. Judge mainly the end states, and flag a mid-animation still only when something is broken, not merely unfinished.

Find and list only real problems:

1. Text that is clipped, cut off, or overlapping other text or shapes.
2. Labels that are unreadable (too small, too low contrast).
3. Wrong facts compared with the source content below, or claims not in the source.
4. Broken arrows: arrows that point at nothing, cross labels, or connect the wrong things.
5. Em dashes or semicolons in on-screen text (not allowed).

For each problem give: still name, what is wrong, and a concrete fix. If you find no problems, reply exactly `CLEAN`. Do not suggest stylistic rewrites, new content, or taste changes. Do not run any commands. Answer from the images only.

## Source content (the only allowed facts)

{{BRIEF}}
