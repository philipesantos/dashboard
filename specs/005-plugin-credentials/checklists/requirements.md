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

- The `keyring` crate and GitHub are named only where the constitution's fixed decision and the
  guide name them; the requirements speak of the keychain, the sign-in standards and the
  connector's declaration. FR-002 forbids service-specific code in the host: every service,
  GitHub included, is described by its connector's declaration (decided with the maintainer on
  2026-10-24).
- Where the spec refines the description:
  - Account use is declared by each widget (FR-004): a widget whose grant does not include the
    connector's account sends no credential even while one is connected. The description had every
    widget granted the connector use the account; least privilege (Principle II) asks for the
    declaration.
  - Redirects (FR-008): feature 003 follows redirects to hosts the same connector declares, so the
    description's "never past a redirect (already refused)" did not hold; a redirect carries the
    credential only to a host the credential is for and drops it for the connector's other hosts.
  - "Scopes" became "the access it asks for", since GitHub's read-only, per-repository client sets
    its access at registration rather than through scopes (FR-003, Assumptions). The side note of
    2026-10-24 found that a classic GitHub client can read private repositories only with write
    access to all of them.
  - Widgets built for older minor versions never use an account (FR-019): they cannot declare
    account use, so a connector's optional credential is simply not sent for them, and one whose
    credential is required needs 1.3.0.
  - Disconnecting discards the kept data of the widgets that used the account (FR-015), as turning
    a connector off does in feature 003, so private data does not outlive the account.
- The numbers (sign-in time limit, key length limit, the bound on service error text) are left to
  the plan; SC-006's 5 ms is a target to confirm in `performance.md`.
- Open for the plan: how GitHub's read-only client renews and revokes tokens from a desktop app
  without a secret (Assumptions), and how the tests reach a keychain on each CI system.
