# design

The host's visual system, generated from [`DESIGN.md`](../DESIGN.md) at the repository root.
Decisions are in [specs/007-host-visual-design/research.md](../specs/007-host-visual-design/research.md).

| Directory | What it holds |
| --- | --- |
| `radix/3.0.0/` | Radix Colors data, the source of every color |
| `format/0.4.0/` | the DESIGN.md format specification the checker follows |
| `fonts/barlow/` | Barlow, the host's built-in font for text |
| `fonts/plex-mono/` | IBM Plex Mono, the host's built-in font for labels |

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

### Barlow 1.422 (SIL Open Font License 1.1)

From [jpt/barlow](https://github.com/jpt/barlow) at tag `1.422` (commit
`697600fcece1685b0ba409eb82868546def9a84e`): `fonts/ttf/` and `fonts/woff2/` for the Regular,
Medium and SemiBold weights, and the root `OFL.txt`. Not subset or renamed. The host must ship the
license with the font.

| File | SHA-256 |
| --- | --- |
| `fonts/barlow/Barlow-Regular.ttf` | `a72240db047b468c957834cdbd607f013da32edee62d096f5fe0a4c2dc8433ce` |
| `fonts/barlow/Barlow-Medium.ttf` | `5a26763b11d11a437ab9f1c46281d3615ab1f6b93c90b8fa181e03978971b9e9` |
| `fonts/barlow/Barlow-SemiBold.ttf` | `cafab6a24fc131fd2dbbe56b9cf8daa616db3d5929fa89c35acba690e620585e` |
| `fonts/barlow/Barlow-Regular.woff2` | `2a74a99f47e8234a4bf31a1e63b7bc2d8edf7ab069cd90b89fcd9c0bed7332f0` |
| `fonts/barlow/Barlow-Medium.woff2` | `8f6ce1b19d88ad3eb7b435758e111c3a7b295e15c6e631abc6b3057f4754f1ae` |
| `fonts/barlow/Barlow-SemiBold.woff2` | `fb08e02f3c06ae4749e5e8fe745ff46815b20169eaf295f11efdc41eb9e66c6e` |
| `fonts/barlow/OFL.txt` | `186d750eb496a4c17a76385f82be6aea2ac1cf2de074a811d63786cf374ea73f` |

### IBM Plex Mono 2.5.0 (SIL Open Font License 1.1)

From the [`@ibm/plex-mono@2.5.0` release](https://github.com/IBM/plex/releases/tag/%40ibm%2Fplex-mono%402.5.0)
(2026-06-11), `ibm-plex-mono.zip` (SHA-256
`6d23f01257663d8cc49a0d64c22ced630b79e0e2a0ac08a0da86e9a38bbc481c`): `fonts/complete/ttf/` and
`fonts/complete/woff2/` for the Regular and Medium weights, and `LICENSE.txt`. The license reserves
the name "Plex", so these files are never subset or modified, only copied. The host must ship the
license with the font.

| File | SHA-256 |
| --- | --- |
| `fonts/plex-mono/IBMPlexMono-Regular.ttf` | `7c6fbddca4b700be918f5f6183d9bd4464fa427fe435f0b480d77fe2bb8c5a43` |
| `fonts/plex-mono/IBMPlexMono-Medium.ttf` | `98fbd727aae340b236955879dabed4d991aac9e8e90b3b2a67ce4a59221cc97c` |
| `fonts/plex-mono/IBMPlexMono-Regular.woff2` | `ba204497f16b6d334cee9d1e963a831b73e3a56e1d6300a8489d18df7214b350` |
| `fonts/plex-mono/IBMPlexMono-Medium.woff2` | `33faf307fa6031fb4062276d7320a6d632de890cbb347576fd80cfa01077bc25` |
| `fonts/plex-mono/LICENSE.txt` | `37784b44044a4ffd9256702b7c0982c37e5c8887ba90c6dca0479aea93dc898d` |