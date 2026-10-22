// ---------- helpers for this video ----------
function playTri(x, y, s, color = COL.blue, a = 1) {
  A(a, () => {
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.moveTo(x - s * 0.35, y - s * 0.45);
    ctx.lineTo(x + s * 0.5, y);
    ctx.lineTo(x - s * 0.35, y + s * 0.45);
    ctx.closePath();
    ctx.fill();
  });
}
// a file card with a folded corner, top-left at x, y
function fileCard(x, y, w, h, o = {}) {
  const a = o.a ?? 1;
  const f = Math.min(36, w * 0.2);
  A(a, () => {
    ctx.fillStyle = "#000";
    stroke(o.color || COL.fg, o.lw || 3);
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x + w - f, y);
    ctx.lineTo(x + w, y + f);
    ctx.lineTo(x + w, y + h);
    ctx.lineTo(x, y + h);
    ctx.closePath();
    ctx.fill();
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x + w - f, y);
    ctx.lineTo(x + w - f, y + f);
    ctx.lineTo(x + w, y + f);
    ctx.stroke();
  });
  if (o.label && o.sub) {
    text(o.label, x + w / 2, y + h / 2 - 20, { size: o.size || 32, weight: 600, color: o.labelColor || COL.fg, alpha: a });
    text(o.sub, x + w / 2, y + h / 2 + 26, { size: o.subSize || 26, color: COL.dim, alpha: a });
  } else if (o.label) {
    text(o.label, x + w / 2, y + h / 2 + 6, { size: o.size || 32, weight: 600, color: o.labelColor || COL.fg, alpha: a });
  }
}
// speech waveform: bars between x0 and x1 around y, moving with t
function waveform(x0, x1, y, amp, t, color = COL.blue, a = 1, live = 1) {
  A(a, () => {
    ctx.fillStyle = color;
    const n = Math.floor((x1 - x0) / 14);
    for (let i = 0; i <= n; i++) {
      const env = 0.25 + 0.75 * Math.abs(Math.sin(i * 0.23 + 1.1) * Math.sin(i * 0.61 + t * 5 * live));
      const h = Math.max(4, amp * env);
      ctx.fillRect(x0 + i * 14 - 3, y - h / 2, 6, h);
    }
  });
}
// pseudo-random line width for fake text rows (deterministic)
const rowW = (i) => 0.55 + 0.42 * (((i * 37) % 17) / 17);

// ---------- 1. The wall of text ----------
SCENES.s1 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  person(190, 560, 140, COL.fg, p(0.2));
  text("You", 190, 690, { size: 30, alpha: p(0.2) });
  const pa = p(c("ask"), 0.5);
  pill("explain this PR", 330, 400, { size: 28, a: pa });
  arrow(270, 560, 396, 560, { p: pa, hs: 16, lw: 3 });
  box(404, 490, 230, 140, { p: pa, label: "Agent", size: 38 });

  // phase one: the wall
  A(1 - p(c("what"), 0.6), () => {
    const pw = p(c("wall"), 0.4);
    arrow(640, 560, 744, 560, { p: pw, hs: 16, lw: 3 });
    box(760, 160, 960, 760, { p: pw, color: COL.dim, lw: 2, r: 16 });
    const skimY = lerp(180, 910, clamp((t - c("skim")) / 1.4));
    const missA = p(c("miss"), 0.4);
    for (let i = 0; i < 20; i++) {
      const y = 200 + i * 34;
      const short = i >= 11 && i <= 13;
      const w = (short ? 380 : 860) * rowW(i);
      const a = P(t, c("wall") + 0.1 + i * 0.05, 0.2);
      const skimmed = t > c("skim") && y < skimY;
      const color = i === 12 && missA > 0.05 ? COL.amber : skimmed ? "#333" : "#8a8a8a";
      A(a, () => {
        ctx.fillStyle = color;
        ctx.fillRect(810, y - 7, w, 14);
      });
    }
    if (t > c("skim") && t < c("skim") + 1.5) {
      A(0.9, () => {
        ctx.fillStyle = COL.blue;
        ctx.fillRect(780, skimY - 2, 920, 4);
      });
    }
    pill("the part that mattered", 1450, 608, { size: 26, color: COL.amber, a: missA });
  });

  // phase two: a video instead
  const pv = p(c("what") + 0.3, 0.6);
  A(pv, () => {
    arrow(640, 560, 764, 560, { p: 1, hs: 16, lw: 3, color: COL.blue });
    const pf = P(t, c("video"), 0.7);
    box(780, 300, 900, 506, { p: pf, color: COL.blue, lw: 4, r: 18 });
    const pi = P(t, c("video") + 0.4, 0.6);
    A(pi, () => {
      stroke(COL.blue, 4);
      ctx.beginPath();
      ctx.arc(1230, 500, 90, 0, Math.PI * 2);
      ctx.stroke();
    });
    playTri(1240, 500, 96, COL.blue, pi);
    box(1000, 660, 150, 80, { p: P(t, c("video") + 0.7, 0.5), color: COL.dim, lw: 2 });
    arrow(1158, 700, 1302, 700, { p: P(t, c("video") + 0.9, 0.5), color: COL.dim, lw: 3, hs: 14 });
    box(1310, 660, 150, 80, { p: P(t, c("video") + 1.1, 0.5), color: COL.dim, lw: 2 });
    text("a video instead?", 1230, 880, { size: 46, weight: 600, alpha: P(t, c("video") + 0.2, 0.6) });
  });
};

