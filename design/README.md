# design

The host's visual system, generated from [`DESIGN.md`](../DESIGN.md) at the repository root.
Decisions are in [specs/007-host-visual-design/research.md](../specs/007-host-visual-design/research.md).

| Directory | What it holds |
| --- | --- |
| `radix/3.0.0/` | Radix Colors data, the source of every color |
| `format/0.4.0/` | the DESIGN.md format specification the checker follows |
| `fonts/inter/` | Inter, the host's built-in font |

## Vendored files

Each file is the upstream file, unmodified. Text files are hashed after turning `\r\n` into `\n`;
fonts are hashed as bytes. `design-gen`'s `pins` test checks every hash below, so a change to any
of these files is a deliberate, reviewed upgrade.

### Radix Colors 3.0.0 (MIT)

From [radix-ui/colors](https://github.com/radix-ui/colors) at commit
`c4d0e50006c71ec51c4d6ff349062ab261b57e67`, the commit of the npm release 3.0.0, `src/` and the
root `LICENSE`. The host compiles these values in, so it must ship this license.

| File | SHA-256 |
| --- | --- |
| `radix/3.0.0/light.ts` | `1a9725486609d36b469caa3b84b8de4348b7f2f05d45905127637b988329acbd` |
| `radix/3.0.0/dark.ts` | `d8599fe69a4454340b2bc82e003a210caa6b79082be9a1b7f13429ff03826c68` |
| `radix/3.0.0/blackA.ts` | `930205059fbb077241db60753dc338d2354c85fe7ce86ccbccc734836e3371fb` |
| `radix/3.0.0/LICENSE` | `9c400881b9139571aa7acc3bd6e422a4f05223b4519169fb661298fb44bf228e` |

### DESIGN.md format 0.4.0 (Apache-2.0)

From [google-labs-code/design.md](https://github.com/google-labs-code/design.md) at tag `0.4.0`
(commit `9bf8eae67128b6cc55ad9bf86665767deb4c11cd`), `docs/spec.md` and the root `LICENSE`.

| File | SHA-256 |
| --- | --- |
| `format/0.4.0/spec.md` | `8f297a7bd38cf08bde06c96f5eea2ebfcd9930a8bc1dfe964f8e8dec894e691a` |
| `format/0.4.0/LICENSE` | `1bdd03995456d191707e379eb06ceac20cf30d520bec90c97c2b6d220906c66e` |

### Inter 4.1 (SIL Open Font License 1.1)

From the [Inter 4.1 release](https://github.com/rsms/inter/releases/tag/v4.1) (2024-11-16),
`Inter-4.1.zip` (SHA-256 `9883fdd4a49d4fb66bd8177ba6625ef9a64aa45899767dde3d36aa425756b11e`):
`InterVariable.ttf`, `web/InterVariable.woff2` and `LICENSE.txt`. Not subset or renamed, so the
license's Reserved Font Name clause never applies. The host must ship the license with the font.

| File | SHA-256 |
| --- | --- |
| `fonts/inter/InterVariable.ttf` | `4989b125924991b90d05b2d16e0e388c48f7d5bb8b30539bbf9c755278d0ccaf` |
| `fonts/inter/InterVariable.woff2` | `693b77d4f32ee9b8bfc995589b5fad5e99adf2832738661f5402f9978429a8e3` |
| `fonts/inter/LICENSE.txt` | `262481e844521b326f5ecd053e59b98c8b2da78c8ee1bdbb6e8174305e54935a` |
