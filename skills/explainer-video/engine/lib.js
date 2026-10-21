// Engine side of the deterministic scene program. seek(T) draws the frame at global time T (seconds).
// A project's scenes.js loads after this file and fills SCENES[id] = (t, c, dur) => { ... }.
// Every scene reads its cue times from timeline.json, so motion lands on the spoken word.
const W = 1920, H = 1080;
const ctx = document.getElementById("c").getContext("2d");
const COL = { fg: "#ffffff", dim: "#9a9a9a", faint: "#555555", blue: "#58C4DD", amber: "#F5B83D" };
const FONT = 'Inter, "Nimbus Sans", sans-serif';
let TL = null;

// ---------- math ----------
const clamp = (x, a = 0, b = 1) => Math.max(a, Math.min(b, x));
const easeIO = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
const lerp = (a, b, t) => a + (b - a) * t;
const P = (t, s, d = 0.6) => easeIO(clamp((t - s) / d));
// keyframed value: frames = [[time, value], ...], each step eases over `d` seconds
function keyed(t, frames, d = 0.7) {
  let v = frames[0][1];
  for (let i = 1; i < frames.length; i++) v = lerp(v, frames[i][1], P(t, frames[i][0], d));
  return v;
}

// ---------- drawing primitives ----------
function A(a, fn) {
  if (a <= 0.001) return;
  ctx.save();
  ctx.globalAlpha *= Math.min(1, a);
  fn();
  ctx.restore();
}
const font = (size, weight = 500) => `${weight} ${size}px ${FONT}`;
function text(s, x, y, o = {}) {
  const size = o.size || 40;
  ctx.save();
  if (o.alpha != null) ctx.globalAlpha *= clamp(o.alpha);
  ctx.font = font(size, o.weight || 500);
  ctx.fillStyle = o.color || COL.fg;
  ctx.textAlign = o.align || "center";
  ctx.textBaseline = "middle";
  const lines = String(s).split("\n");
  const lh = size * 1.25;
  const y0 = y - ((lines.length - 1) * lh) / 2;
  lines.forEach((l, i) => {
    if (window.TEXT_LOG) recordText(l, x, y0 + i * lh);
    ctx.fillText(l, x, y0 + i * lh);
  });
  ctx.restore();
}
// Opt-in recorder for render.ts check: set window.TEXT_LOG = [] before seek().
// measureText and getTransform only read state, so recording never changes a pixel.
function recordText(s, x, y) {
  const m = ctx.measureText(s);
  const tf = ctx.getTransform();
  const corners = [
    [x - m.actualBoundingBoxLeft, y - m.actualBoundingBoxAscent],
    [x + m.actualBoundingBoxRight, y - m.actualBoundingBoxAscent],
    [x - m.actualBoundingBoxLeft, y + m.actualBoundingBoxDescent],
    [x + m.actualBoundingBoxRight, y + m.actualBoundingBoxDescent],
  ].map(([px, py]) => [tf.a * px + tf.c * py + tf.e, tf.b * px + tf.d * py + tf.f]);
  const xs = corners.map((p) => p[0]), ys = corners.map((p) => p[1]);
  window.TEXT_LOG.push({
    s,
    x0: Math.min(...xs), y0: Math.min(...ys), x1: Math.max(...xs), y1: Math.max(...ys),
    alpha: ctx.globalAlpha,
  });
}
function measure(s, size, weight = 500) {
  ctx.save();
  ctx.font = font(size, weight);
  const w = ctx.measureText(s).width;
  ctx.restore();
  return w;
}
function rr(x, y, w, h, r) {
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, r);
}
// box that draws its outline progressively, then fades in its label
function box(x, y, w, h, o = {}) {
  const p = o.p ?? 1;
  if (p <= 0.001) return;
  const r = o.r ?? 18;
  const color = o.color || COL.fg;
  ctx.save();
  if (o.fill) {
    ctx.save();
    ctx.globalAlpha *= clamp((p - 0.3) / 0.7) * (o.fillAlpha ?? 1);
    rr(x, y, w, h, r);
    ctx.fillStyle = o.fill;
    ctx.fill();
    ctx.restore();
  }
  if (o.glow) {
    ctx.shadowColor = o.glowColor || color;
    ctx.shadowBlur = o.glow;
  }
  ctx.strokeStyle = color;
  ctx.lineWidth = o.lw || 3;
  if (o.dash) {
    ctx.globalAlpha *= p;
    ctx.setLineDash(o.dash);
  } else {
    const per = 2 * (w + h);
    ctx.setLineDash([per * p, per]);
  }
  rr(x, y, w, h, r);
  ctx.stroke();
  ctx.restore();
  const la = clamp((p - 0.4) / 0.6);
  const cx = x + w / 2;
  const cy = y + h / 2;
  if (o.label && o.sub) {
    text(o.label, cx, cy - (o.subGap ?? 22), { size: o.size || 40, weight: o.weight || 600, color: o.labelColor || COL.fg, alpha: la });
    text(o.sub, cx, cy + (o.subGap ?? 22) + 4, { size: o.subSize || 28, color: o.subColor || COL.dim, alpha: la });
  } else if (o.label) {
    text(o.label, cx, cy, { size: o.size || 40, weight: o.weight || 600, color: o.labelColor || COL.fg, alpha: la });
  }
}
// arrow along a quadratic curve; returns the curve sampler for travelling dots
function arrow(x1, y1, x2, y2, o = {}) {
  const p = o.p ?? 1;
  const bend = o.bend || 0;
  const mx = (x1 + x2) / 2, my = (y1 + y2) / 2;
  const dx = x2 - x1, dy = y2 - y1;
  const L = Math.hypot(dx, dy) || 1;
  const cx = mx - (dy / L) * bend, cy = my + (dx / L) * bend;
  const pt = (t) => [
    (1 - t) * (1 - t) * x1 + 2 * (1 - t) * t * cx + t * t * x2,
    (1 - t) * (1 - t) * y1 + 2 * (1 - t) * t * cy + t * t * y2,
  ];
  if (p <= 0.001) return pt;
  const color = o.color || COL.fg;
  const hs = o.hs || 18;
  ctx.save();
  if (o.alpha != null) ctx.globalAlpha *= clamp(o.alpha);
  ctx.strokeStyle = color;
  ctx.fillStyle = color;
  ctx.lineWidth = o.lw || 4;
  ctx.lineCap = "round";
  if (o.dash) ctx.setLineDash(o.dash);
  const headT = o.head === false ? 0 : Math.min(0.3, (hs * 0.8) / L);
  const t0 = o.both ? headT : 0;
  const tEnd = Math.max(t0, p - (p > 0.05 ? headT : 0));
  const n = 48;
  ctx.beginPath();
  for (let i = 0; i <= n; i++) {
    const [px, py] = pt(t0 + ((tEnd - t0) * i) / n);
    i ? ctx.lineTo(px, py) : ctx.moveTo(px, py);
  }
  ctx.stroke();
  ctx.setLineDash([]);
  const head = (ex, ey, bx, by) => {
    const ang = Math.atan2(ey - by, ex - bx);
    ctx.beginPath();
    ctx.moveTo(ex, ey);
    ctx.lineTo(ex - hs * Math.cos(ang - 0.42), ey - hs * Math.sin(ang - 0.42));
    ctx.lineTo(ex - hs * Math.cos(ang + 0.42), ey - hs * Math.sin(ang + 0.42));
    ctx.closePath();
    ctx.fill();
  };
  if (o.head !== false && p > 0.05) {
    const [ex, ey] = pt(p);
    const [bx, by] = pt(Math.max(0, p - 0.03));
    head(ex, ey, bx, by);
  }
  if (o.both && p > 0.95) {
    const [sx, sy] = pt(0);
    const [nx, ny] = pt(0.03);
    head(sx, sy, nx, ny);
  }
  ctx.restore();
  return pt;
}
function dot(x, y, r, color, a = 1) {
  A(a, () => {
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(x, y, r, 0, Math.PI * 2);
    ctx.fill();
  });
}
function pill(label, x, y, o = {}) {
  const size = o.size || 26;
  const w = measure(label, size, o.weight || 600) + size * 1.3;
  const h = size * 1.8;
  A(o.a ?? 1, () => {
    rr(x - w / 2, y - h / 2, w, h, h / 2);
    if (o.fill) {
      ctx.fillStyle = o.fill;
      ctx.fill();
    } else {
      ctx.fillStyle = "#000";
      ctx.fill();
    }
    ctx.strokeStyle = o.color || COL.fg;
    ctx.lineWidth = 2.5;
    ctx.stroke();
    text(label, x, y + 1, { size, weight: o.weight || 600, color: o.textColor || o.color || COL.fg });
  });
  return w;
}
function stroke(color, lw = 4) {
  ctx.strokeStyle = color;
  ctx.lineWidth = lw;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
}

