// Every font family DESIGN.md names ships with its license text, as OFL-1.1 requires
// (FR-007, research R7).

import { existsSync } from 'node:fs';
import { join } from 'node:path';
import type { Context } from '../cli.ts';

export const LICENSE_DIR = 'app/public/licenses/fonts';

/** The first family of a stack, unquoted: `'IBM Plex Mono', monospace` → `IBM Plex Mono`. */
export function primaryFamily(stack: string): string {
  return stack.split(',')[0].trim().replace(/^['"]|['"]$/g, '');
}

/** `Figtree Variable` → `figtree.txt`: the @fontsource "Variable" suffix names a package, not a font. */
export function licenseFile(stack: string): string {
  const name = primaryFamily(stack).replace(/\s+Variable$/i, '');
  return `${name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}.txt`;
}

export function checkFontLicenses(root: string, typography: unknown): string[] {
  if (typeof typography !== 'object' || typography === null) return [];
  const families = new Map<string, string>();
  for (const token of Object.values(typography as Record<string, { fontFamily?: unknown }>)) {
    if (typeof token?.fontFamily === 'string') families.set(licenseFile(token.fontFamily), primaryFamily(token.fontFamily));
  }
  const lines: string[] = [];
  for (const [file, family] of families) {
    if (!existsSync(join(root, LICENSE_DIR, file))) {
      lines.push(`${LICENSE_DIR}/${file}: missing the license of "${family}", named in DESIGN.md's typography`);
    }
  }
  return lines;
}

export function checkFonts(ctx: Context): string[] {
  return checkFontLicenses(ctx.root, ctx.design.front.typography);
}
