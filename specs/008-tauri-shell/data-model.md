# Data Model: The Desktop Shell

There is no runtime storage. The entities are configuration and reports. File formats are in
[contracts/](contracts/).

## Content policy

| Field | Rule |
| --- | --- |
| `build` | `release` or `development`. |
| `directives` | Directive name → sources. The release values are fixed in [contracts/content-policy.md](contracts/content-policy.md). |
| `allowances` | Each source beyond `'self'`, `'none'` or the host's channel, with its reason. |

**Validation**:
- The release policy never contains `'unsafe-eval'`, `data:`, `blob:`, `http:`, `https:`, `*`, or
  a host name.
- It contains no `'unsafe-inline'`. Run-time `<style>` elements carry Tauri's per-response nonce
  (research R3); a new allowance would need a recorded reason and a probe case showing it.
- The development policy differs from the release one only by the entries the contract lists.

## Surface entry

One thing the window may call.

| Field | Rule |
| --- | --- |
| `kind` | `permission`, `command` or `plugin`. |
| `name` | A permission id (`core:event:allow-listen`), an app command (`list_widgets`), or a plugin crate (`tauri-plugin-dialog`). |
| `reason` | Required, non-empty: why the host UI needs it. |

**Validation**: the set of entries equals what the app grants, by kind (FR-006). An entry listed
and not granted, or granted and not listed, is a failure.

## Window settings

The engine-level protections applied when the window is built.

| Setting | Release value | Engines |
| --- | --- | --- |
| navigation | own origin only (`devUrl` too in debug) | all |
| new windows | denied | all |
| developer tools | off | all |
| default context menu | off | all |
| incognito data store | on | all |
| general autofill | off | WebView2 |
| password autosave | off | WebView2 |

## Probe case

| Field | Rule |
| --- | --- |
| `id` | Kebab-case, unique, such as `inline-script-attribute`. |
| `rule` | The FR it breaks: FR-001, FR-007, FR-009 to FR-012, FR-016 or FR-021. |
| `detect` | One or more of the contract's kinds: `violation` (the engine reported a policy violation), `absent` (the side effect did not happen), `error` (the call rejected), `handler` (a window handler refused it), `stayed` (the page and the single window remained), `setting` (an engine setting reads as the release value). |
| `result` | `held` or `broken`, with a detail. |

**Validation**: the run passes only when every case is `held`, the report arrived within 60 s,
and no navigation or window was allowed.

## Engine trial

Decision record 0002 records it.

| Field | Rule |
| --- | --- |
| `runtime` | The crate and version tried. |
| `kind` | `desk` (evidence only) or `build`. |
| `requirements` | For each of FR-001 to FR-012 and FR-016: met, not met or not reached, with evidence. |
| `size`, `startup` | For a `build` trial only: installer size and cold start per system. |
| `outcome` | `adopt` or `fallback`, and when the next trial is due. |

## WebKitGTK register entry

A change made only because Linux uses WebKitGTK (FR-022). Format in
[contracts/webkitgtk-register.md](contracts/webkitgtk-register.md).

| Field | Rule |
| --- | --- |
| `id` | Kebab-case, unique. The marker `WEBKITGTK-ONLY(<id>)` names it. |
| `files` | Each exists and holds the marker. |
| `reason` | Required, non-empty. |
| `removal` | Required, non-empty: delete, or replace with what. |

**Validation**: every marker has an entry, and every entry has a marker in each file it names.
**Lifecycle**: added with the change; removed with it when Linux moves to the embedded engine.