// ---------- icons ----------
function person(x, y, s, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 4);
    ctx.beginPath();
    ctx.arc(x, y - s * 0.28, s * 0.2, 0, Math.PI * 2);
    ctx.stroke();
    ctx.beginPath();
    ctx.arc(x, y + s * 0.44, s * 0.38, Math.PI, 0);
    ctx.stroke();
  });
}
function phone(x, y, s, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 3.5);
    rr(x - s * 0.28, y - s * 0.5, s * 0.56, s, s * 0.09);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x - s * 0.08, y + s * 0.38);
    ctx.lineTo(x + s * 0.08, y + s * 0.38);
    ctx.stroke();
  });
}
function tablet(x, y, s, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 3.5);
    rr(x - s * 0.4, y - s * 0.5, s * 0.8, s, s * 0.08);
    ctx.stroke();
    dot(x, y + s * 0.4, 2.5, color);
  });
}
function laptop(x, y, s, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 3.5);
    rr(x - s * 0.45, y - s * 0.38, s * 0.9, s * 0.6, s * 0.05);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x - s * 0.6, y + s * 0.32);
    ctx.lineTo(x + s * 0.6, y + s * 0.32);
    ctx.stroke();
  });
}
function clock(x, y, r, ang, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 3);
    ctx.beginPath();
    ctx.arc(x, y, r, 0, Math.PI * 2);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x, y);
    ctx.lineTo(x + Math.sin(ang) * r * 0.78, y - Math.cos(ang) * r * 0.78);
    ctx.moveTo(x, y);
    ctx.lineTo(x + Math.sin(ang / 12) * r * 0.5, y - Math.cos(ang / 12) * r * 0.5);
    ctx.stroke();
  });
}
function check(x, y, s, color = COL.blue, p = 1, lw = 6) {
  if (p <= 0) return;
  const pts = [[-0.5, 0], [-0.15, 0.35], [0.5, -0.4]];
  const seg1 = Math.hypot(0.35, 0.35), seg2 = Math.hypot(0.65, 0.75);
  let left = p * (seg1 + seg2);
  ctx.save();
  stroke(color, lw);
  ctx.beginPath();
  ctx.moveTo(x + pts[0][0] * s, y + pts[0][1] * s);
  const f1 = Math.min(1, left / seg1);
  ctx.lineTo(x + lerp(pts[0][0], pts[1][0], f1) * s, y + lerp(pts[0][1], pts[1][1], f1) * s);
  left -= seg1;
  if (left > 0) {
    const f2 = Math.min(1, left / seg2);
    ctx.lineTo(x + lerp(pts[1][0], pts[2][0], f2) * s, y + lerp(pts[1][1], pts[2][1], f2) * s);
  }
  ctx.stroke();
  ctx.restore();
}
function cross(x, y, s, color = COL.amber, p = 1, lw = 6) {
  if (p <= 0) return;
  ctx.save();
  stroke(color, lw);
  const a = clamp(p * 2), b = clamp(p * 2 - 1);
  ctx.beginPath();
  ctx.moveTo(x - s / 2, y - s / 2);
  ctx.lineTo(x - s / 2 + s * a, y - s / 2 + s * a);
  if (b > 0) {
    ctx.moveTo(x + s / 2, y - s / 2);
    ctx.lineTo(x + s / 2 - s * b, y - s / 2 + s * b);
  }
  ctx.stroke();
  ctx.restore();
}
function bolt(x, y, s, color = COL.amber, a = 1) {
  A(a, () => {
    const pts = [[0.12, -0.5], [-0.3, 0.06], [0.0, 0.06], [-0.12, 0.5], [0.3, -0.06], [0.0, -0.06]];
    ctx.beginPath();
    pts.forEach(([px, py], i) => (i ? ctx.lineTo(x + px * s, y + py * s) : ctx.moveTo(x + px * s, y + py * s)));
    ctx.closePath();
    ctx.fillStyle = color;
    ctx.fill();
  });
}
function gate(x, y, s, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 4);
    ctx.beginPath();
    ctx.moveTo(x - s * 0.4, y + s * 0.4);
    ctx.lineTo(x - s * 0.4, y - s * 0.4);
    ctx.moveTo(x + s * 0.4, y + s * 0.4);
    ctx.lineTo(x + s * 0.4, y - s * 0.4);
    ctx.moveTo(x - s * 0.4, y - s * 0.12);
    ctx.lineTo(x + s * 0.4, y - s * 0.12);
    ctx.moveTo(x - s * 0.4, y + s * 0.12);
    ctx.lineTo(x + s * 0.4, y + s * 0.12);
    ctx.stroke();
  });
}
function bubble(x, y, w, h, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 3);
    rr(x, y, w, h, h / 2.4);
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(x + 18, y + h);
    ctx.lineTo(x + 12, y + h + 12);
    ctx.lineTo(x + 32, y + h);
    ctx.stroke();
  });
}
function decisionCard(x, y, w, h, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 3);
    rr(x, y, w, h, 10);
    ctx.stroke();
    ctx.fillStyle = color;
    ctx.fillRect(x + w * 0.12, y + h * 0.22, w * 0.6, 4);
    rr(x + w * 0.12, y + h * 0.55, w * 0.3, h * 0.25, 6);
    ctx.fill();
    rr(x + w * 0.52, y + h * 0.55, w * 0.3, h * 0.25, 6);
    ctx.stroke();
  });
}
function retry(x, y, r, color = COL.fg, a = 1) {
  A(a, () => {
    stroke(color, 4);
    ctx.beginPath();
    ctx.arc(x, y, r, -Math.PI * 0.2, Math.PI * 1.45);
    ctx.stroke();
    const ang = -Math.PI * 0.2;
    const ex = x + Math.cos(ang) * r, ey = y + Math.sin(ang) * r;
    ctx.beginPath();
    ctx.moveTo(ex + 10, ey - 6);
    ctx.lineTo(ex, ey);
    ctx.lineTo(ex - 6, ey - 12);
    ctx.stroke();
  });
}
function pin(x, y, s, color = COL.amber, a = 1) {
  A(a, () => {
    ctx.fillStyle = color;
    ctx.beginPath();
    ctx.arc(x, y - s * 0.25, s * 0.22, 0, Math.PI * 2);
    ctx.fill();
    stroke(color, 4);
    ctx.beginPath();
    ctx.moveTo(x, y - s * 0.05);
    ctx.lineTo(x, y + s * 0.5);
    ctx.stroke();
  });
}

