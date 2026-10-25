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
  2. GitHub's account asks for read access to the repositories the user picks (FR-015), after a
     note that a classic OAuth app reads private repositories only with full access to all of
     them; the plan picks the kind of GitHub app.
  3. A redirect carries the credential only to a host it is for, and drops it for another host of
     the connector (FR-007). The description said "never past a redirect (already refused)", but
     feature 003 follows redirects to the connector's own hosts.
- The numbers (sign-in time limit, key length, refresh limits) are left to the plan and measured,
  as in features 001, 003 and 004. SC-006's 5 ms per call is a target to confirm in
  `performance.md`.
