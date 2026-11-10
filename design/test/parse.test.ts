import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseDesign } from '../src/parse.ts';

const valid = readFileSync(new URL('./fixtures/parse/valid.md', import.meta.url), 'utf8');

/** Replaces exactly one occurrence, so a fixture edit cannot silently miss. */
function edit(text: string, from: string, to: string): string {
  assert.equal(text.split(from).length, 2, `fixture must contain "${from}" exactly once`);
  return text.replace(from, to);
}

test('the valid fixture parses with no error', () => {
  const d = parseDesign(valid);
  assert.deepEqual(d.errors, []);
  assert.deepEqual(
    d.colors.map((c) => [c.name, c.light, c.dark]),
    [
      ['background', 'oklch(0.99 0.003 250)', 'oklch(0.17 0.012 255)'],
      ['foreground', 'oklch(0.19 0.02 255)', 'oklch(0.96 0.006 250)'],
      ['card', 'oklch(1 0 0)', 'oklch(0.21 0.014 255)'],
    ],
  );
  assert.deepEqual(d.front.rounded, { base: '0.625rem' });
  assert.deepEqual(d.front.spacing, { unit: '0.25rem' });
  assert.equal((d.front.typography as Record<string, { fontFamily: string }>)['body-md'].fontFamily, 'Figtree Variable');
});

test('design-rules blocks are read from the section they sit in', () => {
  const d = parseDesign(valid);
  assert.deepEqual(d.rules.hostOnly, ['card']);
  assert.deepEqual(d.rules.pairings, [{ fg: 'foreground', on: ['background'], kind: 'text' }]);
  assert.deepEqual(d.rules.elevation, { flat: { shadow: 'none', dark: 'none' } });
  assert.equal(d.rules.unknownTone, 'default');
});

test('a key in two design-rules blocks is an error', () => {
  const text = edit(valid, 'unknownTone: default\n', 'unknownTone: default\nhostOnly: [card]\n');
  const errors = parseDesign(text).errors;
  assert.ok(errors.includes('DESIGN.md: design-rules key "hostOnly" appears in two blocks'), errors.join('\n'));
});

test('a design-rules key outside its section is an error', () => {
  const text = edit(valid, 'unknownTone: default\n', 'unknownTone: default\ndecorative: []\n');
  assert.deepEqual(parseDesign(text).errors, [
    'DESIGN.md: design-rules key "decorative" must be in ## Colors, not ## Components',
  ]);
});

test('a token without a bold rationale is reported', () => {
  const text = edit(valid, '- **card**: panels.\n', '- card: panels.\n');
  assert.deepEqual(parseDesign(text).errors, ['DESIGN.md: token "card" has no bold rationale in ## Colors']);
});

test('a bold name with nothing after it is not a rationale', () => {
  const text = edit(valid, '- **card**: panels.\n', '- **card**\n');
  assert.deepEqual(parseDesign(text).errors, ['DESIGN.md: token "card" has no bold rationale in ## Colors']);
});

test('a -dark color without its light role is reported', () => {
  const text = edit(valid, '  card-dark:', '  panel-dark: "oklch(0.2 0 0)"\n  card-dark:');
  assert.ok(parseDesign(text).errors.includes('DESIGN.md: color "panel-dark" has no light role "panel"'));
});

test('a section out of the format order is an error', () => {
  const typography = '## Typography\n\n- **body-md**: body text.\n\n';
  const moved = edit(edit(valid, typography, ''), '## Elevation & Depth\n', `${typography}## Elevation & Depth\n`);
  assert.deepEqual(parseDesign(moved).errors, [
    'DESIGN.md: section "## Typography" is out of the format\'s order',
  ]);
});

test('a duplicate section heading is an error', () => {
  const text = edit(valid, "## Do's and Don'ts\n", "## Colors\n\nMore.\n\n## Do's and Don'ts\n");
  assert.ok(parseDesign(text).errors.includes('DESIGN.md: section "## Colors" appears twice'));
});

test('a missing section is an error', () => {
  const text = edit(valid, '## Overview\n\nA fixture.\n\n', '');
  assert.ok(parseDesign(text).errors.includes('DESIGN.md: section "## Overview" is missing'));
});

test('CRLF line endings parse identically', () => {
  const lf = parseDesign(valid);
  const crlf = parseDesign(valid.replace(/\n/g, '\r\n'));
  assert.deepEqual(crlf, lf);
});