// ---------- 2. Watch, don't read ----------
SCENES.s2 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  A(1 - p(c("meta"), 0.6), () => {
    const pv = p(c("voice"), 0.6);
    text("narrator", 330, 330, { size: 30, color: COL.dim, alpha: pv });
    waveform(140, 520, 430, 80, t, COL.blue, pv);
    const b = c("build");
    const words = [["idea", b], ["leads to", b + 0.8], ["result", b + 1.6]];
    const wx = [190, 330, 480];
    words.forEach(([w, tw], i) => pill(w, wx[i], 540, { size: 26, color: COL.blue, a: P(t, tw, 0.3) }));

    box(640, 200, 760, 430, { p: p(c("voice") - 0.4, 0.8), color: COL.dim, lw: 2, r: 18 });
    box(700, 355, 240, 120, { p: P(t, b, 0.5), label: "idea", size: 34 });
    arrow(950, 415, 1092, 415, { p: P(t, b + 0.8, 0.5), lw: 4 });
    box(1100, 355, 240, 120, { p: P(t, b + 1.6, 0.5), label: "result", size: 34 });
    pill("one idea per scene", 1020, 700, { size: 30, color: COL.blue, a: p(c("one"), 0.5) });

    const ps = p(c("shapes"), 0.5);
    check(380, 850, 44, COL.blue, ps, 7);
    text("shapes and arrows", 430, 850, { size: 38, weight: 600, align: "left", alpha: ps });

    const pn = p(c("noslides"), 0.5);
    box(1250, 740, 340, 190, { p: pn, color: COL.dim, lw: 2, r: 10 });
    A(pn, () => {
      for (let i = 0; i < 4; i++) {
        const y = 785 + i * 36;
        ctx.fillStyle = "#777";
        ctx.beginPath();
        ctx.arc(1290, y, 6, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillRect(1310, y - 6, 230 * rowW(i + 3), 12);
      }
    });
    cross(1420, 835, 150, COL.amber, P(t, c("noslides") + 0.4, 0.5), 7);
    text("bullet slides", 1420, 970, { size: 28, color: COL.amber, alpha: pn });
  });
  const pm = p(c("meta") + 0.3, 0.7);
  playTri(960, 430, 120, COL.blue, pm);
  text("This video was made with it.", 960, 600, { size: 60, weight: 700, alpha: pm });
};

