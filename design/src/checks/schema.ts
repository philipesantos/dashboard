// The project layer's schema: every role has a dark twin and an oklch() value, the typography
// and shape tokens the generator needs exist, and every name a design-rules block uses resolves
// (specs/007-host-visual-system/contracts/design-md.md, data-model.md).

import { parseOklch } from '../color.ts';
import type { Context } from '../cli.ts';
import { PAIRING_KINDS, type Design, type StyleMap } from '../parse.ts';

export const COMPONENTS = ['text', 'list', 'table', 'chart', 'button', 'form'] as const;
const STYLE_ROLES = ['text', 'fill', 'border'] as const;

/** Splits `role/50` into the role and an opacity in [0, 1]. */
export function splitLayer(ref: string): { role: string; opacity: number } | undefined {
  const m = /^([a-z0-9-]+)(?:\/(\d{1,3}))?$/.exec(String(ref));
  if (!m) return undefined;
  const opacity = m[2] === undefined ? 1 : Number(m[2]) / 100;
  if (opacity <= 0 || opacity > 1) return undefined;
  return { role: m[1], opacity };
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

export function schemaErrors(design: Design): string[] {
  const out: string[] = [];
  const err = (what: string) => out.push(`${design.label}: ${what}`);
  const roles = new Set(design.colors.map((c) => c.name));
  const role = (name: unknown, where: string) => {
    if (typeof name !== 'string' || !roles.has(name)) err(`${where} names "${String(name)}", which is not a color role`);
  };
  const layer = (ref: unknown, where: string) => {
    const l = splitLayer(String(ref));
    if (!l) err(`${where}: "${String(ref)}" is not a role or role/opacity`);
    else role(l.role, where);
  };

  for (const c of design.colors) {
    if (c.dark === undefined) err(`color "${c.name}" has no "${c.name}-dark"`);
    for (const [theme, value] of [['light', c.light], ['dark', c.dark]] as const) {
      if (value !== undefined && !parseOklch(value)) err(`color "${c.name}" (${theme}) is not an oklch() value: ${value}`);
    }
  }

  // Typography, rounded and spacing: what the generator reads.
  const type = isRecord(design.front.typography) ? design.front.typography : {};
  for (const name of ['heading', 'mono']) {
    if (!isRecord(type[name]) || typeof type[name].fontFamily !== 'string') err(`typography "${name}" needs a fontFamily`);
  }
  const bodies = Object.entries(type).filter(([n]) => n.startsWith('body-'));
  if (bodies.length === 0) err('typography needs at least one body-<step> token');
  const families = new Set(bodies.map(([, t]) => (isRecord(t) ? t.fontFamily : undefined)));
  if (families.size > 1) err('every body-<step> token must use the same fontFamily, which becomes --font-sans');
  for (const [name, t] of bodies) {
    if (!isRecord(t) || typeof t.fontSize !== 'string' || !/^[\d.]+rem$/.test(t.fontSize)) {
      err(`typography "${name}" needs a fontSize in rem`);
    }
  }
  const rounded = isRecord(design.front.rounded) ? design.front.rounded : {};
  if (typeof rounded.base !== 'string') err('rounded needs "base", the radius every step derives from');
  const spacing = isRecord(design.front.spacing) ? design.front.spacing : {};
  if (typeof spacing.unit !== 'string') err('spacing needs "unit", Tailwind\'s --spacing');

  // design-rules.
  const r = design.rules;
  for (const prefix of r.hostOnly ?? []) {
    if (![...roles].some((n) => n === prefix || n.startsWith(`${prefix}-`))) {
      err(`hostOnly entry "${prefix}" matches no color role`);
    }
  }
  for (const d of r.decorative ?? []) {
    role(d?.role, 'a decorative entry');
    if (typeof d?.reason !== 'string' || d.reason.trim() === '') err(`decorative role "${d?.role}" has no reason`);
  }
  for (const [i, p] of (r.pairings ?? []).entries()) {
    const where = `pairing ${i + 1} (${p?.fg} on ${Array.isArray(p?.on) ? p.on.join(', ') : p?.on})`;
    layer(p?.fg, where);
    if (!Array.isArray(p?.on) || p.on.length === 0) err(`${where}: "on" must be a non-empty list`);
    else for (const ref of p.on) layer(ref, where);
    if (!(PAIRING_KINDS as readonly string[]).includes(p?.kind)) err(`${where}: kind "${p?.kind}" is not one of ${PAIRING_KINDS.join(', ')}`);
  }
  for (const [level, e] of Object.entries(r.elevation ?? {})) {
    if (!isRecord(e) || typeof e.shadow !== 'string' || typeof e.dark !== 'string') {
      err(`elevation "${level}" needs a shadow and a dark shadow`);
    }
  }
  const styles = (kind: string, entries: Record<string, StyleMap> | undefined) => {
    for (const [name, s] of Object.entries(entries ?? {})) {
      if (!isRecord(s)) {
        err(`${kind} ${name} is not a mapping`);
        continue;
      }
      for (const key of STYLE_ROLES) if (s[key] !== undefined) layer(s[key], `${kind} ${name} ${key}`);
    }
  };
  styles('tone', r.tones);
  styles('variant', r.variants);
  for (const name of Object.keys(r.variants ?? {})) {
    const [component, variant] = name.split('.');
    if (!(COMPONENTS as readonly string[]).includes(component) || !variant) {
      err(`variant "${name}" is not <component>.<variant> with a component in ${COMPONENTS.join(', ')}`);
    }
  }
  if (r.unknownTone !== undefined && !(r.unknownTone in (r.tones ?? {}))) {
    err(`unknownTone "${r.unknownTone}" is not a tone`);
  }
  return out;
}

export function checkSchema(ctx: Context): string[] {
  return schemaErrors(ctx.design);
}
