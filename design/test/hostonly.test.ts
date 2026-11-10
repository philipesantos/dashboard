import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseDesign } from '../src/parse.ts';
import { hostOnlyLines, hostOnlyRoles } from '../src/checks/hostonly.ts';
import { generateWidgetMap } from '../src/generate.ts';

const fixture = readFileSync(new URL('./fixtures/hostonly/design.md', import.meta.url), 'utf8');
const lines = (text: string) => hostOnlyLines(parseDesign(text));

function edit(text: string, from: string, to: string): string {
  assert.equal(text.split(from).length, 2, `fixture must contain "${from}" exactly once`);
  return text.replace(from, to);
}

test('the fixture passes', () => {
  assert.deepEqual(parseDesign(fixture).errors, []);
  assert.deepEqual(lines(fixture), []);
});

test('a hostOnly entry covers every role that starts with it and a dash', () => {
  assert.deepEqual([...hostOnlyRoles(parseDesign(fixture))].sort(), ['frame-header', 'ring', 'sidebar', 'sidebar-border']);
});

test('a variant reaching a host-only role is named', () => {
  const text = edit(fixture, 'fill: secondary, rounded', 'fill: frame-header, rounded');
  assert.deepEqual(lines(text), ['DESIGN.md: variant button.secondary maps fill to host-only role "frame-header"']);
});

test('a tone reaching a host-only role is named', () => {
  const text = edit(fixture, 'muted: { text: muted-foreground }', 'muted: { text: sidebar-border }');
  assert.deepEqual(lines(text), ['DESIGN.md: tone muted maps text to host-only role "sidebar-border"']);
});

test('a variant never names the focus style', () => {
  const text = edit(fixture, 'fill: primary, rounded', 'fill: primary, border: ring, rounded');
  assert.deepEqual(lines(text), [
    'DESIGN.md: variant button.primary maps border to "ring", the focus style the host draws on every control',
  ]);
});

test('a status tone without an icon fails', () => {
  const text = edit(fixture, 'ok: { text: ok, icon: circle-check }', 'ok: { text: ok }');
  assert.deepEqual(lines(text), ['DESIGN.md: tone ok has no icon; the host draws one beside every status (FR-012)']);
});

test('a missing tone fails', () => {
  const text = edit(fixture, '  accent: { text: primary }\n', '');
  assert.deepEqual(lines(text), ['DESIGN.md: tone accent is missing']);
});

test('a level above flat must be host-only', () => {
  const text = edit(fixture, ', hostOnly: true }', ' }');
  assert.deepEqual(lines(text), ['DESIGN.md: elevation "raised" is above flat and must be host-only']);
});

test('a variant and its format component must agree', () => {
  const text = edit(fixture, 'button.secondary: { text: secondary-foreground', 'button.secondary: { text: foreground');
  assert.deepEqual(lines(text), [
    'DESIGN.md: variant button.secondary maps text to "foreground", but component button-secondary has textColor {colors.secondary-foreground}',
  ]);
});

test('the widget map holds role and token names only, and nothing host-only', () => {
  const map = generateWidgetMap(parseDesign(fixture));
  assert.equal(map.unknownTone, 'default');
  assert.deepEqual(map.tones.ok, { text: 'ok', icon: 'circle-check' });
  assert.deepEqual(map.variants['button.primary'], { text: 'primary-foreground', fill: 'primary', rounded: 'md', typography: 'body-sm' });
  assert.deepEqual(map.elevation, ['flat']);
  const json = JSON.stringify(map);
  assert.ok(!json.includes('oklch'), json);
  for (const role of ['frame-header', 'sidebar', 'sidebar-border', 'ring', 'raised']) {
    assert.ok(!json.includes(`"${role}"`), `${role} in ${json}`);
  }
});
