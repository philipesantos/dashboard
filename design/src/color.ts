// Color math for the contrast check: oklch() to sRGB, compositing translucent layers, and the
// WCAG 2.2 contrast ratio. No dependency (specs/007-host-visual-system/research.md, R2).

/** An sRGB color with gamma-encoded channels in [0, 1] and an alpha in [0, 1]. */
export interface Rgba {
  r: number;
  g: number;
  b: number;
  a: number;
}

const OKLCH = /^oklch\(\s*([\d.]+)(%?)\s+([\d.]+)\s+([\d.]+)(?:deg)?\s*(?:\/\s*([\d.]+)(%?)\s*)?\)$/;

/** Parses an `oklch(L C H)` or `oklch(L C H / A)` string. Returns undefined when it is not one. */
export function parseOklch(value: string): { l: number; c: number; h: number; alpha: number } | undefined {
  const m = OKLCH.exec(value.trim());
  if (!m) return undefined;
  const l = Number(m[1]) / (m[2] === '%' ? 100 : 1);
  const c = Number(m[3]);
  const h = Number(m[4]);
  let alpha = 1;
  if (m[5] !== undefined) alpha = Number(m[5]) / (m[6] === '%' ? 100 : 1);
  if ([l, c, h, alpha].some(Number.isNaN) || alpha < 0 || alpha > 1) return undefined;
  return { l, c, h, alpha };
}

function encode(linear: number): number {
  const x = Math.min(1, Math.max(0, linear));
  return x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055;
}

function decode(channel: number): number {
  return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
}

/** Converts an `oklch()` string to sRGB, clamping each channel to the sRGB gamut. */
export function oklchToRgba(value: string): Rgba {
  const p = parseOklch(value);
  if (!p) throw new Error(`not an oklch() color: ${value}`);
  const hr = (p.h * Math.PI) / 180;
  const a = p.c * Math.cos(hr);
  const b = p.c * Math.sin(hr);
  // OKLab to LMS to linear sRGB (Björn Ottosson's matrices).
  const l = (p.l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m = (p.l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s = (p.l - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return {
    r: encode(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s),
    g: encode(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s),
    b: encode(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s),
    a: p.alpha,
  };
}

/** Draws `top` over an opaque `base`, as the browser composites in sRGB. */
export function composite(top: Rgba, base: Rgba): Rgba {
  const a = top.a;
  return {
    r: top.r * a + base.r * (1 - a),
    g: top.g * a + base.g * (1 - a),
    b: top.b * a + base.b * (1 - a),
    a: 1,
  };
}

/** Applies an extra opacity, as Tailwind's `role/50` does. */
export function withOpacity(color: Rgba, opacity: number): Rgba {
  return { ...color, a: color.a * opacity };
}

/** Composites a stack of layers in order. The first must be opaque. */
export function flatten(layers: Rgba[]): Rgba {
  if (layers.length === 0) throw new Error('empty layer stack');
  if (layers[0].a < 1) throw new Error('the first layer of a stack must be opaque');
  let out = layers[0];
  for (const layer of layers.slice(1)) out = composite(layer, out);
  return out;
}

/** WCAG 2.2 relative luminance of an opaque color. */
export function luminance(c: Rgba): number {
  return 0.2126 * decode(c.r) + 0.7152 * decode(c.g) + 0.0722 * decode(c.b);
}

/** WCAG 2.2 contrast ratio of a foreground, composited over an opaque background. */
export function contrast(fg: Rgba, bg: Rgba): number {
  const top = fg.a < 1 ? composite(fg, bg) : fg;
  const l1 = luminance(top);
  const l2 = luminance(bg);
  return (Math.max(l1, l2) + 0.05) / (Math.min(l1, l2) + 0.05);
}

/** `#rrggbb` of an opaque color, for messages and tests. */
export function toHex(c: Rgba): string {
  const h = (x: number) => Math.round(x * 255).toString(16).padStart(2, '0');
  return `#${h(c.r)}${h(c.g)}${h(c.b)}`;
}