// ---------- 3. Three files ----------
SCENES.s3 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  const files = [
    ["brief", "brief.md", "the facts"],
    ["script", "narration.json", "the script"],
    ["scenes", "scenes.js", "the pictures"],
  ];
  const xs = [140, 440, 740];
  files.forEach(([k, name, sub], i) => {
    const a = P(t, Math.min(c(k), c("three") + 0.3 + i * 0.25), 0.5);
    const lit = p(c(k), 0.4);
    fileCard(xs[i], 330, 260, 290, { a, color: lit > 0.5 ? COL.fg : COL.faint, lw: 3 });
    text(name, xs[i] + 130, 450, { size: 30, weight: 600, alpha: a * lerp(0.35, 1, lit) });
    text(sub, xs[i] + 130, 500, { size: 26, color: COL.dim, alpha: lit });
  });
  const pl = p(c("line"), 0.6);
  A(pl, () => {
    stroke(COL.blue, 3);
    ctx.beginPath();
    ctx.moveTo(140, 290);
    ctx.lineTo(140, 270);
    ctx.lineTo(1000, 270);
    ctx.lineTo(1000, 290);
    ctx.stroke();
  });
  text("the creative part", 570, 220, { size: 36, weight: 600, color: COL.blue, alpha: pl });
  text("written by your agent", 570, 690, { size: 28, color: COL.dim, alpha: pl });

  const pe = p(c("engine"), 0.7);
  A(pe, () => {
    stroke(COL.faint, 2);
    ctx.setLineDash([10, 10]);
    ctx.beginPath();
    ctx.moveTo(1100, 200);
    ctx.lineTo(1100, 900);
    ctx.stroke();
    ctx.setLineDash([]);
  });
  arrow(1012, 475, 1172, 475, { p: pe, lw: 4 });
  box(1180, 250, 600, 560, { p: pe, lw: 4, r: 22 });
  text("fixed engine", 1480, 310, { size: 40, weight: 700, alpha: pe });
  const parts = [["timing", "timing"], ["render", "rendering"], ["audio", "audio"], ["loud", "loudness"]];
  parts.forEach(([k, s], i) => pill(s, 1480, 420 + i * 95, { size: 32, color: COL.blue, a: p(c(k), 0.4) }));
  text("runs the same way every time", 1480, 870, { size: 28, color: COL.dim, alpha: P(t, c("loud") + 0.4, 0.5) });
};

// ---------- 4. Landing on the word ----------
SCENES.s4 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  // the script line with its cue
  const s1 = "and the picture ", tag = "{build}", s2 = "builds as each word is said";
  const m1 = measure(s1, 40), m2 = measure(s2, 40);
  const tw = measure(tag, 30, 600) + 30 * 1.3;
  const x0 = 960 - (m1 + tw + 14 + m2) / 2;
  const pl = p(0.2);
  text("narration.json", 960, 160, { size: 26, color: COL.dim, alpha: pl });
  text(s1, x0, 235, { size: 40, align: "left", alpha: pl });
  pill(tag, x0 + m1 + tw / 2, 235, { size: 30, color: COL.amber, a: p(c("cue"), 0.5) });
  text(s2, x0 + m1 + tw + 14, 235, { size: 40, align: "left", alpha: pl });

  // ElevenLabs
  const pe = p(c("eleven"), 0.6);
  arrow(x0 + 40, 272, 360, 412, { p: pe, lw: 3, hs: 14, bend: -40 });
  box(140, 420, 380, 120, { p: pe, label: "ElevenLabs", size: 36 });
  arrow(526, 480, 600, 520, { p: P(t, c("eleven") + 0.4, 0.5), lw: 3, hs: 14 });

  // character timings along a time axis
  const chars = "picture builds as each".split("");
  const ax0 = 640, ax1 = 1760, ay = 600;
  const step = (ax1 - ax0) / (chars.length - 1);
  const pa = P(t, c("eleven") + 0.6, 0.6);
  waveform(ax0, ax1, 520, 70, t, COL.dim, pa, t < c("land") ? 1 : 0);
  A(pa, () => {
    stroke(COL.faint, 2);
    ctx.beginPath();
    ctx.moveTo(ax0 - 20, ay);
    ctx.lineTo(ax1 + 20, ay);
    ctx.stroke();
  });
  const bi = chars.indexOf("b");
  const bx = ax0 + bi * step;
  chars.forEach((ch, i) => {
    const x = ax0 + i * step;
    const a = P(t, c("times") + i * 0.06, 0.25);
    const isB = i === bi && t > c("exact");
    A(a, () => {
      stroke(isB ? COL.blue : COL.dim, 2);
      ctx.beginPath();
      ctx.moveTo(x, ay - 10);
      ctx.lineTo(x, ay + 10);
      ctx.stroke();
    });
    if (ch !== " ") text(ch, x, ay + 40, { size: 30, weight: isB ? 700 : 500, color: isB ? COL.blue : COL.fg, alpha: a });
  });
  text("start time of every character", ax1, 690, { size: 28, color: COL.dim, align: "right", alpha: P(t, c("times") + 0.8, 0.5) });

  // the cue becomes a time
  const px = p(c("exact"), 0.5);
  A(px, () => {
    stroke(COL.blue, 3);
    ctx.beginPath();
    ctx.moveTo(bx, 450);
    ctx.lineTo(bx, 560);
    ctx.stroke();
  });
  pill("{build}", bx, 420, { size: 26, color: COL.blue, a: px });
  text("cue time", bx - 30, 740, { size: 28, color: COL.blue, align: "right", alpha: px });

  // a playhead sweeps, and the shape lands on the word
  const ls = c("land");
  if (t > ls) {
    const hx = lerp(ax0 - 20, bx, clamp((t - ls) / 1.0));
    A(1 - P(t, ls + 1.6, 0.4), () => {
      stroke(COL.fg, 3);
      ctx.beginPath();
      ctx.moveTo(hx, 460);
      ctx.lineTo(hx, 650);
      ctx.stroke();
    });
  }
  const pb = P(t, ls + 1.0, 0.5);
  arrow(bx, 700, bx, 788, { p: pb, lw: 3, hs: 14, color: COL.blue });
  box(bx - 150, 796, 300, 110, { p: pb, color: COL.blue, lw: 4, label: "shape", size: 34 });
  text("lands on the word", 1500, 851, { size: 40, weight: 600, color: COL.blue, alpha: P(t, ls + 1.3, 0.5) });
};

