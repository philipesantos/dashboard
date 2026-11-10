import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { criteriaLines, parseCriteria } from '../src/checks/criteria.ts';

const read = (name: string) => readFileSync(new URL(`./fixtures/criteria/${name}`, import.meta.url), 'utf8');
const criteria = read('host-surfaces.md');
const threats = read('threat-model.md');

function edit(text: string, from: string, to: string): string {
  assert.equal(text.split(from).length, 2, `fixture must contain "${from}" exactly once`);
  return text.replace(from, to);
}

test('the fixtures pass; rows on other boundaries or phases are ignored', () => {
  assert.deepEqual(criteriaLines(criteria, threats), []);
});

test('criteria are read with their check and feature', () => {
  const list = parseCriteria(criteria).criteria;
  assert.deepEqual(list.map((c) => [c.id, c.check.split(':')[0], c.feature.split(' ')[0]]), [
    ['HS-01', 'hostile case', 'Moldura'],
    ['HS-02', 'test', 'Diálogo'],
    ['HS-03', 'review', 'Moldura'],
  ]);
});

test('a criterion without Check: or Feature: fails', () => {
  const noFeature = edit(criteria, 'Feature: Diálogo de instalação com permissões em linguagem clara\n', '');
  assert.deepEqual(criteriaLines(noFeature, threats), ['docs/design/host-surfaces.md: HS-02 has no "Feature:" line']);
  const noCheck = edit(criteria, 'Check: test: the dialog\'s layer is above the chrome.\n', '');
  assert.deepEqual(criteriaLines(noCheck, threats), ['docs/design/host-surfaces.md: HS-02 has no "Check:" line']);
});

test('a Check: that is not a test, a hostile case or a review fails', () => {
  const text = edit(criteria, 'Check: test: the dialog', 'Check: manual: the dialog');
  assert.deepEqual(criteriaLines(text, threats), [
    'docs/design/host-surfaces.md: HS-02 Check: must start with "test", "hostile case" or "review"',
  ]);
});

test('a duplicate id fails', () => {
  const text = edit(criteria, '### HS-03 The frame', '### HS-02 The frame');
  assert.ok(criteriaLines(text, threats).includes('docs/design/host-surfaces.md: HS-02 is defined twice'));
});

test('a B4 row in Phase 3 without a citation fails', () => {
  const text = edit(threats, 'V (HS-01) |', 'V |');
  assert.deepEqual(criteriaLines(criteria, text), [
    'docs/threat-model.md: row "Drawing over the frame" cites no HS- criterion',
  ]);
});

test('a citation of an unknown id fails', () => {
  const text = edit(threats, 'V (HS-01) |', 'V (HS-09) |');
  assert.deepEqual(criteriaLines(criteria, text), [
    'docs/threat-model.md: row "Drawing over the frame" cites HS-09, which docs/design/host-surfaces.md does not define',
  ]);
});

test('a cited criterion checked only by review fails', () => {
  const text = edit(threats, 'V (HS-01) |', 'V (HS-01, HS-03) |');
  assert.deepEqual(criteriaLines(criteria, text), [
    'docs/threat-model.md: row "Drawing over the frame" cites HS-03, whose Check: is a review; a cited criterion needs a test or a hostile case',
  ]);
});

test('CRLF files parse the same', () => {
  assert.deepEqual(criteriaLines(criteria.replace(/\n/g, '\r\n'), threats.replace(/\n/g, '\r\n')), []);
});
