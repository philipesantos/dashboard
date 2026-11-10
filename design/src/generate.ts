// Writes the app's theme from DESIGN.md (specs/007-host-visual-system/contracts/outputs.md).

import type { Design, StyleMap } from './parse.ts';

export const THEME_FILE = 'app/src/theme.generated.css';
export const WIDGET_MAP_FILE = 'design/widget-map.generated.json';

/** What the declarative UI kit reads: role and token names only, never a value. */
export interface WidgetMap {
  generatedFrom: 'DESIGN.md';
  unknownTone: string | undefined;
  tones: Record<string, StyleMap>;
  variants: Record<string, StyleMap>;
  /** The elevation levels widget content may use. */
  elevation: string[];
}

const MAP_KEYS = ['text', 'fill', 'border', 'icon', 'rounded', 'spacing', 'typography'] as const;

function pick(style: StyleMap): StyleMap {
  const out: StyleMap = {};
  for (const key of MAP_KEYS) if (style[key] !== undefined) out[key] = String(style[key]);
  return out;
}

export function generateWidgetMap(design: Design): WidgetMap {
  const r = design.rules;
  return {
    generatedFrom: 'DESIGN.md',
    unknownTone: r.unknownTone,
    tones: Object.fromEntries(Object.entries(r.tones ?? {}).map(([k, v]) => [k, pick(v)])),
    variants: Object.fromEntries(Object.entries(r.variants ?? {}).map(([k, v]) => [k, pick(v)])),
    elevation: Object.entries(r.elevation ?? {}).filter(([, e]) => !e.hostOnly).map(([k]) => k),
  };
}

export function widgetMapJson(design: Design): string {
  return `${JSON.stringify(generateWidgetMap(design), null, 2)}\n`;
}

/** shadcn's radius steps, as multipliers of `--radius`. */
export const RADIUS_STEPS: [string, number][] = [
  ['sm', 0.6],
  ['md', 0.8],
  ['lg', 1],
  ['xl', 1.4],
  ['2xl', 1.8],
  ['3xl', 2.2],
  ['4xl', 2.6],
];

interface TypographyToken {
  fontFamily?: string;
  fontSize?: string;
  lineHeight?: string | number;
}

/** Quotes the first family of a stack when it holds a space: `'Figtree Variable', sans-serif`. */
export function fontStack(family: string): string {
  const parts = family.split(',').map((p) => p.trim()).filter(Boolean);
  return parts
    .map((p, i) => (i === 0 && /\s/.test(p) && !/^['"]/.test(p) ? `'${p}'` : p))
    .join(', ');
}

function rem(value: string): number | undefined {
  const m = /^([\d.]+)rem$/.exec(value);
  return m ? Number(m[1]) : undefined;
}

/** A line height as Tailwind writes it: unitless, `calc(<line> / <size>)` when both are rem. */
function lineHeight(size: string, line: string | number): string {
  if (typeof line === 'number') return String(line);
  const s = rem(size);
  const l = rem(line);
  if (s !== undefined && l !== undefined) return `calc(${l} / ${s})`;
  return line;
}

function typography(design: Design): Record<string, TypographyToken> {
  const t = design.front.typography;
  return typeof t === 'object' && t !== null ? (t as Record<string, TypographyToken>) : {};
}

export function generateCss(design: Design): string {
  const type = typography(design);
  const theme: string[] = [];
  const light: string[] = [];
  const dark: string[] = [];

  const heading = type.heading?.fontFamily;
  const body = Object.entries(type).find(([name, t]) => name.startsWith('body-') && t.fontFamily)?.[1].fontFamily;
  const mono = type.mono?.fontFamily;
  if (heading) theme.push(`--font-heading: ${fontStack(heading)};`);
  if (body) theme.push(`--font-sans: ${fontStack(body)};`);
  if (mono) theme.push(`--font-mono: ${fontStack(mono)};`);

  for (const color of design.colors) {
    theme.push(`--color-${color.name}: var(--${color.name});`);
    light.push(`--${color.name}: ${color.light};`);
    if (color.dark !== undefined) dark.push(`--${color.name}: ${color.dark};`);
  }

  const rounded = design.front.rounded as Record<string, string> | undefined;
  if (rounded?.base !== undefined) {
    for (const [step, k] of RADIUS_STEPS) {
      theme.push(`--radius-${step}: ${k === 1 ? 'var(--radius)' : `calc(var(--radius) * ${k})`};`);
    }
    light.push(`--radius: ${rounded.base};`);
  }

  const spacing = design.front.spacing as Record<string, string> | undefined;
  if (spacing?.unit !== undefined) theme.push(`--spacing: ${spacing.unit};`);

  for (const [name, t] of Object.entries(type)) {
    if (!name.startsWith('body-') || t.fontSize === undefined) continue;
    const step = name.slice('body-'.length);
    theme.push(`--text-${step}: ${t.fontSize};`);
    if (t.lineHeight !== undefined) theme.push(`--text-${step}--line-height: ${lineHeight(t.fontSize, t.lineHeight)};`);
  }

  for (const [level, e] of Object.entries(design.rules.elevation ?? {})) {
    theme.push(`--shadow-${level}: var(--elevation-${level});`);
    light.push(`--elevation-${level}: ${e.shadow};`);
    dark.push(`--elevation-${level}: ${e.dark};`);
  }

  const block = (selector: string, lines: string[]) =>
    `${selector} {\n${lines.map((l) => `    ${l}`).join('\n')}\n}\n`;
  return [
    '/* Generated from DESIGN.md by `pnpm --filter ./design gen`. Do not edit. */\n',
    block('@theme inline', theme),
    block(':root', light),
    block('.dark', dark),
  ].join('\n');
}