// ---------- 5. The render ----------
SCENES.s5 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  const ps = p(c("seek") + 0.1, 0.6);
  const k = 0.5 + 0.5 * Math.sin(t * 1.4);
  text("seek(t)", 300, 220, { size: 44, weight: 700, alpha: ps });
  A(ps, () => {
    stroke(COL.dim, 4);
    ctx.beginPath();
    ctx.moveTo(160, 330);
    ctx.lineTo(440, 330);
    ctx.stroke();
  });
  dot(lerp(160, 440, k), 330, 14, COL.blue, ps);
  text("t", lerp(160, 440, k), 290, { size: 26, color: COL.blue, alpha: ps });
  box(500, 255, 240, 150, { p: ps, color: COL.dim, lw: 2, r: 12 });
  dot(lerp(535, 705, k), 330, 14, COL.blue, ps);
  text("any frame, from one time value", 450, 460, { size: 26, color: COL.dim, alpha: P(t, c("seek") + 0.8, 0.5) });

  const pc = p(c("chrome"), 0.6);
  arrow(752, 330, 862, 330, { p: pc, lw: 3, hs: 14 });
  box(870, 265, 380, 130, { p: pc, label: "Headless Chrome", size: 34 });

  const pf = p(c("fps"), 0.4);
  arrow(1258, 330, 1330, 330, { p: pf, lw: 3, hs: 14 });
  for (let i = 0; i < 7; i++) {
    const a = P(t, c("fps") + 0.15 + i * 0.12, 0.25);
    box(1345 + i * 62, 290, 52, 80, { p: a, color: COL.fg, lw: 2, r: 6 });
    dot(1345 + i * 62 + 14 + i * 4, 330, 6, COL.blue, a);
  }
  text("30 frames per second", 1560, 420, { size: 26, color: COL.dim, alpha: P(t, c("fps") + 0.6, 0.5) });

  const pg = p(c("ffmpeg"), 0.6);
  arrow(1560, 450, 1560, 588, { p: pg, lw: 3, hs: 14 });
  box(1380, 595, 360, 120, { p: pg, label: "ffmpeg", size: 38 });
  waveform(160, 880, 655, 60, t, COL.blue, P(t, c("ffmpeg") + 0.3, 0.6));
  text("voice", 520, 725, { size: 28, color: COL.dim, alpha: P(t, c("ffmpeg") + 0.3, 0.6) });
  arrow(905, 655, 1372, 655, { p: P(t, c("ffmpeg") + 0.5, 0.6), lw: 3, hs: 14, color: COL.blue });

  const pm = p(c("mp4"), 0.6);
  arrow(1560, 721, 1560, 792, { p: pm, lw: 3, hs: 14 });
  fileCard(1440, 800, 240, 150, { a: pm, color: COL.blue, label: "final.mp4", labelColor: COL.blue });
  const specs = [["1920x1080", c("mp4") + 0.5], ["30 fps", c("mp4") + 0.8], ["H.264 + AAC", c("mp4") + 1.1], ["-14 LUFS", c("lufs")]];
  let x = 160;
  specs.forEach(([s, ts]) => {
    const w = measure(s, 28, 600) + 28 * 1.3;
    pill(s, x + w / 2, 875, { size: 28, color: COL.blue, a: P(t, ts, 0.4) });
    x += w + 18;
  });
};

