# Specification Quality Checklist: Plugin Storage

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-10-24
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- SQLite is named only in Assumptions, as the constitution's fixed technical decision; the
  requirements speak of named values, as the contract will. FR-004 states the store's hardening
  (no attaching, no extensions, most restrictive mode) as a security rule, because the gate case
  depends on it; how it is achieved is left to the plan.
- The numbers (host cap, cost per entry, name and value limits, calls and bytes per run, page
  size, file cap) are left to the plan and measured, as in features 001 and 003. SC-006's 1 ms per
  call is a target to confirm in `performance.md`.
- The description allowed names with path or command text to be either kept as is or refused; the
  spec chose to keep any non-empty text within the length limit as an opaque name (FR-003, edge
  cases), so only the empty name and over-long names are invalid.
- `/speckit-clarify` on 2026-10-24 (spec, Clarifications): `approve` needs the approval digest
  whenever storage is declared (FR-008); a widget that declares storage must export `update`
  (FR-012); the tool lists every store and clears one by id and origin, file or no file (FR-014).
- Constitution re-check on 2026-10-24, after `/speckit-plan`: an independent review against
  v2.3.1 found 3 conflicts, 13 risks and 11 consistency gaps. Each SQLite claim was reproduced in
  a separate spike (rusqlite 0.41.0, bundled SQLite 3.54.0, Linux), and each code or crate claim
  read in the source, before acting.
  - Conflicts: a write that reaches the file cap makes SQLite roll the update's transaction back
    and run later writes in autocommit, each kept (reproduced: `is_autocommit()` true after
    "database or disk is full", the next insert kept, `ROLLBACK` refused), so the run now aborts
    (R7); `libsqlite3-sys` is a caret requirement of rusqlite, so a `Cargo.lock` update alone could
    change the bundled SQLite, now pinned in `runtime/Cargo.toml` and listed in the PR template
    (R1, R15); the spec's unavailable data directory had no load step (R11, step 8a).
  - Risks: the file cap of 2 × quota stopped honest widgets at half their quota (reproduced: values
    of about 1,000 B make a file 4.4 times their usage); `VACUUM` cannot run on a hardened
    connection (reproduced); a read-only open fails with code 776 after a process killed during an
    update (reproduced once pages spill to disk; a read-write open recovers, R8); the commit-time
    grant check had no test that could fail without it, and mid-run changes needed a race-free way
    (R17); the hardening test named too few layers and invited test-only switches (R4); URI names
    (`SQLITE_USE_URI` is compiled in) and a relative `--data` (R4); the system SQLite through the
    environment (R1); `max_page_count` cannot go below the file's size (reproduced, R10); which
    quota a re-approval keeps (R12); `storage show` could be spoofed with `"` (R14); the gap between
    the commit check and the commit (R7); rules with no named test (data model, Other tests named);
    `SQLITE_LIMIT_LENGTH` at exactly name plus value refuses the largest entry (reproduced, R4).
  - Consistency: when the schema is created and checked relative to the authorizer; the stand-in
    and "every call counted" (FR-007); `host-records-untouched` now snapshots after setup;
    `storage list` checks file names against the identity inside; `secure_delete` is `ON`, not
    `FAST`; rusqlite's default `cache` feature is off; `list` takes no page size, and `prefix` and
    `after` have a length limit; quickstart and CLI output agree; the Constitution Check table lists
    every row.
  - Decided with the maintainer: the file cap is five times the approved quota plus 64 KiB, usage
    staying logical (R6, R10); an approval keeps the quota it showed, not the largest ever approved
    (spec clarification 4, R12).
- `/speckit-analyze` on 2026-10-24, after `/speckit-tasks`: no constitution conflict; fixed in
  tasks, research R4 and the data model: `storage-file` builds on `stored-widget` with `updater`,
  since load step 5 (`missing-updater`, `contract-too-old`) runs before the import check that must
  refuse it; `foreign-store` plants its files through a test-only function of the storage module,
  keeping the Clippy rule without exceptions; the hardened setup is two production functions,
  `configure` and `authorize`, so the white-box case reads the pragmas between them; a new case,
  `unwritable-store`, for a store that cannot be written (23 cases); the reverse of
  `same-id-other-origin` (same path, another id); exact sizes for `killed-mid-update`.
  - Settled by the spike of T003: a guest built against `stored-widget` that never calls storage
    does not import it (R13).
