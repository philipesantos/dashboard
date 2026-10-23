# Contract: plugin package

A plugin package is one binary WebAssembly component file. It is the unit the host loads, the
CLI takes and, from Phase 3, the release asset installed from the author's repository.

## Manifest section

| Property | Value |
| --- | --- |
| Where | A custom section (id 0) at the top level of the component, not inside a nested core module or component |
| Name | `dashboard-manifest` |
| Payload | The manifest: a UTF-8 JSON document, at most 16 KiB, valid against [manifest.schema.json](manifest.schema.json) |
| Count | Exactly one in the package |
| Position | Anywhere among the top-level sections; `runtime pack` and the xtask append it at the end |

The host refuses:

| Package | Kind |
| --- | --- |
| No `dashboard-manifest` section anywhere | `no-manifest` |
| Two or more at the top level, or one inside a nested module or component | `manifest-mismatch` |
| A section over 16 KiB, not UTF-8, not JSON, or not valid against the schema | `invalid-manifest` |

Nothing else in the package is read as the manifest: no file next to it, no export, no other
section name.

## Digest

`sha256:<64 lowercase hex digits>`: the SHA-256 of the whole file, the manifest section
included. The grant record shows the digest last approved; Phase 3 pins it at installation, and
Phase 6 signs the same bytes.

## Manifest

```json
{
  "manifest_version": 1,
  "id": "local/hello-widget",
  "version": "0.2.0",
  "author": "Dashboard contributors",
  "permissions": [
    { "name": "log", "required": true }
  ],
  "login_providers": []
}
```

The fields and their rules are in [data-model.md](../data-model.md#manifest); the schema is
published as `wit/manifest.schema.json`.

## Making a package

- In this repository: `cargo xtask plugins` adds each crate's `manifest.json` (beside its
  `Cargo.toml`) to its component.
- Elsewhere: `runtime pack <component> --manifest <file> --output <file>`
  ([cli.md](cli.md#runtime-pack)) checks the manifest and adds the section. It refuses a
  component that already has one.