// ---------- 6. Two checks ----------
SCENES.s6 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  const pk = p(c("check") - 0.2, 0.6);
  box(110, 200, 470, 380, { p: pk, color: COL.dim, lw: 2, r: 16 });
  text("check", 345, 250, { size: 36, weight: 700, alpha: pk });
  const items = [["clipped text", 0.6], ["missing cues", 1.4], ["page errors", 2.4]];
  items.forEach(([s, d], i) => {
    const y = 340 + i * 80;
    const a = P(t, c("check") + d, 0.4);
    check(170, y, 32, COL.blue, a, 5);
    text(s, 210, y, { size: 32, align: "left", alpha: a });
  });

  // stills grid
  for (let i = 0; i < 9; i++) {
    const x = 680 + (i % 3) * 170, y = 220 + Math.floor(i / 3) * 109;
    const a = P(t, c("stills") + i * 0.1, 0.3);
    box(x, y, 140, 79, { p: a, color: COL.fg, lw: 2, r: 6 });
    A(a, () => {
      stroke(COL.dim, 2);
      ctx.strokeRect(x + 20, y + 25, 36, 28);
      ctx.beginPath();
      ctx.moveTo(x + 62, y + 39);
      ctx.lineTo(x + 84, y + 39);
      ctx.stroke();
      ctx.strokeRect(x + 88, y + 25, 32, 28);
    });
  }
  text("stills from every scene", 925, 555, { size: 26, color: COL.dim, alpha: P(t, c("stills") + 0.8, 0.5) });

  const pc = p(c("codex"), 0.6);
  arrow(1180, 370, 1292, 370, { p: pc, lw: 3, hs: 14 });
  box(1300, 300, 420, 140, { p: pc, label: "Codex", sub: "a second model", size: 40, subSize: 26 });
  const pb = p(c("brief"), 0.5);
  fileCard(1420, 520, 180, 120, { a: pb, label: "brief.md", size: 28 });
  arrow(1510, 514, 1510, 448, { p: pb, lw: 3, hs: 14 });

  const pf = p(c("find"), 0.5);
  arrow(1340, 446, 1250, 596, { p: pf, lw: 3, hs: 14, color: COL.amber, bend: 30 });
  arrow(1010, 760, 800, 846, { p: P(t, c("fix") - 0.2, 0.5), lw: 3, hs: 14, color: COL.amber, bend: 30 });
  const solved = P(t, c("clean"), 0.5);
  text("findings", 1150, 620, { size: 28, color: COL.amber, alpha: pf * (1 - solved) });
  text("fixed", 1150, 620, { size: 28, color: COL.blue, alpha: pf * solved });
  pill("wrong fact", 1150, 680, { size: 26, color: solved > 0.5 ? COL.blue : COL.amber, a: pf });
  pill("unreadable label", 1150, 745, { size: 26, color: solved > 0.5 ? COL.blue : COL.amber, a: P(t, c("find") + 0.6, 0.5) });

  // fix and review loop
  const px = p(c("fix"), 0.5);
  pill("fix", 760, 880, { size: 32, a: px });
  pill("review", 1160, 880, { size: 32, a: px });
  const pl = p(c("loop"), 0.6);
  const top = arrow(800, 850, 1100, 850, { p: pl, bend: -60, lw: 3, hs: 14 });
  const bot = arrow(1100, 910, 800, 910, { p: pl, bend: -60, lw: 3, hs: 14 });
  text("up to 3 rounds", 950, 880, { size: 26, color: COL.dim, alpha: pl });
  if (t > c("loop") + 0.6 && t < c("clean") + 0.6) {
    const f = ((t - c("loop") - 0.6) / 1.1) % 1;
    const [dx, dy] = f < 0.5 ? top(f * 2) : bot((f - 0.5) * 2);
    dot(dx, dy, 9, COL.blue);
  }
  const pcl = p(c("clean"), 0.5);
  arrow(1240, 880, 1412, 880, { p: pcl, lw: 3, hs: 14, color: COL.blue });
  pill("CLEAN", 1520, 880, { size: 36, fill: COL.blue, color: COL.blue, textColor: "#000", a: pcl });
};

