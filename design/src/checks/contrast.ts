// Contrast: every declared pairing meets its kind's WCAG minimum in both themes, and every color
// role is in a pairing or marked decorative (FR-009 to FR-011, research R4).

import { contrast, flatten, oklchToRgba, withOpacity, type Rgba } from '../color.ts';
import type { Context } from '../cli.ts';
import type { Design, PairingKind } from '../parse.ts';
import { splitLayer } from './schema.ts';

export const MINIMUM: Record<PairingKind, number> = {
  text: 4.5,
  'large-text': 3,
  boundary: 3,
  focus: 3,
  chart: 3,
};

type Theme = 'light' | 'dark';

export function contrastLines(design: Design): string[] {
  const out: string[] = [];
  const label = design.label;
  const colors = new Map(design.colors.map((c) => [c.name, c]));
  const value = (role: string, theme: Theme): string | undefined => {
    const c = colors.get(role);
    return theme === 'light' ? c?.light : c?.dark;
  };
  const resolve = (ref: string, theme: Theme): Rgba | undefined => {
    const l = splitLayer(ref);
    const v = l && value(l.role, theme);
    if (!l || !v) return undefined;
    try {
      return withOpacity(oklchToRgba(v), l.opacity);
    } catch {
      return undefined;
    }
  };

  // A decorative entry is a whole role (`border`) or one opacity of it (`destructive/40`).
  const entries = (design.rules.decorative ?? []).map((d) => String(d.role));
  const decorative = new Set(entries.filter((e) => !e.includes('/')));
  const covered = new Set<string>(entries.map((e) => splitLayer(e)?.role ?? e));
  for (const p of design.rules.pairings ?? []) {
    if (!Array.isArray(p.on)) continue;
    for (const ref of [p.fg, ...p.on]) {
      const l = splitLayer(String(ref));
      if (l) covered.add(l.role);
    }
    const fgRole = splitLayer(String(p.fg))?.role;
    if (fgRole && decorative.has(fgRole) && (p.kind === 'boundary' || p.kind === 'focus')) {
      out.push(`${label}: role "${fgRole}" is decorative and cannot be the fg of a ${p.kind} pairing`);
    }

    const name = `${p.fg} on ${p.on.join(' + ')} (${p.kind}`;
    const min = MINIMUM[p.kind];
    if (min === undefined) continue; // the schema step names an unknown kind
    const ratios: string[] = [];
    const themes = Array.isArray(p.themes) ? p.themes : (['light', 'dark'] as const);
    for (const theme of themes) {
      const fg = resolve(String(p.fg), theme);
      const layers = p.on.map((ref) => resolve(String(ref), theme));
      if (!fg || layers.some((l) => !l)) continue; // the schema step names what does not resolve
      if ((layers[0] as Rgba).a < 1) {
        out.push(`${label}: pairing ${name}, ${theme}): the first layer "${p.on[0]}" is not opaque`);
        continue;
      }
      const ratio = contrast(fg, flatten(layers as Rgba[]));
      ratios.push(`${theme} ${ratio.toFixed(2)}`);
      // Compare at the precision reported, so a message never reads "4.50 < 4.50".
      if (Number(ratio.toFixed(2)) < min) {
        out.push(`${label}: pairing ${name}, ${theme}): ${ratio.toFixed(2)} < ${min.toFixed(2)}`);
      }
    }
    if (ratios.length === themes.length) out.push(`info: ${label}: ${name}): ${ratios.join(', ')}`);
  }

  for (const c of design.colors) {
    if (!covered.has(c.name)) out.push(`${label}: role "${c.name}" is in no pairing and not decorative`);
  }
  return out;
}

export function checkContrast(ctx: Context): string[] {
  return contrastLines(ctx.design);
}
