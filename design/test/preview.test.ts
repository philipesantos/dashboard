import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseDesign } from '../src/parse.ts';
import { previewHtml } from '../src/preview.ts';

const design = parseDesign(readFileSync(new URL('../../DESIGN.md', import.meta.url), 'utf8'));
const html = previewHtml(design);

test('the page loads nothing: no script, no remote URL, no @import', () => {
  assert.ok(!/<script/i.test(html));
  assert.ok(!html.includes('http://'));
  assert.ok(!html.includes('https://'));
  assert.ok(!html.includes('@import'));
});

test('both themes, with every role and its pairings\' ratios', () => {
  assert.ok(html.includes('class="theme light"'));
  assert.ok(html.includes('class="theme dark"'));
  for (const c of design.colors) assert.ok(html.includes(`>${c.name}<`), c.name);
  assert.match(html, /chart-2 on card[^<]*<\/td><td[^>]*>\d+\.\d\d/);
});

test('the scales, the tones with their icons and every variant', () => {
  for (const step of ['sm', 'md', 'lg', 'xl', '2xl', '3xl', '4xl']) assert.ok(html.includes(`radius-${step}`), step);
  for (const name of ['body-xs', 'body-2xl', 'heading', 'mono', 'unit', 'grid-gap']) assert.ok(html.includes(name), name);
  for (const icon of ['circle-check', 'triangle-alert', 'circle-x']) assert.ok(html.includes(icon), icon);
  for (const v of Object.keys(design.rules.variants ?? {})) assert.ok(html.includes(v), v);
});

test('sketches of the chrome, a frame and a dialog over its scrim', () => {
  for (const part of ['sketch-chrome', 'sketch-frame', 'sketch-frame-header', 'sketch-scrim', 'sketch-dialog']) {
    assert.ok(html.includes(part), part);
  }
});