// ---------- 7. The cost ----------
SCENES.s7 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  const po = p(c("one"), 0.6);
  pill("one 2:46 video, made with the skill", 960, 175, { size: 28, a: po });
  const rowY = [320, 520, 700, 870];
  const names = [["tokens", "Agent"], ["dollars", "API price"], ["voice", "Voice"], ["render", "Render"]];
  names.forEach(([k, s], i) => text(s, 400, rowY[i], { size: 38, weight: 700, align: "right", alpha: p(c(k), 0.5) }));

  // tokens: a bar that fills, then splits into cache reads and the rest
  const pt = p(c("tokens"), 0.5);
  const fill = P(t, c("tokens") + 0.2, 1.4);
  const W0 = 440, BW = 1060;
  const split = P(t, c("cache"), 0.6);
  const readW = BW * 0.946;
  A(pt, () => {
    ctx.fillStyle = "#777";
    ctx.fillRect(W0, rowY[0] - 30, (split > 0 ? readW : BW) * fill, 60);
    if (split > 0) {
      ctx.globalAlpha *= split;
      ctx.fillStyle = COL.amber;
      ctx.fillRect(W0 + readW + 4, rowY[0] - 30, BW - readW - 4, 60);
    }
  });
  const shown = (4.44 * fill).toFixed(1);
  text(`${shown}M tokens`, W0 + BW + 30, rowY[0], { size: 36, weight: 700, align: "left", alpha: pt });
  text("95% cache reads, the cheap kind", W0 + readW / 2, rowY[0] + 62, { size: 28, color: COL.dim, alpha: split });
  text("writes and output", W0 + BW, rowY[0] + 62, { size: 24, color: COL.amber, align: "right", alpha: P(t, c("cache") + 0.5, 0.5) });

  // dollars
  const pd = p(c("dollars"), 0.5);
  text("$3.10", W0, rowY[1], { size: 64, weight: 700, color: COL.amber, align: "left", alpha: pd });
  text("at Claude Opus API list prices", W0 + 220, rowY[1] - 16, { size: 28, color: COL.dim, align: "left", alpha: P(t, c("dollars") + 0.4, 0.5) });
  text("on a subscription, it is plan usage instead", W0 + 220, rowY[1] + 24, { size: 26, color: COL.dim, align: "left", alpha: P(t, c("dollars") + 0.8, 0.5) });

  // voice
  const pv = p(c("voice"), 0.5);
  waveform(W0, W0 + 280, rowY[2], 50, t, COL.amber, pv);
  text("2,462 ElevenLabs characters", W0 + 320, rowY[2], { size: 34, align: "left", alpha: pv });

  // render
  const pr = p(c("render"), 0.5);
  check(W0 + 30, rowY[3], 48, COL.blue, P(t, c("render") + 0.2, 0.5), 8);
  text("0 tokens", W0 + 80, rowY[3], { size: 40, weight: 700, color: COL.blue, align: "left", alpha: pr });
  text("no model calls", W0 + 290, rowY[3], { size: 28, color: COL.dim, align: "left", alpha: P(t, c("render") + 0.4, 0.5) });
};

