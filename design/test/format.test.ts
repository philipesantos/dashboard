import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { formatLines, runFormatChecker } from '../src/checks/format.ts';

const fixture = (name: string) => readFileSync(new URL(`./fixtures/format/${name}`, import.meta.url), 'utf8');

test('errors fail and warnings print with "warning:"; infos are dropped', () => {
  const lines = formatLines('DESIGN.md', {
    findings: [
      { severity: 'error', message: 'Reference {rounded.nope} does not resolve', rule: 'broken-ref', path: 'components.button-primary.rounded' },
      { severity: 'warning', message: 'Section order', rule: 'section-order' },
      { severity: 'info', message: 'Summary', rule: 'token-summary' },
    ],
  });
  assert.deepEqual(lines, [
    'DESIGN.md: components.button-primary.rounded: Reference {rounded.nope} does not resolve (format: broken-ref)',
    'warning: DESIGN.md: Section order (format: section-order)',
  ]);
});

test('the pinned checker fails a broken reference', async () => {
  const lines = await runFormatChecker(fixture('broken-ref.md'));
  const errors = lines.filter((l) => !l.startsWith('warning: '));
  assert.ok(errors.length > 0, lines.join('\n'));
  assert.ok(errors.some((l) => l.includes('broken-ref')), errors.join('\n'));
});

test('the probe of research R1 passes with warnings only', async () => {
  const lines = await runFormatChecker(fixture('probe.md'));
  assert.ok(lines.length > 0, 'the probe should draw warnings');
  assert.deepEqual(lines.filter((l) => !l.startsWith('warning: ')), []);
});
