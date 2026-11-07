# Specification Quality Checklist: Contract v1 Closed and Docs for Plugin Authors

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-10-25
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

- Rust, WIT, the digest pin and CI name what the feature is about (the contract, the language of
  the only plugins until Phase 5, the build that keeps the docs true), as earlier specs name
  HTTPS or the keychain; how the drift check and the walkthrough check are built is left to the
  plan.
- Decided with the maintainer before specifying (2026-10-25): a measurement showed that
  jco/componentize-js puts the whole JS engine in each plugin (12 MB, over the 10 MiB package
  cap; 3.3 s of native compilation per plugin), so TypeScript moves to Phase 5 with a JS engine
  run by the host (guide rev 76), and this feature closes the last Phase 2 item; the widget
  content format stays in Phase 3 (FR-014).
- The spec refines the description in four places, each to be confirmed:
  1. The walkthrough starts outside the repository and must not need its internal build tooling
     for the author's own plugin (FR-007); the development CLI is still built from a clone until
     Phase 6 (Assumptions).
  2. The reference and the drift check also cover the connector format's fields (FR-012,
     FR-017), since connector authors read the same docs.
  3. A review finding that would make the host refuse something it accepts today is applied only
     if it closes a security hole; otherwise it is a breaking-change finding (FR-005, FR-006).
  4. The docs the drift and walkthrough checks read count as code, so a change to them runs the
     whole CI, the security suite included (FR-019; corrected during planning, since the
     constitution does not count Markdown the build uses as documentation).
- Clarified after a side note (2026-10-25, see the spec's Clarifications): a flaw only a breaking
  change can fix is not left for a later v2. Before the first public release it goes to the
  maintainer, and if chosen ships here as 2.0.0 with v1 retired, so the host keeps one current
  contract (FR-006). The description said "recorded for v2".
- SC-001's 30 minutes is a target to confirm when the walkthrough is walked on the three systems.
