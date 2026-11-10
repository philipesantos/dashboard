// Reads DESIGN.md: the format's front matter and sections, plus the project's `design-rules`
// blocks and the bold-named rationale of each token
// (specs/007-host-visual-system/contracts/design-md.md).

import { parse as parseYaml } from 'yaml';

/** The format's `##` sections, in the order the format requires. */
export const SECTIONS = [
  'Overview',
  'Colors',
  'Typography',
  'Layout',
  'Elevation & Depth',
  'Shapes',
  'Components',
  "Do's and Don'ts",
] as const;

/** The section each `design-rules` key belongs to. */
export const RULE_SECTIONS: Record<string, string> = {
  hostOnly: 'Colors',
  decorative: 'Colors',
  pairings: 'Colors',
  elevation: 'Elevation & Depth',
  tones: 'Components',
  variants: 'Components',
  unknownTone: 'Components',
};

/** The section whose prose holds the rationale of each front-matter group. */
export const GROUP_SECTIONS: Record<string, string> = {
  colors: 'Colors',
  typography: 'Typography',
  spacing: 'Layout',
  rounded: 'Shapes',
  components: 'Components',
};

export const PAIRING_KINDS = ['text', 'large-text', 'boundary', 'focus', 'chart'] as const;
export type PairingKind = (typeof PAIRING_KINDS)[number];

export interface Pairing {
  fg: string;
  on: string[];
  kind: PairingKind;
}

export interface ElevationLevel {
  shadow: string;
  dark: string;
  hostOnly?: boolean;
}

/** A tone or variant: the roles it draws with, and for variants the shape tokens. */
export interface StyleMap {
  text?: string;
  fill?: string;
  border?: string;
  icon?: string;
  rounded?: string;
  spacing?: string;
  typography?: string;
}

export interface Rules {
  hostOnly?: string[];
  decorative?: { role: string; reason: string }[];
  pairings?: Pairing[];
  elevation?: Record<string, ElevationLevel>;
  tones?: Record<string, StyleMap>;
  variants?: Record<string, StyleMap>;
  unknownTone?: string;
}

export interface Section {
  heading: string;
  /** The section's text with every fenced block removed. */
  prose: string;
}

export interface Color {
  name: string;
  light: string;
  dark?: string;
}

export interface Design {
  label: string;
  front: Record<string, unknown>;
  /** Color roles in front-matter order, each with its `-dark` twin folded in. */
  colors: Color[];
  sections: Section[];
  rules: Rules;
  errors: string[];
}

const FENCE = /^(`{3,}|~{3,})\s*([^\s`]*)/;

