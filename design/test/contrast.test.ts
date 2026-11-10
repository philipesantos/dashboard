import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseDesign } from '../src/parse.ts';
import { contrastLines, MINIMUM } from '../src/checks/contrast.ts';

const fixture = readFileSync(new URL('./fixtures/contrast/design.md', import.meta.url), 'utf8');
const failures = (text: string) => contrastLines(parseDesign(text)).filter((l) => !l.startsWith('info: '));

function edit(text: string, from: string, to: string): string {
  assert.equal(text.split(from).length, 2, `fixture must contain "${from}" exactly once`);
  return text.replace(from, to);
}

test('each kind has its minimum ratio', () => {
  assert.deepEqual(MINIMUM, { text: 4.5, 'large-text': 3, boundary: 3, focus: 3, chart: 3 });
});

test('the fixture passes in both themes, and every pairing is listed with both ratios', () => {
  const lines = contrastLines(parseDesign(fixture));
  assert.deepEqual(lines.filter((l) => !l.startsWith('info: ')), []);
  assert.ok(lines.some((l) => /^info: DESIGN\.md: chart-2 on card \(chart\): light [\d.]+, dark [\d.]+$/.test(l)), lines.join('\n'));
});

test('a failing pairing names the pairing, the theme and both ratios', () => {
  const text = edit(fixture, 'chart-2: "oklch(0.58 0.10 200)"', 'chart-2: "oklch(0.68 0.1 200)"');
  assert.deepEqual(failures(text), ['DESIGN.md: pairing chart-2 on card (chart, light): 2.76 < 3.00']);
});

test('a layer stack is composited before measuring', () => {
  const text = edit(fixture, 'destructive: "oklch(0.54 0.2 27)"', 'destructive: "oklch(0.56 0.2 27)"');
  assert.deepEqual(failures(text), ['DESIGN.md: pairing destructive on card + destructive/10 (text, light): 4.42 < 4.50']);
});

test('a role in no pairing and not decorative fails', () => {
  const text = edit(fixture, '  - role: border\n    reason: Dividers whose meaning the layout carries.\n', '  []\n').replace('decorative:\n  []\n', '');
  assert.deepEqual(failures(text), ['DESIGN.md: role "border" is in no pairing and not decorative']);
});

test('a decorative role cannot be the fg of a boundary or focus pairing', () => {
  const text = edit(fixture, '  - { fg: chart-2, on: [card], kind: chart }\n', '  - { fg: chart-2, on: [card], kind: chart }\n  - { fg: border, on: [card], kind: boundary }\n');
  assert.ok(failures(text).includes('DESIGN.md: role "border" is decorative and cannot be the fg of a boundary pairing'), failures(text).join('\n'));
});

test('a stack whose first layer is translucent fails', () => {
  const text = edit(fixture, '  - { fg: chart-2, on: [card], kind: chart }\n', '  - { fg: chart-2, on: [card], kind: chart }\n  - { fg: foreground, on: [input], kind: text }\n');
  assert.deepEqual(failures(text), ['DESIGN.md: pairing foreground on input (text, dark): the first layer "input" is not opaque']);
});

test('a pairing limited to one theme is measured in that theme only', () => {
  const text = edit(fixture, '  - { fg: chart-2, on: [card], kind: chart }\n', '  - { fg: chart-2, on: [card], kind: chart }\n  - { fg: foreground, on: [card, input/80], kind: text, themes: [dark] }\n');
  const lines = contrastLines(parseDesign(text));
  assert.deepEqual(lines.filter((l) => !l.startsWith('info: ')), []);
  assert.ok(lines.some((l) => /^info: DESIGN\.md: foreground on card \+ input\/80 \(text\): dark [\d.]+$/.test(l)), lines.join('\n'));
});
