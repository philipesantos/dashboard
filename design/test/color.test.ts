import { test } from 'node:test';
import assert from 'node:assert/strict';
import { contrast, flatten, oklchToRgba, parseOklch, toHex, withOpacity } from '../src/color.ts';

const ratio = (fg: string, bg: string) => contrast(oklchToRgba(fg), oklchToRgba(bg));
const near = (actual: number, expected: number) =>
  assert.ok(Math.abs(actual - expected) <= 0.01, `${actual.toFixed(3)} is not ${expected}`);

test('oklch converts to sRGB at the reference points', () => {
  assert.equal(toHex(oklchToRgba('oklch(1 0 0)')), '#ffffff');
  assert.equal(toHex(oklchToRgba('oklch(0 0 0)')), '#000000');
  const red = oklchToRgba('oklch(0.628 0.2577 29.23)');
  assert.ok(Math.abs(red.r - 1) <= 1 / 255);
  assert.ok(red.g <= 1 / 255);
  assert.ok(red.b <= 1 / 255);
});

test('oklch parses alpha, percent lightness, and rejects other syntax', () => {
  assert.deepEqual(parseOklch('oklch(1 0 0 / 9%)'), { l: 1, c: 0, h: 0, alpha: 0.09 });
  assert.equal(parseOklch('oklch(50% 0.1 200)')?.l, 0.5);
  assert.equal(parseOklch('#ffffff'), undefined);
  assert.equal(parseOklch('oklch(1 0 0 / 2)'), undefined);
});

test('a translucent color is composited over the surface beneath', () => {
  const surface = oklchToRgba('oklch(0.17 0.012 255)');
  const mixed = flatten([surface, oklchToRgba('oklch(1 0 0 / 9%)')]);
  assert.equal(mixed.a, 1);
  for (const k of ['r', 'g', 'b'] as const) {
    near(mixed[k], surface[k] * 0.91 + 0.09);
  }
});

test('the WCAG ratios research R4 measured', () => {
  near(ratio('oklch(0.19 0.02 255)', 'oklch(0.99 0.003 250)'), 17.94);
  near(ratio('oklch(0.96 0.006 250)', 'oklch(0.17 0.012 255)'), 17.03);
  near(ratio('oklch(0.60 0.02 250)', 'oklch(1 0 0)'), 3.94);
  near(ratio('oklch(0.68 0.1 200)', 'oklch(1 0 0)'), 2.76);
  const ring = withOpacity(oklchToRgba('oklch(0.62 0.12 240)'), 0.5);
  near(contrast(ring, oklchToRgba('oklch(0.99 0.003 250)')), 1.79);
});

test('a layer stack: destructive on destructive/10 over card', () => {
  const destructive = oklchToRgba('oklch(0.56 0.2 27)');
  const bg = flatten([oklchToRgba('oklch(1 0 0)'), withOpacity(destructive, 0.1)]);
  near(contrast(destructive, bg), 4.42);
});

test('a stack must start opaque', () => {
  assert.throws(() => flatten([oklchToRgba('oklch(1 0 0 / 50%)')]));
});
