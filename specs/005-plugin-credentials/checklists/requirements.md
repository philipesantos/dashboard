# Specification Quality Checklist: Plugin Credentials

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

- `keyring` is named only in Assumptions, as the constitution's fixed technical decision. The
  `Authorization` header (FR-002), HTTPS (FR-007) and the authorization code with proof key
  (FR-009) are stated as security rules, as 003 states HTTPS; how they are built is left to the
  plan.
- Decided with the maintainer before specifying (2026-10-24): constitution v2.4.0 lets the
  development tool connect accounts (Principle VII); the manifest's login providers become
  obsolete (FR-016); the account belongs to the connector, shared by the widgets approved to use
  it (FR-004); CI uses a test sign-in service, development uses an app the developer configures,
  and the project's app is registered in Phase 6 (FR-015); "subscription required" is in this
  feature (FR-012).
- The spec refines the description in three places, each to be confirmed:
  1. A widget declares whether it uses a connector's account (FR-003), so a widget that only
     shows public data never acts as the user even when an account is connected (least
     privilege, Principle II). The description had every widget granted the connector use it.
  2. No GitHub account in this feature (FR-015, planning session): GitHub's browser-redirect flow
     needs a client secret and the device flow was left out, so the catalog's GitHub connector
     stays public-only and the example uses a development connector (FR-020).
  3. A redirect carries the credential only to a host it is for, and drops it for another host of
     the connector (FR-007). The description said "never past a redirect (already refused)", but
     feature 003 follows redirects to the connector's own hosts.
- The numbers (sign-in time limit, key length, refresh limits) are left to the plan and measured,
  as in features 001, 003 and 004. SC-006's 5 ms per call is a target to confirm in
  `performance.md`.
- Planning session (2026-10-25): the maintainer first chose both sign-in flows, then removed the
  device flow; with it went the GitHub account (GitHub's redirect flow needs a client secret), so
  `catalog/github` stays public-only and the example uses a development connector. Constitution
  v2.4.1 names keyring-rs's crates. The CLI keeps 003's rule of no test seam: sign-in, the example
  and the hostile cases are library tests.
- Independent review of the plan against v2.4.1 (2026-10-25), each finding checked at the source
  before the artifacts changed:
  - Conflicts (4), all fixed: the `data flow:` line for data read as an account (Principle III,
    FR-005); no plain-HTTP destination in a connector with a credential (FR-007); token,
    revocation and identity on the credential's own destinations (FR-001); the account fixed at
    link time, so one connected mid-run applies from the next load (Principle II, FR-011).
  - Risks (10), all addressed in research: no test path through the CLI (R15, the maintainer's
    choice); a key cannot be typed in CI (pseudo-terminal, library checks); the Windows store's
    ordering across threads and abandoned writes with rotating refresh tokens (one vault thread,
    lock held through the write); secrets in `keyring-core` and `serde_json` errors (secret-free
    mapping, a test); escaped echoes (bearer-token set, escaped forms searched); a type-only
    `connectors` import (rule and test); one widget locking the shared account out (401 confirmed
    with identity); macOS prompts across binaries (same binary creates and deletes); CI and bench
    wiring on Linux.
  - Gaps (7), all closed: kept data deleted at disconnect, not only at the next load; `identity`
    optional with `id` required; named tests for the rules that had none; host requests and the
    rate limits; the disconnect race (lock across both steps); header names compared without case;
    counts (32 hostile cases, 14 named tests).
