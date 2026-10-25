# Data Model: Contract v1 Closed and Docs for Plugin Authors

No new record in the host. The entities below live in the repository's files: the docs, the
contract's history and the feature's own records. Formats are in
[contracts/docs-format.md](contracts/docs-format.md).

## Published contract version

| Field | Meaning |
| --- | --- |
| version | `1.0.0` … `1.3.0` (or `2.0.0` if a breaking fix is chosen, spec FR-006) |
| file | `wit/history/<version>/widget.wit`, never edited |
| digest | SHA-256 of the file, pinned in `runtime/tests/contract_v1.rs` |

Rules: a version is frozen once; while `wit/v1` (or `wit/v2`) declares a frozen version, its bytes
equal the frozen copy; each new minor only adds to the one before it (the only-adds tests).

## Review finding (`specs/006-contract-v1-docs/review.md`)

| Field | Meaning |
| --- | --- |
| id | `F<n>` |
| item | the contract item, world, name or doc page it concerns |
| what | what is wrong: unreachable, behaves otherwise than documented, undocumented, promises what the host does not do |
| checked | how it was reproduced at the source (file and line, a test, a run) |
| outcome | `docs`, `host` (with the test that fails without the fix), `breaking` (the maintainer chose 2.0.0), `kept` (with the reason) |
| cost | for a breaking finding: what changes in the contract, host, examples, fixtures and docs |

States: `open` → one outcome. The contract is frozen only when no finding is `open`.

## Reference entry

| Field | Meaning |
| --- | --- |
| kind | `wit`, `error`, `manifest`, `connector`, `limit` |
| id | the item's path (see the marker format) |
| text | what it is, the minor it arrived in, when it happens, what a widget should do |
| default | limits only: the default in the reference's units, equal to `Limits::default()` |

Identity: (kind, id) is unique across `docs/authors/reference/`. The set of entries equals the
set the drift test derives from the sources (R7).

## Runnable block and page session

| Field | Meaning |
| --- | --- |
| page | a file under `docs/authors/` whose blocks run, in order, as one session |
| block | a fenced block marked `check`: commands with their whole output, or a file to write |
| os | the systems the block runs on (all by default) |
| exit | the exit code each command in the block must return (0 by default) |
| output | whole (default) or `any` for blocks without a `runtime` command (cargo, git) |
| service | page-level: the test services the page needs (`github`, `notes`); the page's `runtime` commands then run through the test-only CLI |
| placeholders | `<digest>`, `<path>`, `<n>`, `<time>`, `<version>` |

Session state: a fresh temporary folder as the working directory and a cleared development data
directory (or, for the test-only CLI, a data directory of the session's own), kept across the
page's blocks, dropped at its end, with the page's accounts disconnected.

## Walk (`specs/006-contract-v1-docs/walks.md`)

| Field | Meaning |
| --- | --- |
| who | the agent (in a container) or the maintainer |
| system | Linux for both; the three systems are walked by CI |
| date, docs commit | what was walked |
| stuck points | each place the walker needed more than the docs, and the fix |
| time | the maintainer's walk only, without tool downloads |