// ---------- header and global frame ----------
// closing scenes (narration.json `closing: true`) get no number
function header(i, title, closing) {
  const label = closing ? "" : String(i + 1);
  if (label) text(label, 80, 70, { size: 28, weight: 700, color: COL.blue, align: "left" });
  text(title, label ? 80 + measure(label, 28, 700) + 16 : 80, 70, { size: 28, weight: 500, color: COL.dim, align: "left" });
}

const SCENES = {};

// ---------- frame ----------
window.seek = function seek(T) {
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.globalAlpha = 1;
  ctx.fillStyle = "#000";
  ctx.fillRect(0, 0, W, H);
  const scenes = TL.scenes;
  const sc = scenes.find((s) => T >= s.start && T < s.start + s.dur) || scenes[scenes.length - 1];
  const idx = scenes.indexOf(sc);
  const t = T - sc.start;
  const fade = Math.min(clamp(t / 0.35), clamp((sc.dur - t) / 0.35));
  const c = (name) => {
    const v = sc.cues[name];
    if (v == null) throw new Error(`missing cue ${sc.id}.${name}`);
    return v + sc.lead;
  };
  A(fade, () => {
    header(idx, sc.title, sc.closing);
    SCENES[sc.id](t, c, sc.dur);
  });
  ctx.fillStyle = "#1a1a1a";
  ctx.fillRect(0, H - 6, W, 6);
  ctx.fillStyle = "#555";
  ctx.fillRect(0, H - 6, (W * T) / TL.total, 6);
};

