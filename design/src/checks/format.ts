// Runs the format's own checker, @google/design.md at the version pinned in package.json
// (FR-001, research R1). Its errors fail; its warnings print and do not.

import { execFile } from 'node:child_process';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import type { Context } from '../cli.ts';

const run = promisify(execFile);

/** The checker's CLI, resolved from this package's dependencies, never from npx or PATH. */
const CLI = fileURLToPath(import.meta.resolve('@google/design.md'));

interface Finding {
  severity: string;
  message: string;
  rule?: string;
  path?: string;
}

/** Maps the checker's JSON report to failure lines and `warning:` lines. Infos are dropped. */
export function formatLines(label: string, report: { findings?: Finding[] }): string[] {
  const lines: string[] = [];
  // Every role that no format component references draws one orphaned-tokens warning. Host
  // surfaces and status roles are used through design-rules instead, so these are expected and
  // print as one line (research R1).
  const orphans = (report.findings ?? []).filter((f) => f.severity === 'warning' && f.rule === 'orphaned-tokens');
  if (orphans.length > 0) {
    const names = orphans.map((f) => (f.path ?? '').replace(/^colors\./, '')).join(', ');
    lines.push(`warning: ${label}: ${orphans.length} tokens no format component references: ${names} (format: orphaned-tokens)`);
  }
  for (const f of report.findings ?? []) {
    if (f.rule === 'orphaned-tokens') continue;
    const where = f.path ? `${f.path}: ` : '';
    const line = `${label}: ${where}${f.message} (format: ${f.rule ?? 'unknown'})`;
    if (f.severity === 'error') lines.push(line);
    else if (f.severity === 'warning') lines.push(`warning: ${line}`);
  }
  return lines;
}

/** Runs the pinned checker on DESIGN.md text. */
export async function runFormatChecker(text: string, label = 'DESIGN.md'): Promise<string[]> {
  const dir = mkdtempSync(join(tmpdir(), 'design-format-'));
  const file = join(dir, 'DESIGN.md');
  try {
    writeFileSync(file, text);
    let stdout: string;
    try {
      ({ stdout } = await run(process.execPath, [CLI, 'lint', '--format', 'json', file]));
    } catch (e) {
      // The checker exits 1 when it finds an error, and still prints its report.
      const out = (e as { stdout?: string }).stdout;
      if (!out) return [`${label}: the format checker failed: ${(e as Error).message.split('\n')[0]}`];
      stdout = out;
    }
    let report: { findings?: Finding[] };
    try {
      report = JSON.parse(stdout);
    } catch {
      return [`${label}: the format checker printed something other than its JSON report`];
    }
    return formatLines(label, report);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

export function checkFormat(ctx: Context): Promise<string[]> {
  return runFormatChecker(ctx.source);
}
