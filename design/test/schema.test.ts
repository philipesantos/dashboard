import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseDesign } from '../src/parse.ts';
import { schemaErrors, splitLayer } from '../src/checks/schema.ts';

const base = readFileSync(new URL('./fixtures/generate/design.md', import.meta.url), 'utf8');
const errors = (text: string) => schemaErrors(parseDesign(text));

test('the generator fixture has a valid schema', () => {
  assert.deepEqual(errors(base), []);
});

test('a role without its -dark twin is named', () => {
  const text = base.replace('  primary-dark: "oklch(0.74 0.12 235)"\n', '');
  assert.deepEqual(errors(text), ['DESIGN.md: color "primary" has no "primary-dark"']);
});

test('a value that is not oklch() is named', () => {
  const text = base.replace('primary: "oklch(0.5 0.13 245)"', 'primary: "#0068a7"');
  assert.deepEqual(errors(text), ['DESIGN.md: color "primary" (light) is not an oklch() value: #0068a7']);
});

test('names in design-rules must resolve', () => {
  const text = base.replace(
    '## Elevation & Depth\n',
    '## Elevation & Depth\n',
  ).replace(
    '- **primary**: the accent.\n',
    '- **primary**: the accent.\n\n```design-rules\nhostOnly: [frame]\npairings:\n  - { fg: primary, on: [nope], kind: text }\n  - { fg: primary/150, on: [background], kind: huge }\n```\n',
  );
  assert.deepEqual(errors(text), [
    'DESIGN.md: hostOnly entry "frame" matches no color role',
    'DESIGN.md: pairing 1 (primary on nope) names "nope", which is not a color role',
    'DESIGN.md: pairing 2 (primary/150 on background): "primary/150" is not a role or role/opacity',
    'DESIGN.md: pairing 2 (primary/150 on background): kind "huge" is not one of text, large-text, boundary, focus, chart',
  ]);
});

test('role/opacity splits into a role and a fraction', () => {
  assert.deepEqual(splitLayer('input/30'), { role: 'input', opacity: 0.3 });
  assert.deepEqual(splitLayer('ring'), { role: 'ring', opacity: 1 });
  assert.equal(splitLayer('ring/0'), undefined);
});
