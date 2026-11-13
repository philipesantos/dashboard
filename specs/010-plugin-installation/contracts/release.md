# Contract: What the App Installs

This is the author-facing contract; `docs/authors/guides/publishing.md` restates it.

## The repository

- On `github.com`, public, one plugin per repository.
- The manifest `id` is `github.com/<owner>/<repo>`, lowercase, exactly the repository's name.
- A renamed repository is a new plugin: people install it again under the new id.

## The release

- The latest release that is neither a draft nor a prerelease is the one installed.
- Its tag is `v<version>` or `<version>`, and `<version>` equals the manifest's `version`.
- It carries exactly one asset whose name ends in `.wasm`: the package built by
  `cargo xtask plugins` or the author tooling, at most 10 MiB, with the `dashboard-manifest`
  section (`runtime/src/package.rs:13`).
- A tag is never moved or re-uploaded with other bytes: the app pins the first bytes it saw.
  Publish a new version instead.

## Updates

- The app offers an update only for a release with a higher version, from the repository with the
  permanent identifier it recorded.
- An update that asks for nothing new is confirmed by the person; one that asks for more, or
  changes a connector operation it calls, is approved again in the dialog.

## The compile process

The app starts its own executable as `<app> --compile-package`, writes the package to its
standard input and reads the compiled form from its standard output. The child confines itself
before reading the package (research R13): it cannot open a file or a socket, or reach the
keychain or another process. Its limits are in `data-model.md`; reaching one refuses the
installation with `compile-time-limit` or `compile-memory-limit`.
