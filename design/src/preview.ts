// A static page for reviewing the visual system without building the app (FR-008, research R8):
// no script, no remote resource, inline CSS only. Not committed; CI publishes it.

import { contrast, flatten, oklchToRgba, withOpacity, type Rgba } from './color.ts';
import { RADIUS_STEPS } from './generate.ts';
import { MINIMUM } from './checks/contrast.ts';
import { hostOnlyRoles } from './checks/hostonly.ts';
import { splitLayer } from './checks/schema.ts';
import type { Design, StyleMap } from './parse.ts';

export const PREVIEW_FILE = 'design/preview/index.html';

type Theme = 'light' | 'dark';

function esc(s: unknown): string {
  return String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

/** `role` or `role/50` as a CSS color over the theme's variables. */
function cssColor(ref: string | undefined): string {
  if (!ref) return 'transparent';
  const l = splitLayer(ref);
  if (!l) return 'transparent';
  return l.opacity === 1 ? `var(--${l.role})` : `color-mix(in oklab, var(--${l.role}) ${Math.round(l.opacity * 100)}%, transparent)`;
}

function themeVars(design: Design, theme: Theme): string {
  const lines = design.colors.map((c) => `--${c.name}: ${theme === 'light' ? c.light : (c.dark ?? c.light)};`);
  const rounded = isRecord(design.front.rounded) ? design.front.rounded : {};
  lines.push(`--radius: ${rounded.base ?? '0.5rem'};`);
  for (const [level, e] of Object.entries(design.rules.elevation ?? {})) {
    lines.push(`--elevation-${level}: ${theme === 'light' ? e.shadow : e.dark};`);
  }
  return lines.join(' ');
}

function ratios(design: Design, theme: Theme): Map<string, { ratio: number; min: number }> {
  const colors = new Map(design.colors.map((c) => [c.name, c]));
  const resolve = (ref: string): Rgba | undefined => {
    const l = splitLayer(ref);
    const c = l && colors.get(l.role);
    const v = c && (theme === 'light' ? c.light : c.dark);
    return l && v ? withOpacity(oklchToRgba(v), l.opacity) : undefined;
  };
  const out = new Map<string, { ratio: number; min: number }>();
  for (const p of design.rules.pairings ?? []) {
    if (Array.isArray(p.themes) && !p.themes.includes(theme)) continue;
    const fg = resolve(p.fg);
    const layers = p.on.map(resolve);
    if (!fg || layers.some((l) => !l) || (layers[0] as Rgba).a < 1) continue;
    out.set(`${p.fg} on ${p.on.join(' + ')} (${p.kind})`, {
      ratio: contrast(fg, flatten(layers as Rgba[])),
      min: MINIMUM[p.kind],
    });
  }
  return out;
}

function colorsSection(design: Design, theme: Theme): string {
  const hostOnly = hostOnlyRoles(design);
  const swatches = design.colors
    .map((c) => {
      const value = theme === 'light' ? c.light : c.dark;
      const mark = hostOnly.has(c.name) ? ' <span class="tag">host-only</span>' : '';
      return `<li><span class="swatch" style="background: var(--${esc(c.name)})"></span><span class="role">${esc(c.name)}</span>${mark}<code>${esc(value)}</code></li>`;
    })
    .join('');
  const rows = [...ratios(design, theme)]
    .map(([name, r]) => {
      const ok = Number(r.ratio.toFixed(2)) >= r.min;
      return `<tr class="${ok ? 'pass' : 'fail'}"><td>${esc(name)}</td><td class="num">${r.ratio.toFixed(2)}</td><td class="num">${r.min.toFixed(1)}</td></tr>`;
    })
    .join('');
  return `<h2>Colors</h2><ul class="swatches">${swatches}</ul>
<h3>Pairings</h3><table class="pairings"><thead><tr><th>Pairing</th><th>Ratio</th><th>Min</th></tr></thead><tbody>${rows}</tbody></table>`;
}

function typeSection(design: Design): string {
  const type = isRecord(design.front.typography) ? design.front.typography : {};
  const rows = Object.entries(type)
    .map(([name, t]) => {
      if (!isRecord(t)) return '';
      const style = [
        t.fontFamily ? `font-family: ${esc(t.fontFamily)}` : '',
        t.fontSize ? `font-size: ${esc(t.fontSize)}` : '',
        t.lineHeight ? `line-height: ${esc(t.lineHeight)}` : '',
        t.fontWeight ? `font-weight: ${esc(t.fontWeight)}` : '',
      ].filter(Boolean).join('; ');
      const sample = name === 'mono' ? 'github.com/owner/repo' : 'Tarefas de hoje: 3 abertas';
      return `<li><span class="label">${esc(name)}</span><span style="${style}">${sample}</span></li>`;
    })
    .join('');
  return `<h2>Typography</h2><p class="note">Fonts render here in their fallback unless installed; the app bundles them.</p><ul class="type">${rows}</ul>`;
}

function scalesSection(design: Design): string {
  const spacing = isRecord(design.front.spacing) ? design.front.spacing : {};
  const space = Object.entries(spacing)
    .map(([name, v]) => `<li><span class="label">${esc(name)}</span><span class="bar" style="width: ${esc(v)}"></span><code>${esc(v)}</code></li>`)
    .join('');
  const radius = RADIUS_STEPS.map(
    ([step, k]) => `<li><span class="box radius-${step}" style="border-radius: calc(var(--radius) * ${k})"></span><span class="label">radius-${step}</span></li>`,
  ).join('');
  const elevation = Object.entries(design.rules.elevation ?? {})
    .map(([level, e]) => `<li><span class="box" style="box-shadow: var(--elevation-${esc(level)})"></span><span class="label">${esc(level)}${e.hostOnly ? ' (host-only)' : ''}</span></li>`)
    .join('');
  return `<h2>Layout</h2><ul class="space">${space}</ul>
<h2>Shapes</h2><ul class="boxes">${radius}</ul>
<h2>Elevation</h2><ul class="boxes">${elevation}</ul>`;
}

function componentsSection(design: Design): string {
  const tones = Object.entries(design.rules.tones ?? {})
    .map(([name, t]) => {
      const icon = t.icon ? `<span class="icon" title="${esc(t.icon)}">[${esc(t.icon)}]</span> ` : '';
      return `<li style="color: ${cssColor(t.text)}">${icon}<strong>${esc(name)}</strong> tone</li>`;
    })
    .join('');
  const variants = Object.entries(design.rules.variants ?? {})
    .map(([name, v]: [string, StyleMap]) => {
      const step = RADIUS_STEPS.find(([s]) => s === v.rounded)?.[1];
      const style = [
        `color: ${cssColor(v.text)}`,
        `background: ${cssColor(v.fill)}`,
        v.border ? `border: 1px solid ${cssColor(v.border)}` : 'border: 1px solid transparent',
        step !== undefined ? `border-radius: calc(var(--radius) * ${step})` : '',
      ].filter(Boolean).join('; ');
      return `<li><span class="variant" style="${style}">${esc(name)}</span></li>`;
    })
    .join('');
  return `<h2>Components</h2><h3>Tones</h3><ul class="tones">${tones}</ul><h3>Variants</h3><ul class="variants">${variants}</ul>`;
}

function sketch(): string {
  return `<h2>Host surfaces</h2>
<div class="sketch">
  <div class="sketch-chrome">Dashboard</div>
  <div class="sketch-grid">
    <div class="sketch-frame"><div class="sketch-frame-header">● Tarefas <small>github.com/owner/tasks</small></div><div class="sketch-frame-body">Widget content</div></div>
    <div class="sketch-frame"><div class="sketch-frame-header">● Notas</div><div class="sketch-frame-body">Widget content</div></div>
  </div>
  <div class="sketch-scrim"></div>
  <div class="sketch-dialog"><strong>Instalar Tarefas?</strong><p>github.com/owner/tasks pede acesso ao armazenamento.</p><span class="variant safe">Cancelar</span> <span class="variant risky">Instalar</span></div>
</div>`;
}

const STYLE = `
* { box-sizing: border-box; }
body { margin: 0; font-family: system-ui, sans-serif; background: #888; }
header { padding: 16px 24px; background: #222; color: #eee; }
main { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 560px), 1fr)); }
.theme { padding: 16px 24px; background: var(--background); color: var(--foreground); }
h2 { font-size: 1.1rem; margin: 24px 0 8px; } h3 { font-size: 0.95rem; margin: 16px 0 6px; }
ul { list-style: none; padding: 0; margin: 0; }
code { font-family: ui-monospace, monospace; font-size: 0.75rem; color: var(--muted-foreground); margin-left: auto; }
.swatches li, .type li, .space li { display: flex; align-items: center; gap: 8px; padding: 2px 0; }
.swatch { width: 28px; height: 20px; border-radius: 4px; border: 1px solid var(--border); flex: none; }
.role, .label { font-size: 0.8rem; min-width: 10rem; }
.tag { font-size: 0.65rem; padding: 0 4px; border: 1px solid var(--muted-foreground); border-radius: 4px; color: var(--muted-foreground); }
table { border-collapse: collapse; width: 100%; font-size: 0.75rem; }
td, th { padding: 2px 6px; border-bottom: 1px solid var(--border); text-align: left; }
.num { text-align: right; font-variant-numeric: tabular-nums; }
tr.fail td { color: var(--destructive); font-weight: 700; }
.note { font-size: 0.75rem; color: var(--muted-foreground); }
.bar { display: inline-block; height: 10px; background: var(--primary); }
.boxes { display: flex; flex-wrap: wrap; gap: 16px; }
.boxes li { display: flex; flex-direction: column; align-items: center; gap: 4px; }
.box { width: 56px; height: 40px; background: var(--card); border: 1px solid var(--border); }
.tones li { padding: 2px 0; }
.variants { display: flex; flex-wrap: wrap; gap: 8px; }
.variant { display: inline-block; padding: 4px 10px; font-size: 0.8rem; }
.sketch { position: relative; height: 260px; border: 1px solid var(--border); overflow: hidden; background: var(--background); }
.sketch-chrome { position: absolute; inset: 0 auto 0 0; width: 110px; padding: 8px; background: var(--sidebar); color: var(--sidebar-foreground); border-right: 1px solid var(--sidebar-border); font-size: 0.8rem; }
.sketch-grid { position: absolute; inset: 12px 12px 12px 122px; display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.sketch-frame { background: var(--frame); color: var(--frame-foreground); border: 1px solid var(--frame-border); border-radius: calc(var(--radius) * 1.4); box-shadow: var(--elevation-frame); overflow: hidden; }
.sketch-frame-header { padding: 6px 10px; background: var(--frame-header); color: var(--frame-header-foreground); font-size: 0.8rem; }
.sketch-frame-header small { color: var(--muted-foreground); font-family: ui-monospace, monospace; }
.sketch-frame-body { padding: 10px; font-size: 0.8rem; }
.sketch-scrim { position: absolute; inset: 0; background: var(--scrim); }
.sketch-dialog { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); width: 280px; padding: 14px; background: var(--dialog); color: var(--dialog-foreground); border-radius: calc(var(--radius) * 1.4); box-shadow: var(--elevation-dialog); font-size: 0.8rem; }
.sketch-dialog .safe { background: var(--secondary); color: var(--secondary-foreground); border-radius: var(--radius); }
.sketch-dialog .risky { background: var(--primary); color: var(--primary-foreground); border-radius: var(--radius); }
`;

export function previewHtml(design: Design): string {
  const column = (theme: Theme) => `<section class="theme ${theme}" style="${esc(themeVars(design, theme))}">
<h1>${theme === 'light' ? 'Light' : 'Dark'}</h1>
${colorsSection(design, theme)}
${typeSection(design)}
${scalesSection(design)}
${componentsSection(design)}
${sketch()}
</section>`;
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(design.front.name ?? 'Design')} visual system</title>
<style>${STYLE}</style>
</head>
<body>
<header><strong>${esc(design.front.name ?? '')}</strong>: ${esc(String(design.front.description ?? '').replace(/\.$/, ''))}. Generated from DESIGN.md by <code>pnpm --filter ./design preview</code>.</header>
<main>
${column('light')}
${column('dark')}
</main>
</body>
</html>
`;
}
