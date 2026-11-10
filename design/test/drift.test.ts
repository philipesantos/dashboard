import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseDesign } from '../src/parse.ts';
import { generateCss } from '../src/generate.ts';
import { compareCss } from '../src/checks/drift.ts';

const design = parseDesign(readFileSync(new URL('./fixtures/generate/design.md', import.meta.url), 'utf8'));
const expected = generateCss(design);
const FILE = 'app/src/theme.generated.css';

test('a committed file equal to the generated one passes', () => {
  assert.deepEqual(compareCss(FILE, expected, expected), []);
});

test('a committed file that differs only in CRLF passes', () => {
  assert.deepEqual(compareCss(FILE, expected.replace(/\n/g, '\r\n'), expected), []);
});

test('every differing variable is named, one line each', () => {
  const committed = expected
    .replace('--primary: oklch(0.5 0.13 245);', '--primary: oklch(0.5 0.13 200);')
    .replace('--background: oklch(0.17 0.012 255);', '--background: oklch(0.1 0 0);');
  assert.deepEqual(compareCss(FILE, committed, expected), [
    `${FILE}: --primary differs from DESIGN.md in :root (run pnpm --filter ./design gen)`,
    `${FILE}: --background differs from DESIGN.md in .dark (run pnpm --filter ./design gen)`,
  ]);
});

test('a missing and an extra variable are named', () => {
  const committed = expected.replace('    --spacing: 0.25rem;\n', '    --spacing-x: 1rem;\n');
  assert.deepEqual(compareCss(FILE, committed, expected), [
    `${FILE}: --spacing is missing from @theme inline (run pnpm --filter ./design gen)`,
    `${FILE}: --spacing-x in @theme inline is not in DESIGN.md (run pnpm --filter ./design gen)`,
  ]);
});

test('a change outside any variable is still caught', () => {
  const committed = expected.replace('Do not edit.', 'Edited.');
  assert.deepEqual(compareCss(FILE, committed, expected), [
    `${FILE}: differs from what DESIGN.md generates (run pnpm --filter ./design gen)`,
  ]);
});

test('a missing file is named', () => {
  assert.deepEqual(compareCss(FILE, undefined, expected), [
    `${FILE}: missing (run pnpm --filter ./design gen)`,
  ]);
});
