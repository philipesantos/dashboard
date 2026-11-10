import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { ALLOWED, allowed, installedPackages, licenseLines } from '../src/licenses.ts';

const store = fileURLToPath(new URL('./fixtures/licenses/node_modules/.pnpm', import.meta.url));

test('the allowlist is deny.toml\'s plus the npm licenses research R13 adds', () => {
  for (const id of ['Apache-2.0', 'MIT', 'ISC', 'BSD-3-Clause', 'MPL-2.0', 'BSD-2-Clause', '0BSD', 'BlueOak-1.0.0', 'CC0-1.0', 'OFL-1.1']) {
    assert.ok(ALLOWED.has(id), id);
  }
  assert.ok(!ALLOWED.has('GPL-3.0-only'));
});

test('SPDX expressions: OR passes when one side is allowed, AND needs both', () => {
  assert.equal(allowed('MIT'), true);
  assert.equal(allowed('(MIT OR GPL-3.0)'), true);
  assert.equal(allowed('Apache-2.0 AND BSD-2-Clause'), true);
  assert.equal(allowed('MIT AND CC-BY-4.0'), false);
  assert.equal(allowed('Apache-2.0 WITH LLVM-exception'), true);
  assert.equal(allowed('GPL-3.0-only'), false);
});

test('the store is read package by package, scoped ones included', () => {
  assert.deepEqual(
    installedPackages(store).map((p) => `${p.name}@${p.version} ${p.license ?? '-'}`),
    [
      '@scope/c@3.0.0 Apache-2.0 AND BSD-2-Clause',
      'a@1.0.0 MIT',
      'b@2.0.0 (MIT OR GPL-3.0)',
      'd@1.0.0 GPL-3.0-only',
      'e@1.0.0 MIT',
      'f@1.0.0 -',
      'g@1.0.0 MIT AND CC-BY-4.0',
    ],
  );
});

test('each package outside the allowlist is named with its license', () => {
  assert.deepEqual(licenseLines(store), [
    'node_modules: d@1.0.0 is under "GPL-3.0-only", which is not on the allowlist',
    'node_modules: f@1.0.0 declares no license',
    'node_modules: g@1.0.0 is under "MIT AND CC-BY-4.0", which is not on the allowlist',
  ]);
});

test('a named exception holds only while the package declares exactly what it names', () => {
  const exceptions = [
    { name: 'f', declared: undefined, reason: 'fixture' },
    { name: 'g', declared: 'MIT AND CC-BY-4.0', reason: 'fixture' },
    { name: 'd', declared: 'GPL-2.0-only', reason: 'a different license than d declares' },
  ];
  assert.deepEqual(licenseLines(store, exceptions), [
    'node_modules: d@1.0.0 is under "GPL-3.0-only", which is not on the allowlist',
  ]);
});
