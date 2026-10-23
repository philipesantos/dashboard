# Contract: `runtime` command-line tool

Extends [feature 001's contract](../../001-plugin-runtime/contracts/cli.md). The tool is the
host's control in this phase: it works in development mode only, on local package files, and
keeps its decisions in the development grant record, as Principle VII allows until the host
dialogs exist (constitution v1.3.0). It is a binary of the `runtime` crate:
`cargo run -p runtime --release -- <command>`.

## Common rules

- Every command except `pack` takes `--grants <path>`: the grant record to use. Without it,
  the record is `<data dir>/dev.philsantos.dashboard/development-grants.json`
  ([research R4](../research.md#r4-the-grant-record)); if the system has no data directory, it
  is a usage error.
- `<plugin>` is a package file. The tool reads at most the package limit plus one byte, as in
  feature 001.
- Text that comes from a package (id, author, versions, permission names, plugin messages) is
  escaped before it is printed, so it cannot add lines or control the terminal.
- On failure, stderr gets `error: <kind>: <detail>` and the exit status is 1; usage errors exit
  with 2 and print the usage.

## `runtime run <plugin> [--config <json> | --config-file <path>] [--grants <path>]`

As in feature 001, after the manifest and grant checks of
[data-model.md](../data-model.md#load-decision). A plugin without an approval is refused as
`not-granted`. Calls to an optional capability that is off answer `not-granted` inside the
plugin; the tool prints nothing for them.

## `runtime bench <plugin> [--iterations <n>] [--config …] [--grants <path>]`

As in feature 001, with two more lines on stdout:

```text
checks_us: <find and parse the manifest, read the grants, check imports and grants; no compile>
relink_us: <link again after a grant change>
```

## `runtime permissions <plugin> [--grants <path>]`

Prints what the plugin requests and what is granted (FR-014), and changes nothing.

```text
plugin: local/hello-widget 0.2.0
author: Dashboard contributors
contract: v1
approved: version 0.2.0 (sha256:3f1c…)
permissions:
  log  required  granted
login providers: none
```

`approved` reads `no` when the plugin has never been approved. Each permission line is
`<name> required|optional granted|not granted`. Declared login providers are listed with
`(connecting accounts arrives in feature 005)`. A package that fails the manifest checks is
refused with its kind, as in `run`.

## `runtime approve <plugin> [--grants <path>]`

Runs load steps 1–6 of [data-model.md](../data-model.md#load-decision) first: a package refused
there is refused with that kind, and nothing is granted. Then grants every required permission
the manifest declares, all at once (FR-018), and records the version and digest. Optional permissions are left as they are; permissions this manifest no
longer declares are removed (FR-012). Prints `approved <id> <version>` and one
`granted: <permission>` line per required permission. There is no way to approve only some
required permissions.

## `runtime optional <plugin> <permission> on|off [--grants <path>]`

Turns one optional permission on or off. Refused when:

| Case | Kind |
| --- | --- |
| The plugin has no approval | `not-granted` (the detail says to run `approve` first) |
| The permission is required in this manifest | `required-together` |
| The manifest does not declare it | `not-declared` |
| The host does not know it | `unknown-permission` |
| The plugin is contract v0 | `optional-needs-v1` |

Prints `<permission> on` or `<permission> off`.

## `runtime remove <plugin-id> [--grants <path>]`

Removes the plugin's approval and every grant with it. Prints `removed <id>`, or
`<id> has no approval` with exit status 0 when there was nothing to remove. A malformed id is a
usage error.

## `runtime pack <component> --manifest <file> --output <file>`

Checks the manifest (schema, id, version, known permissions) and writes the component with the
manifest section added at the end ([package.md](package.md)). Refused with `invalid-manifest`,
`unknown-permission`, or `manifest-mismatch` when the component already carries a manifest.
Does not compile the component and needs no grant record. Prints the output path and its digest.

## Exit status

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | The plugin or the change was refused, or the plugin failed or was stopped |
| 2 | Usage error: unknown command or option, missing file, invalid configuration JSON, no data directory |

## Error kinds

Feature 001's kinds keep their meaning; `contract-mismatch` now also covers an import from a
contract newer than the host's. New kinds, one per rule of FR-015:

| Kind | When |
| --- | --- |
| `no-manifest` | The package has no manifest section |
| `manifest-mismatch` | Two or more manifest sections, one inside a nested part of the package, or `pack` on a component that already has one |
| `invalid-manifest` | The manifest is too large, not JSON, or breaks the schema; the detail names the field and the problem |
| `unknown-permission` | The manifest requests a permission this host does not know; the detail names it |
| `not-declared` | The component imports a capability its manifest does not declare; the detail lists every one |
| `not-granted` | The plugin has no approval, or the grant record cannot be read; the detail lists the required permissions not granted, and the reason the record was not read |
| `needs-new-approval` | An earlier version was approved and this one requires permissions not granted; the detail lists them |
| `required-together` | `optional … off` (or `on`) on a required permission: required permissions change only together |
| `optional-needs-v1` | A contract v0 plugin marks a permission optional, or `optional` targets a v0 plugin |