// ---------- boot ----------
// index.html calls this after the project's scenes.js. It sets READY, or BOOT_ERROR for render.ts to report.
window.boot = async function boot() {
  try {
    TL = await (await fetch("timeline.json")).json();
    await Promise.all(["500", "600", "700"].map((w) => document.fonts.load(`${w} 40px Inter`))).catch((e) => {
      throw new Error(`Inter failed to load: ${e instanceof Error ? e.message : e}`);
    });
    // fonts.check() is true when a family has no face at all, so look at the Inter face itself.
    const inter = [...document.fonts].filter((f) => f.family.replace(/"/g, "") === "Inter");
    if (!inter.length || inter.some((f) => f.status !== "loaded") || !document.fonts.check("700 40px Inter")) {
      throw new Error("Inter failed to load");
    }
    const ids = TL.scenes.map((s) => s.id);
    const unbuilt = ids.filter((id) => typeof SCENES[id] !== "function");
    if (unbuilt.length) throw new Error(`narration scene(s) with no SCENES entry in scenes.js: ${unbuilt.join(", ")}`);
    const extra = Object.keys(SCENES).filter((id) => !ids.includes(id));
    if (extra.length) throw new Error(`SCENES entries in scenes.js with no narration scene: ${extra.join(", ")}`);
    seek(0);
    window.READY = true;
  } catch (e) {
    window.BOOT_ERROR = e instanceof Error ? e.message : String(e);
  }
};
