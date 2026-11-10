// Criteria coverage: every frame and dialog criterion says how it is checked and which feature
// runs it, and every threat-model row on the plugin UI boundary (B4) in Phase 3 cites criteria
// that a test or a hostile case checks (FR-016, SC-005, research R10).

import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Context } from '../cli.ts';

export const CRITERIA_FILE = 'docs/design/host-surfaces.md';
export const THREAT_FILE = 'docs/threat-model.md';
const CHECK_KINDS = ['test', 'hostile case', 'review'];

export interface Criterion {
  id: string;
  title: string;
  check: string;
  feature: string;
}

export function parseCriteria(text: string): { criteria: Criterion[]; errors: string[] } {
  const criteria: Criterion[] = [];
  const errors: string[] = [];
  const seen = new Set<string>();
  let current: Criterion | undefined;
  const close = () => {
    if (!current) return;
    if (!current.check) errors.push(`${CRITERIA_FILE}: ${current.id} has no "Check:" line`);
    else if (!CHECK_KINDS.some((k) => current!.check.toLowerCase().startsWith(k))) {
      errors.push(`${CRITERIA_FILE}: ${current.id} Check: must start with "test", "hostile case" or "review"`);
    }
    if (!current.feature) errors.push(`${CRITERIA_FILE}: ${current.id} has no "Feature:" line`);
    criteria.push(current);
  };
  for (const line of text.replace(/\r\n/g, '\n').split('\n')) {
    const h = /^###\s+(HS-\d{2})\s+(.*)$/.exec(line);
    if (h) {
      close();
      if (seen.has(h[1])) errors.push(`${CRITERIA_FILE}: ${h[1]} is defined twice`);
      seen.add(h[1]);
      current = { id: h[1], title: h[2].trim(), check: '', feature: '' };
      continue;
    }
    if (/^#{1,3}\s/.test(line)) {
      close();
      current = undefined;
      continue;
    }
    if (!current) continue;
    const c = /^Check:\s*(.*)$/.exec(line);
    if (c) current.check = c[1].trim();
    const f = /^Feature:\s*(.*)$/.exec(line);
    if (f) current.feature = f[1].trim();
  }
  close();
  return { criteria, errors };
}

interface Row {
  threat: string;
  boundary: string;
  mitigation: string;
  phase: string;
}

/** The rows of the "Threats and planned tests" table. */
function threatRows(text: string): Row[] {
  const lines = text.replace(/\r\n/g, '\n').split('\n');
  const start = lines.findIndex((l) => /^##\s+Threats and planned tests/.test(l));
  if (start === -1) return [];
  const rows: Row[] = [];
  for (const line of lines.slice(start + 1)) {
    if (/^##\s/.test(line)) break;
    if (!line.startsWith('|')) continue;
    const cells = line.slice(1, line.lastIndexOf('|')).split('|').map((c) => c.trim());
    if (cells.length < 6 || cells[0] === 'Threat' || /^-+$/.test(cells[0])) continue;
    rows.push({ threat: cells[0], boundary: cells[3], mitigation: cells[4], phase: cells[5] });
  }
  return rows;
}

export function criteriaLines(criteriaText: string | undefined, threatText: string | undefined): string[] {
  if (criteriaText === undefined) return [`${CRITERIA_FILE}: missing`];
  if (threatText === undefined) return [`${THREAT_FILE}: missing`];
  const { criteria, errors } = parseCriteria(criteriaText);
  const out = [...errors];
  const byId = new Map(criteria.map((c) => [c.id, c]));
  for (const row of threatRows(threatText)) {
    if (!/\bB4\b/.test(row.boundary) || !/^Phase 3\b/.test(row.phase)) continue;
    const cited = /\(((?:HS-\d{2})(?:,\s*HS-\d{2})*)\)\s*$/.exec(row.mitigation);
    if (!cited) {
      out.push(`${THREAT_FILE}: row "${row.threat}" cites no HS- criterion`);
      continue;
    }
    for (const id of cited[1].split(/,\s*/)) {
      const c = byId.get(id);
      if (!c) out.push(`${THREAT_FILE}: row "${row.threat}" cites ${id}, which ${CRITERIA_FILE} does not define`);
      else if (c.check.toLowerCase().startsWith('review')) {
        out.push(`${THREAT_FILE}: row "${row.threat}" cites ${id}, whose Check: is a review; a cited criterion needs a test or a hostile case`);
      }
    }
  }
  return out;
}

function read(path: string): string | undefined {
  return existsSync(path) ? readFileSync(path, 'utf8') : undefined;
}

export function checkCriteria(ctx: Context): string[] {
  return criteriaLines(read(join(ctx.root, CRITERIA_FILE)), read(join(ctx.root, THREAT_FILE)));
}