function splitFrontMatter(text: string): { yaml: string; body: string } | undefined {
  if (!text.startsWith('---\n')) return undefined;
  const end = text.indexOf('\n---\n', 4);
  if (end === -1) return undefined;
  return { yaml: text.slice(4, end + 1), body: text.slice(end + 5) };
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function escapeRegExp(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/** True when `name` appears in bold in `prose`, followed by more text. */
export function hasRationale(prose: string, name: string): boolean {
  return new RegExp(`\\*\\*${escapeRegExp(name)}\\*\\*[ \\t]*[:,.—–-]?[ \\t]*\\S`).test(prose);
}

/** Parses DESIGN.md text. Structural problems come back as `<label>: <what>` lines. */
export function parseDesign(source: string, label = 'DESIGN.md'): Design {
  const text = source.replace(/\r\n/g, '\n');
  const errors: string[] = [];
  const err = (what: string) => errors.push(`${label}: ${what}`);
  const design: Design = { label, front: {}, colors: [], sections: [], rules: {}, errors };

  const split = splitFrontMatter(text);
  if (!split) {
    err('no YAML front matter between --- lines at the top');
    return design;
  }
  try {
    const front = parseYaml(split.yaml);
    if (!isRecord(front)) err('the front matter is not a mapping');
    else design.front = front;
  } catch (e) {
    err(`the front matter is not valid YAML: ${(e as Error).message.split('\n')[0]}`);
  }

  // Colors, with each `<role>-dark` folded into its role.
  const colors = isRecord(design.front.colors) ? design.front.colors : {};
  const names = Object.keys(colors);
  for (const name of names) {
    const value = colors[name];
    if (typeof value !== 'string') {
      err(`color "${name}" is not a string`);
      continue;
    }
    if (name.endsWith('-dark')) {
      const base = name.slice(0, -'-dark'.length);
      if (!(base in colors)) err(`color "${name}" has no light role "${base}"`);
      continue;
    }
    const dark = colors[`${name}-dark`];
    design.colors.push({ name, light: value, dark: typeof dark === 'string' ? dark : undefined });
  }

  // Sections and design-rules blocks.
  const ruleSource: Record<string, string> = {};
  let current: { heading: string; lines: string[] } | undefined;
  let fence: { marker: string; info: string; lines: string[] } | undefined;
  const closeSection = () => {
    if (current) design.sections.push({ heading: current.heading, prose: current.lines.join('\n') });
  };
  for (const line of split.body.split('\n')) {
    if (fence) {
      const t = line.trim();
      if (t.length >= fence.marker.length && t === fence.marker[0].repeat(t.length)) {
        if (fence.info === 'design-rules') readRules(fence.lines.join('\n'), current?.heading, ruleSource, design.rules, err);
        fence = undefined;
      } else fence.lines.push(line);
      continue;
    }
    const f = FENCE.exec(line);
    if (f) {
      fence = { marker: f[1], info: f[2], lines: [] };
      continue;
    }
    const h = /^## (.+?)\s*$/.exec(line);
    if (h) {
      closeSection();
      current = { heading: h[1], lines: [] };
      continue;
    }
    current?.lines.push(line);
  }
  if (fence) err(`a fenced block opened with ${fence.marker} is never closed`);
  closeSection();

  // Section order: every format section once, in order. Unknown headings are kept, as the
  // format allows.
  const seen = new Set<string>();
  let last = -1;
  for (const s of design.sections) {
    const index = (SECTIONS as readonly string[]).indexOf(s.heading);
    if (index === -1) continue;
    if (seen.has(s.heading)) err(`section "## ${s.heading}" appears twice`);
    else if (index < last) err(`section "## ${s.heading}" is out of the format's order`);
    seen.add(s.heading);
    last = Math.max(last, index);
  }
  for (const name of SECTIONS) if (!seen.has(name)) err(`section "## ${name}" is missing`);

  // Rationale: each token named in bold in its group's section.
  const prose = (heading: string) =>
    design.sections.filter((s) => s.heading === heading).map((s) => s.prose).join('\n');
  for (const [group, heading] of Object.entries(GROUP_SECTIONS)) {
    const tokens = design.front[group];
    if (!isRecord(tokens)) continue;
    const text = prose(heading);
    for (const name of Object.keys(tokens)) {
      if (name.endsWith('-dark') && name.slice(0, -5) in tokens) continue;
      if (!hasRationale(text, name)) err(`token "${name}" has no bold rationale in ## ${heading}`);
    }
  }
  if (design.rules.elevation) {
    const text = prose('Elevation & Depth');
    for (const level of Object.keys(design.rules.elevation)) {
      if (!hasRationale(text, level)) err(`elevation "${level}" has no bold rationale in ## Elevation & Depth`);
    }
  }
  return design;
}

function readRules(
  yaml: string,
  heading: string | undefined,
  source: Record<string, string>,
  rules: Rules,
  err: (what: string) => void,
): void {
  let block: unknown;
  try {
    block = parseYaml(yaml);
  } catch (e) {
    err(`a design-rules block is not valid YAML: ${(e as Error).message.split('\n')[0]}`);
    return;
  }
  if (!isRecord(block)) {
    err(`a design-rules block in ## ${heading ?? '(none)'} is not a mapping`);
    return;
  }
  for (const [key, value] of Object.entries(block)) {
    const home = RULE_SECTIONS[key];
    if (!home) {
      err(`design-rules key "${key}" is unknown`);
      continue;
    }
    if (key in source) {
      err(`design-rules key "${key}" appears in two blocks`);
      continue;
    }
    if (heading !== home) {
      err(`design-rules key "${key}" must be in ## ${home}, not ## ${heading ?? '(none)'}`);
      continue;
    }
    source[key] = home;
    (rules as Record<string, unknown>)[key] = value;
  }
}
