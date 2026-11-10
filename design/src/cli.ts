// pnpm --filter ./design <gen | check | preview | licenses>
// (specs/007-host-visual-system/contracts/outputs.md, "Commands").
//
// Paths resolve from the repository root, two levels above this file, never from the working
// directory, so the scripts behave the same from any directory.

import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseDesign, type Design } from './parse.ts';
import { generateCss, THEME_FILE } from './generate.ts';

export const ROOT = fileURLToPath(new URL('../../', import.meta.url));

export interface Context {
  root: string;
  design: Design;
}

/** A check returns its failure lines, each prefixed with the file it is about. */
export type Check = (ctx: Context) => string[] | Promise<string[]>;

/** Every check `check` runs, in order (contracts/outputs.md). */
const CHECKS: [string, Check][] = [['project layer', (ctx) => ctx.design.errors]];

export function load(root = ROOT): Context {
  const source = readFileSync(join(root, 'DESIGN.md'), 'utf8');
  return { root, design: parseDesign(source) };
}

async function runChecks(ctx: Context): Promise<number> {
  let failures = 0;
  for (const [name, check] of CHECKS) {
    const lines = await check(ctx);
    for (const line of lines) {
      if (line.startsWith('warning: ')) console.warn(line);
      else {
        console.error(line);
        failures++;
      }
    }
    if (!lines.some((l) => !l.startsWith('warning: '))) console.log(`ok: ${name}`);
  }
  return failures;
}

async function main(command: string | undefined): Promise<number> {
  switch (command) {
    case 'check': {
      const failures = await runChecks(load());
      if (failures > 0) console.error(`${failures} failure(s)`);
      return failures > 0 ? 1 : 0;
    }
    case 'gen': {
      const ctx = load();
      if (ctx.design.errors.length > 0) {
        for (const line of ctx.design.errors) console.error(line);
        return 1;
      }
      writeFileSync(join(ctx.root, THEME_FILE), generateCss(ctx.design));
      console.log(`wrote ${THEME_FILE}`);
      return 0;
    }
    case 'preview':
    case 'licenses':
      console.error(`${command}: not implemented yet`);
      return 1;
    default:
      console.error('usage: cli.ts <gen | check | preview | licenses>');
      return 2;
  }
}

if (import.meta.main) process.exitCode = await main(process.argv[2]);
