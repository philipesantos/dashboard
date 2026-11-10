import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseDesign } from '../src/parse.ts';
import { generateCss } from '../src/generate.ts';

const fixture = (name: string) => readFileSync(new URL(`./fixtures/generate/${name}`, import.meta.url), 'utf8');
const design = fixture('design.md');

test('the generated CSS matches the golden file: @theme inline, :root and .dark in role order', () => {
  const d = parseDesign(design);
  assert.deepEqual(d.errors, []);
  assert.equal(generateCss(d), fixture('expected.css'));
});

test('the radius steps derive from --radius with the fixed multipliers', () => {
  const css = generateCss(parseDesign(design));
  const steps = [...css.matchAll(/--radius-(\w+): (.+);/g)].map((m) => `${m[1]}=${m[2]}`);
  assert.deepEqual(steps, [
    'sm=calc(var(--radius) * 0.6)',
    'md=calc(var(--radius) * 0.8)',
    'lg=var(--radius)',
    'xl=calc(var(--radius) * 1.4)',
    '2xl=calc(var(--radius) * 1.8)',
    '3xl=calc(var(--radius) * 2.2)',
    '4xl=calc(var(--radius) * 2.6)',
  ]);
});

test('changing rounded.base changes only --radius', () => {
  const before = generateCss(parseDesign(design)).split('\n');
  const after = generateCss(parseDesign(design.replace('base: 0.625rem', 'base: 0.375rem'))).split('\n');
  const changed = after.filter((line, i) => line !== before[i]);
  assert.deepEqual(changed, ['    --radius: 0.375rem;']);
});

test('changing spacing.unit changes only --spacing', () => {
  const before = generateCss(parseDesign(design)).split('\n');
  const after = generateCss(parseDesign(design.replace('unit: 0.25rem', 'unit: 0.3rem'))).split('\n');
  assert.deepEqual(after.filter((line, i) => line !== before[i]), ['    --spacing: 0.3rem;']);
});

test('CRLF input generates the same LF output', () => {
  assert.equal(generateCss(parseDesign(design.replace(/\n/g, '\r\n'))), fixture('expected.css'));
});