// ---------- 8. What to use it for ----------
SCENES.s8 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  const cards = [
    ["pr", "a pull request", "before you merge"],
    ["concept", "a concept", "you're learning"],
    ["plan", "a big plan", "before you say yes"],
  ];
  const xs = [120, 730, 1340];
  cards.forEach(([k, label, sub], i) => {
    const a = p(c(k), 0.6);
    box(xs[i], 230, 460, 460, { p: a, lw: 3, r: 22 });
    text(label, xs[i] + 230, 570, { size: 40, weight: 700, alpha: a });
    text(sub, xs[i] + 230, 625, { size: 28, color: COL.dim, alpha: a });
  });
  // pull request: branch and merge
  {
    const cx = 350, a = P(t, c("pr") + 0.3, 0.6);
    A(a, () => {
      stroke(COL.blue, 5);
      ctx.beginPath();
      ctx.moveTo(cx - 70, 300);
      ctx.lineTo(cx - 70, 470);
      ctx.moveTo(cx + 70, 340);
      ctx.quadraticCurveTo(cx + 70, 440, cx - 62, 455);
      ctx.stroke();
      [[cx - 70, 300], [cx - 70, 470], [cx + 70, 330]].forEach(([x, y]) => {
        ctx.fillStyle = "#000";
        ctx.beginPath();
        ctx.arc(x, y, 18, 0, Math.PI * 2);
        ctx.fill();
        ctx.stroke();
      });
    });
  }
  // concept: connected ideas
  {
    const cx = 960, a = P(t, c("concept") + 0.3, 0.6);
    const n = [[cx - 90, 330], [cx + 80, 300], [cx + 20, 400], [cx - 60, 470], [cx + 100, 460]];
    const e = [[0, 2], [1, 2], [2, 3], [2, 4], [0, 1]];
    A(a, () => {
      stroke(COL.blue, 3);
      e.forEach(([i, j], q) => {
        const f = clamp(P(t, c("concept") + 0.3 + q * 0.12, 0.4));
        ctx.beginPath();
        ctx.moveTo(n[i][0], n[i][1]);
        ctx.lineTo(lerp(n[i][0], n[j][0], f), lerp(n[i][1], n[j][1], f));
        ctx.stroke();
      });
      n.forEach(([x, y]) => {
        ctx.fillStyle = COL.blue;
        ctx.beginPath();
        ctx.arc(x, y, 13, 0, Math.PI * 2);
        ctx.fill();
      });
    });
  }
  // plan: steps with checks
  {
    const x0 = 1450, a = P(t, c("plan") + 0.3, 0.6);
    for (let i = 0; i < 3; i++) {
      const y = 320 + i * 70;
      A(a, () => {
        stroke(COL.fg, 3);
        ctx.strokeRect(x0, y - 18, 36, 36);
        ctx.fillStyle = "#8a8a8a";
        ctx.fillRect(x0 + 60, y - 7, 160 * rowW(i + 5), 14);
      });
      check(x0 + 18, y, 26, COL.blue, P(t, c("plan") + 0.7 + i * 0.35, 0.4), 5);
    }
  }
  // anything you'd skip as text
  const ps = p(c("skip"), 0.6);
  A(ps, () => {
    for (let i = 0; i < 5; i++) {
      ctx.fillStyle = "#666";
      ctx.fillRect(560, 790 + i * 26, 220 * rowW(i + 1), 10);
    }
  });
  arrow(810, 845, 1090, 845, { p: P(t, c("skip") + 0.3, 0.5), lw: 4, hs: 16, color: COL.blue });
  box(1120, 780, 230, 130, { p: P(t, c("skip") + 0.5, 0.5), color: COL.blue, lw: 3, r: 14 });
  playTri(1240, 845, 64, COL.blue, P(t, c("skip") + 0.7, 0.5));
  text("anything you'd skip as text", 960, 965, { size: 34, weight: 600, alpha: P(t, c("skip") + 0.6, 0.5) });
};

// ---------- 9. Try it ----------
SCENES.s9 = (t, c) => {
  const p = (s, d = 0.6) => P(t, s, d);
  text("explainer-video", 960, 230, { size: 76, weight: 700, alpha: p(0.2) });
  const po = p(c("oss"), 0.5);
  pill("open source", 960, 335, { size: 32, color: COL.blue, a: po });
  text("github.com/AojdevStudio/agentic-utilities", 960, 420, { size: 36, alpha: P(t, c("oss") + 0.4, 0.6) });
  const req = ["Bun", "ffmpeg", "ElevenLabs key"];
  const ws = req.map((s) => measure(s, 30, 600) + 30 * 1.3);
  let x = 960 - (ws.reduce((a, b) => a + b, 0) + 24 * 2) / 2;
  text("you need", 960, 515, { size: 26, color: COL.dim, alpha: p(c("need"), 0.5) });
  req.forEach((s, i) => {
    pill(s, x + ws[i] / 2, 580, { size: 30, a: P(t, c("need") + 0.3 + i * 0.4, 0.4) });
    x += ws[i] + 24;
  });
  const pa = p(c("ask"), 0.5);
  person(660, 760, 110, COL.fg, pa);
  bubble(740, 700, 540, 76, COL.fg, pa);
  text("make me an explainer video", 1010, 738, { size: 32, alpha: pa });
  const pw = p(c("watch"), 0.6);
  playTri(640, 900, 60, COL.blue, pw);
  text("Watch, instead of read.", 1000, 900, { size: 56, weight: 700, color: COL.blue, alpha: pw });
};
