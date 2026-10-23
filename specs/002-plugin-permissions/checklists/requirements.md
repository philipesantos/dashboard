# Specification Quality Checklist: Plugin Permissions

**Purpose**: Validate specification completeness and quality before proceeding to planning

**Created**: 2026-10-22

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

- Clarified with the maintainer on 2026-10-22:
  - Every plugin needs a manifest, and `log` is declared and granted like any other permission
    (FR-001, FR-013), as Principle II requires.
  - Plugins are installed only from repositories, one plugin per repository; the id is the
    repository (`<host>/<owner>/<repository>`), and the installation (Phase 3) keeps the
    repository's permanent identifier against a reused name. Plugins that come from no
    repository use `local/<name>` (FR-002, FR-017, edge cases, assumptions).
  - Required permissions are approved all together and never taken back one by one; they
    change only with a new version, approved again, or by removing the plugin's approval.
    Optional permissions start off and are turned on or off in the host's controls; while off,
    the capability is linked to a stand-in that answers "not granted" (FR-013, FR-014, FR-018,
    FR-019, User Story 4), which constitution v1.2.0 allows in Principle II.
  - Disabling and uninstalling arrive with installation in Phase 3 (assumptions).
  - The Phase 3 warning for dangerous combinations runs on every grant change, judged on the
    permissions the plugin will hold afterwards (assumptions).
  - A grant change takes effect from the plugin's next call, not its next load, so a plugin
    that stays loaded in the app loses access at once; a call already running finishes within
    its time limit (FR-014, SC-005).
- After `/speckit-analyze`, approved by the maintainer on 2026-10-23:
  - Constitution v1.3.0 lets the development command-line tool grant permissions to local
    packages until the host dialogs exist (Principle VII).
  - FR-009 and FR-010 get a hostile case: running any plugin must leave the grant record
    unchanged (FR-016).
  - A package is refused for more than one manifest or a nested one (User Stories 1 and 2).
  - FR-007 refuses an ungranted required permission whether or not the plugin uses it.
  - Repository ids accept `github.com` only, for now (assumptions).
- Approved by the maintainer on 2026-10-23, after a side review: a plugin's development grants
  belong to the file it was approved from, so another file that copies the id inherits nothing
  (FR-020, edge cases). Grants per digest were rejected: approving on every build turns
  approval into a reflex.
- "Command-line tool", "JSON" and "schema" name the interfaces the user works with, as in
  feature 001, not the implementation; the plan decides how the manifest is bound to the plugin
  and where grants are stored.
