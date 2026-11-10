import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { parseDesign } from '../src/parse.ts';
import { scanUsages, usageLines } from '../src/checks/usage.ts';

const design = parseDesign(readFileSync(new URL('./fixtures/contrast/design.md', import.meta.url), 'utf8'));
const root = fileURLToPath(new URL('./fixtures/usage/', import.meta.url));

test('the scan reads roles, opacities and variants from .tsx and @apply lines', () => {
  const found = scanUsages(root, ['foreground', 'card', 'input', 'ring', 'destructive', 'secondary']).map((u) => `${u.file} ${u.utility} ${u.role ?? '-'} ${u.opacity}`);
  assert.deepEqual(found, [
    'app/src/index.css outline-ring/50 ring 50',
    'app/src/widget.tsx bg-card card 100',
    'app/src/widget.tsx text-foreground foreground 100',
    'app/src/widget.tsx ring-ring/50 ring 50',
    'app/src/widget.tsx dark:hover:bg-input/50 input 50',
    'app/src/widget.tsx bg-black/10 - 10',
    'app/src/widget.tsx text-red-500 - 100',
    'app/src/widget.tsx bg-[#fff] - 100',
    'app/src/widget.tsx hover:bg-[color-mix(in_oklch,var(--secondary),var(--foreground)_5%)] secondary 100',
    'app/src/widget.tsx hover:bg-[color-mix(in_oklch,var(--secondary),var(--foreground)_5%)] foreground 5',
  ]);
});

test('each undeclared or role-less color names the file and the utility', () => {
  assert.deepEqual(usageLines(root, design), [
    'app/src/index.css: outline-ring/50 names "ring", which is not a color role',
    'app/src/widget.tsx: ring-ring/50 names "ring", which is not a color role',
    'app/src/widget.tsx: dark:hover:bg-input/50 is not declared (input is declared at 100 only)',
    'app/src/widget.tsx: bg-black/10 names no color role',
    'app/src/widget.tsx: text-red-500 names no color role',
    'app/src/widget.tsx: bg-[#fff] names no color role',
    'app/src/widget.tsx: hover:bg-[color-mix(in_oklch,var(--secondary),var(--foreground)_5%)] names "secondary", which is not a color role',
    'app/src/widget.tsx: hover:bg-[color-mix(in_oklch,var(--secondary),var(--foreground)_5%)] is not declared (foreground is declared at 100 only)',
  ]);
});

test('a decorative entry with an opacity covers that opacity only', () => {
  const text = readFileSync(new URL('./fixtures/contrast/design.md', import.meta.url), 'utf8').replace(
    '  - role: border\n',
    '  - role: input/50\n    reason: A tint nothing is drawn on.\n  - role: border\n',
  );
  const lines = usageLines(root, parseDesign(text));
  assert.ok(!lines.some((l) => l.includes('bg-input/50')), lines.join('\n'));
});
