// What widget content can select resolves to shared roles only (FR-013 to FR-015, research R6).

import type { Context } from '../cli.ts';
import { RADIUS_STEPS } from '../generate.ts';
import type { Design, StyleMap } from '../parse.ts';
import { splitLayer } from './schema.ts';

export const TONES = ['default', 'muted', 'accent', 'ok', 'warning', 'error'] as const;
export const STATUS_TONES = ['ok', 'warning', 'error'] as const;
const STYLE_KEYS = ['text', 'fill', 'border'] as const;
/** The focus style is the host's on every control; no tone or variant may name it. */
const FOCUS = 'ring';

/** Every role a `hostOnly` entry covers: the entry itself and every role starting with it and `-`. */
export function hostOnlyRoles(design: Design): Set<string> {
  const prefixes = design.rules.hostOnly ?? [];
  return new Set(
    design.colors.map((c) => c.name).filter((n) => prefixes.some((p) => n === p || n.startsWith(`${p}-`))),
  );
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

export function hostOnlyLines(design: Design): string[] {
  const out: string[] = [];
  const err = (what: string) => out.push(`${design.label}: ${what}`);
  const hostOnly = hostOnlyRoles(design);
  const r = design.rules;

  const reach = (kind: 'tone' | 'variant', name: string, style: StyleMap) => {
    for (const key of STYLE_KEYS) {
      const ref = style[key];
      if (ref === undefined) continue;
      const role = splitLayer(String(ref))?.role;
      if (role === FOCUS) err(`${kind} ${name} maps ${key} to "${FOCUS}", the focus style the host draws on every control`);
      else if (role && hostOnly.has(role)) err(`${kind} ${name} maps ${key} to host-only role "${role}"`);
    }
  };

  const tones = r.tones ?? {};
  for (const name of TONES) if (!(name in tones)) err(`tone ${name} is missing`);
  for (const [name, style] of Object.entries(tones)) {
    if (!isRecord(style)) continue;
    reach('tone', name, style);
    if ((STATUS_TONES as readonly string[]).includes(name) && typeof style.icon !== 'string') {
      err(`tone ${name} has no icon; the host draws one beside every status (FR-012)`);
    }
    if (style.icon !== undefined && !/^[a-z0-9]+(-[a-z0-9]+)*$/.test(String(style.icon))) {
      err(`tone ${name} icon "${style.icon}" is not a lucide icon name`);
    }
  }
  if (r.unknownTone === undefined) err('unknownTone is missing; an unknown tone needs a fallback');

  const components = isRecord(design.front.components) ? design.front.components : {};
  const spacing = isRecord(design.front.spacing) ? design.front.spacing : {};
  const typography = isRecord(design.front.typography) ? design.front.typography : {};
  const steps = new Set(RADIUS_STEPS.map(([s]) => s));
  const variants = r.variants ?? {};
  for (const [name, style] of Object.entries(variants)) {
    if (!isRecord(style)) continue;
    reach('variant', name, style);
    if (style.rounded !== undefined && !steps.has(String(style.rounded))) {
      err(`variant ${name} rounded "${style.rounded}" is not a radius step (${[...steps].join(', ')})`);
    }
    if (style.spacing !== undefined && !(String(style.spacing) in spacing)) {
      err(`variant ${name} spacing "${style.spacing}" is not a spacing token`);
    }
    if (style.typography !== undefined && !(String(style.typography) in typography)) {
      err(`variant ${name} typography "${style.typography}" is not a typography token`);
    }
    // The variant and the format's component describe the same thing; they must agree.
    const componentName = name.replace('.', '-');
    const component = components[componentName];
    if (!isRecord(component)) {
      err(`variant ${name} has no format component ${componentName}`);
      continue;
    }
    for (const [key, prop] of [['text', 'textColor'], ['fill', 'backgroundColor']] as const) {
      const ref = style[key];
      if (ref === undefined || String(ref).includes('/') || component[prop] === undefined) continue;
      if (hostOnly.has(String(ref)) || ref === FOCUS) continue; // already named above
      if (component[prop] !== `{colors.${ref}}`) {
        err(`variant ${name} maps ${key} to "${ref}", but component ${componentName} has ${prop} ${component[prop]}`);
      }
    }
  }
  for (const name of Object.keys(components)) {
    if (name.endsWith('-dark') && name.slice(0, -5) in components) continue;
    const variant = name.replace('-', '.');
    if (!(variant in variants)) err(`component ${name} has no variant ${variant}`);
  }

  for (const [level, e] of Object.entries(r.elevation ?? {})) {
    if (level !== 'flat' && !(isRecord(e) && e.hostOnly === true)) {
      err(`elevation "${level}" is above flat and must be host-only`);
    }
  }
  return out;
}

export function checkHostOnly(ctx: Context): string[] {
  return hostOnlyLines(ctx.design);
}
