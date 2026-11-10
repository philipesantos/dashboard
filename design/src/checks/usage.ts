// The usage scan: every color the app's code draws is a declared role at a declared opacity
// (research R4, data-model.md "Usage"). Pairings only prove contrast for what is declared; this
// ties them to what the components really draw.

import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';
import type { Context } from '../cli.ts';
import type { Design } from '../parse.ts';
import { splitLayer } from './schema.ts';

/** Tailwind utilities that take a color. `border` also has per-side forms. */
const PREFIXES = [
  'bg', 'text', 'border', 'border-x', 'border-y', 'border-t', 'border-r', 'border-b', 'border-l',
  'border-s', 'border-e', 'ring', 'ring-offset', 'inset-ring', 'outline', 'fill', 'stroke',
  'divide', 'placeholder', 'caret', 'accent', 'shadow', 'inset-shadow', 'decoration', 'from',
  'via', 'to',
];
/** No color of their own: transparent shows the layer beneath, the others repeat the text color. */
const SKIP = new Set(['transparent', 'current', 'inherit']);
const PALETTE =
  /^(?:black|white|(?:slate|gray|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d{2,3})$/;
const CSS_COLOR = /^(?:#|rgba?\(|hsla?\(|hwb\(|lab\(|lch\(|oklab\(|oklch\(|color\(|color-mix\()/;

export interface Usage {
  file: string;
  utility: string;
  /** The role named, when it is one. */
  role?: string;
  /** A name used as a color that is not a role. */
  unknown?: string;
  /** 1 to 100. */
  opacity: number;
}

/** Splits `dark:hover:bg-input/50` into variants and the utility, ignoring `:` inside brackets. */
function baseUtility(token: string): string {
  let depth = 0;
  let start = 0;
  for (let i = 0; i < token.length; i++) {
    const ch = token[i];
    if (ch === '[' || ch === '(') depth++;
    else if (ch === ']' || ch === ')') depth--;
    else if (ch === ':' && depth === 0) start = i + 1;
  }
  return token.slice(start).replace(/^!|!$/g, '').replace(/^-/, '');
}

function opacityOf(suffix: string | undefined): number | undefined {
  if (suffix === undefined) return 100;
  if (/^\d{1,3}$/.test(suffix)) return Number(suffix);
  const m = /^\[(0?\.\d+|1(?:\.0+)?)\]$/.exec(suffix);
  return m ? Math.round(Number(m[1]) * 100) : undefined;
}

/** The color usages one class token makes; empty when it is not a color. */
export function usagesOf(token: string, roles: ReadonlySet<string>, file: string): Usage[] {
  const util = baseUtility(token);
  const prefix = PREFIXES.filter((p) => util.startsWith(`${p}-`)).sort((a, b) => b.length - a.length)[0];
  if (!prefix) return [];
  let rest = util.slice(prefix.length + 1);

  // Arbitrary values: bg-[#fff], bg-[color-mix(…var(--secondary),var(--foreground)_5%)].
  if (rest.startsWith('[')) {
    const close = rest.lastIndexOf(']');
    const inner = rest.slice(1, close).replace(/^color:/, '');
    const opacity = opacityOf(rest.slice(close + 1).replace(/^\//, '') || undefined) ?? 100;
    if (!CSS_COLOR.test(inner) && !/^var\(--/.test(inner)) return [];
    const vars = [...inner.matchAll(/var\(--([\w-]+)\)(?:_(\d{1,3})%)?/g)];
    if (vars.length === 0 || /#|rgba?\(|hsla?\(|oklch\(/.test(inner.replace(/var\([^)]*\)/g, ''))) {
      return [{ file, utility: token, opacity }];
    }
    return vars.map((m) =>
      roles.has(m[1])
        ? { file, utility: token, role: m[1], opacity: m[2] ? Number(m[2]) : opacity }
        : { file, utility: token, unknown: m[1], opacity: m[2] ? Number(m[2]) : opacity },
    );
  }
  // Variable shorthand: bg-(--card).
  if (rest.startsWith('(--')) {
    const name = rest.slice(3, rest.indexOf(')'));
    return roles.has(name) ? [{ file, utility: token, role: name, opacity: 100 }] : [];
  }

  const slash = rest.lastIndexOf('/');
  const suffix = slash === -1 ? undefined : rest.slice(slash + 1);
  if (slash !== -1) rest = rest.slice(0, slash);
  const opacity = opacityOf(suffix);
  if (opacity === undefined) return [];
  if (SKIP.has(rest)) return [];
  if (roles.has(rest)) return [{ file, utility: token, role: rest, opacity }];
  if (PALETTE.test(rest)) return [{ file, utility: token, opacity }];
  // A name with an opacity is a color even when it is not a role; without one it may be a size.
  if (suffix !== undefined && /^[a-z][a-z0-9-]*$/.test(rest)) return [{ file, utility: token, unknown: rest, opacity }];
  return [];
}

function walk(dir: string, out: string[] = []): string[] {
  let entries: string[];
  try {
    entries = readdirSync(dir).sort();
  } catch {
    return out;
  }
  for (const name of entries) {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) walk(path, out);
    else out.push(path);
  }
  return out;
}

/** Scans `app/src/**\/*.tsx` and the `@apply` lines of `app/src/*.css` under `root`. */
export function scanUsages(root: string, roles: Iterable<string>): Usage[] {
  const known = new Set(roles);
  const src = join(root, 'app', 'src');
  const files = walk(src).filter(
    (f) => f.endsWith('.tsx') || (f.endsWith('.css') && relative(src, f).split(sep).length === 1),
  );
  const out: Usage[] = [];
  for (const path of files.sort()) {
    const file = relative(root, path).split(sep).join('/');
    let text = readFileSync(path, 'utf8');
    if (path.endsWith('.css')) {
      text = [...text.matchAll(/@apply\s+([^;]+);/g)].map((m) => m[1]).join(' ');
    }
    for (const raw of text.match(/[^\s"'`{};]+/g) ?? []) {
      out.push(...usagesOf(raw.replace(/,$/, ''), known, file));
    }
  }
  return out;
}

/** Each role's declared opacities: as a pairing's fg, as a layer, or any when decorative. */
export function declaredOpacities(design: Design): { opacities: Map<string, Set<number>>; decorative: Set<string> } {
  const opacities = new Map<string, Set<number>>();
  const add = (ref: unknown) => {
    const l = splitLayer(String(ref));
    if (!l) return;
    if (!opacities.has(l.role)) opacities.set(l.role, new Set());
    opacities.get(l.role)!.add(Math.round(l.opacity * 100));
  };
  for (const p of design.rules.pairings ?? []) {
    add(p.fg);
    if (Array.isArray(p.on)) p.on.forEach(add);
  }
  return { opacities, decorative: new Set((design.rules.decorative ?? []).map((d) => d.role)) };
}

export function usageLines(root: string, design: Design): string[] {
  const roles = design.colors.map((c) => c.name);
  const { opacities, decorative } = declaredOpacities(design);
  const out: string[] = [];
  for (const u of scanUsages(root, roles)) {
    if (u.unknown !== undefined) {
      out.push(`${u.file}: ${u.utility} names "${u.unknown}", which is not a color role`);
    } else if (u.role === undefined) {
      out.push(`${u.file}: ${u.utility} names no color role`);
    } else if (!decorative.has(u.role) && !opacities.get(u.role)?.has(u.opacity)) {
      const declared = [...(opacities.get(u.role) ?? [])].sort((a, b) => a - b);
      const at = declared.length === 0 ? 'in no pairing' : `declared at ${declared.join(', ')}${declared.length === 1 ? ' only' : ''}`;
      out.push(`${u.file}: ${u.utility} is not declared (${u.role} is ${at})`);
    }
  }
  return out;
}

export function checkUsage(ctx: Context): string[] {
  return usageLines(ctx.root, ctx.design);
}
