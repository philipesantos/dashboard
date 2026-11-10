// Fails when a committed generated file differs from what DESIGN.md generates now (FR-005).

import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Context } from '../cli.ts';
import { generateCss, THEME_FILE } from '../generate.ts';

const HINT = '(run pnpm --filter ./design gen)';

/** Reads `selector { --name: value; … }` blocks into a map per selector, in order. */
function variables(css: string): Map<string, Map<string, string>> {
  const blocks = new Map<string, Map<string, string>>();
  let current: Map<string, string> | undefined;
  for (const line of css.split('\n')) {
    const open = /^(\S.*?)\s*\{\s*$/.exec(line);
    if (open) {
      current = new Map();
      blocks.set(open[1], current);
      continue;
    }
    if (/^\}\s*$/.test(line)) {
      current = undefined;
      continue;
    }
    const v = /^\s*(--[\w-]+):\s*(.*?);\s*$/.exec(line);
    if (v && current) current.set(v[1], v[2]);
  }
  return blocks;
}

/** Compares a committed CSS file with the expected one, naming every variable that differs. */
export function compareCss(file: string, committed: string | undefined, expected: string): string[] {
  if (committed === undefined) return [`${file}: missing ${HINT}`];
  const actual = committed.replace(/\r\n/g, '\n');
  if (actual === expected) return [];

  const want = variables(expected);
  const have = variables(actual);
  const lines: string[] = [];
  for (const [block, vars] of want) {
    const got = have.get(block) ?? new Map<string, string>();
    for (const [name, value] of vars) {
      if (!got.has(name)) lines.push(`${file}: ${name} is missing from ${block} ${HINT}`);
      else if (got.get(name) !== value) lines.push(`${file}: ${name} differs from DESIGN.md in ${block} ${HINT}`);
    }
  }
  for (const [block, vars] of have) {
    const wanted = want.get(block);
    for (const name of vars.keys()) {
      if (!wanted?.has(name)) lines.push(`${file}: ${name} in ${block} is not in DESIGN.md ${HINT}`);
    }
  }
  if (lines.length === 0) lines.push(`${file}: differs from what DESIGN.md generates ${HINT}`);
  return lines;
}

function read(path: string): string | undefined {
  return existsSync(path) ? readFileSync(path, 'utf8') : undefined;
}

export function checkDrift(ctx: Context): string[] {
  return compareCss(THEME_FILE, read(join(ctx.root, THEME_FILE)), generateCss(ctx.design));
}
