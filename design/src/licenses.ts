// The npm license allowlist (research R13): every installed package's license must be one an
// Apache-2.0 client may ship or build with. `pnpm licenses` printed nothing on pnpm 12.14.0, so
// this reads the packages in pnpm's store directly.

import { existsSync, lstatSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

/** deny.toml's list, plus the licenses common in npm that research R13 adds. */
export const ALLOWED = new Set([
  // deny.toml
  'Apache-2.0',
  'Apache-2.0 WITH LLVM-exception',
  'BSD-3-Clause',
  'CDLA-Permissive-2.0',
  'ISC',
  'MIT',
  'MPL-2.0',
  'Unicode-3.0',
  'Zlib',
  // research R13
  'BSD-2-Clause',
  '0BSD',
  'BlueOak-1.0.0',
  'CC0-1.0',
  'OFL-1.1',
]);

/**
 * Packages accepted by name, each decided by the maintainer (2026-11-10). An exception holds only
 * while the package declares exactly `declared` (undefined: no license field), so a change of
 * license fails again.
 */
export interface LicenseException {
  name: string;
  version?: string;
  declared: string | undefined;
  reason: string;
}

export const EXCEPTIONS: LicenseException[] = [
  {
    name: '@google/design.md',
    version: '0.4.0',
    declared: undefined,
    reason: 'The format checker. Its package.json has no license field, but the package ships the Apache-2.0 text, as its repository does.',
  },
  {
    name: 'caniuse-lite',
    declared: 'CC-BY-4.0',
    reason: 'Browser support data that browserslist reads at build time, through the shadcn CLI and Babel. It never ships in app/dist.',
  },
];

/** True when an SPDX expression is allowed: one side of an OR, every side of an AND. */
export function allowed(expression: string): boolean {
  const expr = expression.trim().replace(/^\((.*)\)$/, '$1');
  const alternatives = expr.split(/\s+OR\s+/);
  return alternatives.some((alt) =>
    alt
      .replace(/[()]/g, '')
      .split(/\s+AND\s+/)
      .every((id) => ALLOWED.has(id.trim())),
  );
}

export interface InstalledPackage {
  name: string;
  version: string;
  license?: string;
}

function licenseOf(pkg: Record<string, unknown>): string | undefined {
  if (typeof pkg.license === 'string') return pkg.license;
  const typed = (v: unknown) => (typeof v === 'object' && v !== null ? (v as { type?: unknown }).type : undefined);
  if (typeof typed(pkg.license) === 'string') return typed(pkg.license) as string;
  if (Array.isArray(pkg.licenses)) {
    const types = pkg.licenses.map(typed).filter((t): t is string => typeof t === 'string');
    if (types.length > 0) return types.join(' OR ');
  }
  return undefined;
}

function isRealDir(path: string): boolean {
  try {
    const s = lstatSync(path);
    return s.isDirectory() && !s.isSymbolicLink();
  } catch {
    return false;
  }
}

/** Every package in a pnpm store (`node_modules/.pnpm`), each read once. */
export function installedPackages(store: string): InstalledPackage[] {
  const found = new Map<string, InstalledPackage>();
  if (!existsSync(store)) return [];
  for (const entry of readdirSync(store)) {
    const modules = join(store, entry, 'node_modules');
    if (entry === 'node_modules' || !isRealDir(modules)) continue;
    const dirs: string[] = [];
    for (const name of readdirSync(modules)) {
      const path = join(modules, name);
      if (name.startsWith('@') && isRealDir(path)) {
        for (const sub of readdirSync(path)) dirs.push(join(path, sub));
      } else dirs.push(path);
    }
    for (const dir of dirs) {
      const manifest = join(dir, 'package.json');
      if (!isRealDir(dir) || !existsSync(manifest)) continue;
      const pkg = JSON.parse(readFileSync(manifest, 'utf8')) as Record<string, unknown>;
      if (typeof pkg.name !== 'string') continue;
      const p = { name: pkg.name, version: String(pkg.version ?? ''), license: licenseOf(pkg) };
      found.set(`${p.name}@${p.version}`, p);
    }
  }
  return [...found.values()].sort((a, b) => `${a.name}@${a.version}`.localeCompare(`${b.name}@${b.version}`, 'en'));
}

export function licenseLines(store: string, exceptions: LicenseException[] = EXCEPTIONS): string[] {
  const out: string[] = [];
  for (const p of installedPackages(store)) {
    const id = `${p.name}@${p.version}`;
    const excepted = exceptions.some(
      (e) => e.name === p.name && (e.version === undefined || e.version === p.version) && e.declared === p.license,
    );
    if (excepted) continue;
    if (p.license === undefined) out.push(`node_modules: ${id} declares no license`);
    else if (!allowed(p.license)) out.push(`node_modules: ${id} is under "${p.license}", which is not on the allowlist`);
  }
  return out;
}
