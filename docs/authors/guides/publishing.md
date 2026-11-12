# Publishing

People install a widget in the app from its repository on GitHub: they type its address, and the
app fetches the latest release, checks it, compiles it apart from itself, and shows them what it
asks for before anything of it runs. This guide is what a release needs for that to work.

## The repository

- On `github.com`, public, and one widget per repository.
- The manifest's `id` is the repository, in lowercase: a widget published from
  `github.com/Ada/Notes-Widget` declares `"id": "github.com/ada/notes-widget"`. Any other id is
  refused as [`identity-mismatch`](../reference/errors.md#identity-mismatch).
- A renamed repository is a new widget. The app takes updates only from the repository it
  installed from, found by its permanent identifier, which survives renames; once renamed, the
  installed widget gets no more updates
  ([`repository-changed`](../reference/errors.md#repository-changed)), and people install it again
  from its new address.

## The release

- The latest release that is neither a draft nor a prerelease is the one the app installs.
- Its tag is `v<version>` or `<version>`, where `<version>` is the manifest's `version`: the
  release of version `1.2.0` is tagged `v1.2.0` or `1.2.0`. Anything else is
  [`tag-mismatch`](../reference/errors.md#tag-mismatch).
- It carries exactly one asset whose name ends in `.wasm`: the package, as
  [the walkthrough](../getting-started.md) builds it, with its manifest inside, at most 10 MiB
  ([limits](../reference/limits.md)). None, two, or a larger one is
  [`no-package-asset`](../reference/errors.md#no-package-asset).
- Never move a tag or replace a release's package. The app pins a release by its package's
  digest the first time it sees it; the same tag with other bytes is
  [`changed-release`](../reference/errors.md#changed-release). Publish a new version instead.

## Connectors

A widget installed from a repository may declare connectors of the official catalog only. One
from a repository, or a `local/` one, is refused at installation, even when it is optional:
connectors from repositories arrive with a later item, which verifies that a service's owner
authorized them. Develop against a `local/` connector with the development CLI; publish once the
connector is in the catalog.

## What the person sees

The installation dialog names the widget by its manifest `name` and its repository id, and shows
the `author` as the manifest declares it. It describes each permission in a sentence the app
writes, and each connector with its destinations and the operations the widget calls, what they
send, and whether each is a read or a change, as the catalog reviewed it. Optional permissions
and connectors start off; the person turns on the ones they want. When the grants together let
data read as a connected account reach another destination, the dialog says so.

## Updates

People check an installed widget for a newer release from its frame. The app offers one only
for a higher version, by Semantic Versioning, from the same repository; an older or equal one is
[`not-newer`](../reference/errors.md#not-newer). An update that asks for nothing new is only
confirmed. One that asks for a new permission or connector, a larger storage quota, or changes
what an operation sends or where, goes through the full dialog again, every grant listed and the
changes marked; until it is approved, the installed version keeps running.

## Compiling

The app compiles every installed package in a separate process, under limits on time, CPU and
memory, confined so that it can open no file and reach no network
([limits](../reference/limits.md#installing)). A package past a limit is refused as
[`compile-time-limit`](../reference/errors.md#compile-time-limit) or
[`compile-memory-limit`](../reference/errors.md#compile-memory-limit). An ordinary widget compiles
in well under a second.
