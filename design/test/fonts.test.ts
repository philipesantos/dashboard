import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { checkFontLicenses, licenseFile } from '../src/checks/fonts.ts';

const root = (name: string) => fileURLToPath(new URL(`./fixtures/fonts/${name}/`, import.meta.url));
const typography = {
  'body-sm': { fontFamily: 'Figtree Variable, ui-sans-serif, sans-serif' },
  mono: { fontFamily: 'IBM Plex Mono, monospace' },
};

test('a family maps to its license file by name, without "Variable"', () => {
  assert.equal(licenseFile('Figtree Variable, ui-sans-serif'), 'figtree.txt');
  assert.equal(licenseFile('Bricolage Grotesque Variable'), 'bricolage-grotesque.txt');
  assert.equal(licenseFile("'IBM Plex Mono', monospace"), 'ibm-plex-mono.txt');
});

test('passes when every family has its license text', () => {
  assert.deepEqual(checkFontLicenses(root('with'), typography), []);
});

test('fails naming the family whose license text is missing', () => {
  assert.deepEqual(checkFontLicenses(root('without'), typography), [
    'app/public/licenses/fonts/ibm-plex-mono.txt: missing the license of "IBM Plex Mono", named in DESIGN.md\'s typography',
  ]);
});
