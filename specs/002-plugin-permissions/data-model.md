# Data Model: Plugin Permissions

Host types and records of this feature. Feature 001's [data model](../001-plugin-runtime/data-model.md)
still holds for limits, render reports, log messages and its error kinds.

## Plugin package

One binary WebAssembly component file ([contracts/package.md](contracts/package.md)).

| Field | Rule |
| --- | --- |
| bytes | At most `max_package_bytes` (10 MiB, feature 001) |
| manifest section | Exactly one top-level custom section `dashboard-manifest`, at most 16 KiB |
| digest | `sha256:` + hex SHA-256 of the whole file |
| contract | `v0` or `v1`, from the version of its exported `dashboard:widget/renderer` |

## Manifest

Published as `wit/manifest.schema.json` ([draft](contracts/manifest.schema.json)). Every field
is required and no other field is accepted, except `$schema`, which the host ignores.

| Field | Rule |
| --- | --- |
| `manifest_version` | `1` |
| `id` | `github.com/<owner>/<repository>` or `local/<name>`, lowercase ([research R3](research.md#r3-plugin-ids)) |
| `version` | SemVer 2.0.0, at most 64 characters |
| `author` | 1–100 characters, no control characters (`char::is_control`: C0, DEL and C1) |
| `permissions` | At most 32 entries of `{ "name": <permission>, "required": true \| false }`; a name is `[a-z][a-z0-9-]{0,31}` |
| `login_providers` | At most 8, unique, each one of `github` |

Rules the schema checks too: types, patterns, lengths, unknown fields. Rules only the host
checks, after the schema:

- A permission name the host does not know is `unknown-permission`, naming it (FR-008).
- The same permission twice with the same `required` counts once; with different values it is
  `invalid-manifest`.
- A contract v0 plugin with an optional permission is `optional-needs-v1` (FR-013).

Text fields are untrusted: every display escapes them as feature 001 escapes log messages.

## Permission

| Field | Meaning |
| --- | --- |
| name | The manifest's name for it |
| required | `true`: approved with the other required permissions, or the plugin does not run; `false`: optional, off until the user turns it on |
| interface | The contract interface that carries it, from the registry |

### Permission registry

The permissions this host knows, each tied to one interface per contract version. Features
003–005 add rows.

| Permission | Contract v0 | Contract v1 |
| --- | --- | --- |
| `log` | `dashboard:widget/log@0.1.0` | `dashboard:widget/log@1.0.0` |

## Grant record

The development record: `development-grants.json`, in the data directory or at `--grants`
([research R4](research.md#r4-the-grant-record)). Host data: plugins have no file access, and
only the CLI changes it.

```json
{
  "record_version": 1,
  "plugins": {
    "local/hello-widget": {
      "granted": ["log"],
      "approved_version": "0.2.0",
      "approved_digest": "sha256:3f1c…"
    }
  }
}
```

| Field | Rule |
| --- | --- |
| `record_version` | `1`; a newer value makes the record unreadable to this host |
| `plugins` | Map from plugin id to its entry; a plugin with no entry has no approval |
| `granted` | Sorted, unique permission names: the required permissions approved plus the optional ones turned on. A name the host does not know is kept and never linked |
| `approved_version`, `approved_digest` | The package last approved, for display only |

Changes, each under the record's lock and written atomically:

| Change | Effect on the entry |
| --- | --- |
| Approve (CLI `approve`) | Created if missing; `granted` = (`granted` ∩ declared) ∪ required; version and digest updated |
| Turn an optional permission on or off (CLI `optional`) | Adds or removes that one name; refused for a required one |
| Remove (CLI `remove`) | Entry deleted |
| Load of a package that no longer declares a granted permission | That name removed (FR-012) |

A record that is missing counts as empty. One that cannot be read, is not JSON, fails these
rules or has a newer `record_version` grants nothing, and no change is written over it.

### Grant store

The runtime's view of one record: the record, where it lives (a file, or memory in tests) and a
generation counter that every change increments.

## Load decision

`Runtime::load(package, grant store)`. Each step refuses with its kind and stops; a step that
finds several offending names lists them all.

| Step | Check | Refusal |
| --- | --- | --- |
| 1 | Size and component preamble (feature 001) | `invalid-package` |
| 2 | Exactly one top-level manifest section | `no-manifest`, `manifest-mismatch` |
| 3 | Size, JSON, schema rules, then known permissions | `invalid-manifest`, `unknown-permission` |
| 4 | Compile (feature 001) | `invalid-package` |
| 5 | Contract version of the `renderer` export; every `dashboard:widget` import from that same version; no optional permission in v0 | `contract-mismatch`, `optional-needs-v1` |
| 6 | Every import belongs to a registered interface, and its permission is declared | `capability-refused`, `not-declared` |
| 7 | The plugin has an entry, and every required permission is granted | `not-granted` (no entry or unreadable record), `needs-new-approval` (entry, missing permissions) |
| 8 | Granted permissions no longer declared leave the entry | — |
| 9 | Link: the real interface for each granted permission the component imports; the stand-in for each optional one that is off | `contract-mismatch` if linking still fails |

### Combinations (SC-002)

Every combination of declared, granted and imported for one permission, as steps 6–9 decide
it. Each row assumes the plugin has an approval (an entry in the record); a plugin without one
is `not-granted` whatever it declares.

| Declared | Granted | Imported | Outcome |
| --- | --- | --- | --- |
| no | no | no | Runs; nothing linked |
| no | yes, left from an earlier version | no | Runs; nothing linked; the grant leaves the entry (step 8) |
| no | no | yes | `not-declared` |
| no | yes, left from an earlier version | yes | `not-declared` |
| required | yes | yes | Runs; the real interface |
| required | yes | no | Runs; nothing linked |
| required | no | yes | `needs-new-approval` |
| required | no | no | `needs-new-approval` |
| optional | on | yes | Runs; the real interface |
| optional | on | no | Runs; nothing linked |
| optional | off | yes | Runs; the stand-in |
| optional | off | no | Runs; nothing linked |

## Plugin

| Field | Meaning |
| --- | --- |
| component | Compiled once at load and kept |
| manifest | As read at load |
| contract | `v0` or `v1` |
| grant store | The one it was loaded with |
| linked | The prepared instance (`instantiate_pre`) and the generation it was built for, or the refusal that generation produced |

Before every call, if the store's generation differs from `linked`'s, the plugin re-runs steps
7–9 and replaces `linked`. A refusal there makes that call's `RenderReport` an error with the
refusal's kind, and nothing is instantiated. A call already running keeps its instance and ends
within its time limit (FR-014, research R6).

## Stand-in

One host type per capability interface of contract v1, separate from the real implementation.
Every function returns that interface's `not-granted` case and has no other effect: for `log`,
`write` returns `err(not-granted)` and records nothing (FR-019).

## Runtime error

New kinds, in addition to feature 001's ([contracts/cli.md](contracts/cli.md#error-kinds)).
Details are escaped and cut to 1 KiB, as in feature 001; every name in a list is escaped.

| Kind | Source |
| --- | --- |
| `no-manifest` | Load, step 2 |
| `manifest-mismatch` | Load, step 2; `pack` on a component with a manifest |
| `invalid-manifest` | Load, step 3; `pack` |
| `unknown-permission` | Load, step 3; `pack`; `optional` |
| `optional-needs-v1` | Load, step 5; `optional` on a v0 plugin |
| `not-declared` | Load, step 6; `optional` on a permission not in the manifest |
| `not-granted` | Load or call, step 7; `optional` before `approve` |
| `needs-new-approval` | Load or call, step 7 |
| `required-together` | `optional` on a required permission |

## Hostile-plugin cases

New cases of `runtime/tests/hostile.rs`, one per rule of FR-016. Each must be refused with its
kind (or, for FR-019, contained; for FR-009 and FR-010, leave the record unchanged), and a
well-behaved plugin must run right after on the same runtime. `probe` (v1, calls `log` and
returns what it answered) and `quiet` (v1, no imports) are the bare fixtures under
`plugins/fixtures/`; each case packs them with its own manifest.

| Case | Rule | Package | Grants | Expected |
| --- | --- | --- | --- | --- |
| `no-manifest` | FR-001 | `probe`, no manifest section | — | `no-manifest` |
| `invalid-manifest` | FR-003 | `probe` + a manifest with an unknown field, and one with an uppercase id | — | `invalid-manifest`, naming the field |
| `two-manifests` | FR-004 | `probe` + a manifest without permissions + a second one requesting `log` | — | `manifest-mismatch` |
| `nested-manifest` | FR-004 | A component whose only manifest section sits in a nested module | — | `manifest-mismatch` |
| `undeclared-capability` | FR-006 | `probe` + a manifest without `log` | Approved | `not-declared`, naming `log` |
| `ungranted-capability` | FR-007 | `probe` + `log` required | None | `not-granted`, naming `log` |
| `unknown-permission` | FR-008 | `quiet` + a permission `telepathy` | — | `unknown-permission`, naming it |
| `escalating-update` | FR-011 | `probe` + `log` optional, approved; then `probe` + `log` required, same id | Approval of the first | Second refused `needs-new-approval`, naming `log`; the entry unchanged |
| `optional-off` | FR-019 | `probe` + `log` optional | Approved, `log` off | Runs; the probe reports `not-granted`; the report holds no log message |
| `grant-record-untouched` | FR-009, FR-010 | Every case above and `probe`, run against a file-backed record | Approved as each case needs | The record's bytes and the store's generation are the same before and after every render; reading the record stays impossible (`read-file`, feature 001) |

## Performance record

`specs/002-plugin-permissions/performance.md`: for each operating system, `checks_us` and
`relink_us` from `runtime bench`, the commit measured, and the verdict against SC-007 (under
10 ms).
