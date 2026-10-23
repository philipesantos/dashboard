# Specification Quality Checklist: Plugin Connectors

**Purpose**: Validate specification completeness and quality before proceeding to planning

**Created**: 2026-10-23

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

- Decided with the maintainer on 2026-10-23, before the spec (constitution v2.0.0):
  - No generic network: declarative connectors run by the host, with typed operations and
    reads declared apart from changes; a widget depends on a specific connector, never a type.
  - Destinations come from the official catalog (reserved domains; protocol connectors take
    them from the user) or from a domain whose owner authorizes the connector's repository, so
    there are no impostor or duplicate connectors, and authors can run their own, paid services.
  - Network calls happen in an update entry point with its own time limit; render keeps 1 s and
    reads the data the host kept.
  - Services on the user's machine or local network: decided now (constitution v2.1.0 and
    v2.2.0: catalog connectors only, at an address the user chooses, kept local; a credential
    only over HTTPS with a certificate valid or accepted by the user at first use; plain HTTP,
    shown as unencrypted, only for connectors without credentials), built after feature 005,
    since most of them need a credential. This feature refuses every non-public address and
    leaves room for that destination kind (assumptions).
- An operation's kind (read or change) is the word of whoever wrote the connector, checked only
  by the catalog's review. Decided with the maintainer on 2026-10-23: a read means it changes
  nothing in the service, never that no data leaves; outside the catalog a read must be a GET or
  HEAD without a body; and the approval shows the parameters each operation sends and whether
  its kind was reviewed or declared (FR-002, FR-014, edge cases).
- Scope cut with the maintainer on 2026-10-23, to keep the Phase 2 gate within reach: this
  feature keeps the core (connector format, catalog with GitHub public data, proxy, update and
  kept data, contract 1.1.0, local connectors in development, hostile cases, documents). Moved
  to a feature of its own alongside installation in Phase 3: connectors from repositories with
  `/.well-known/` verification (until then, a connector whose id names a repository is refused
  as not verified, which covers the gate's "unverified destination"), registrable-domain
  reservation (this feature reserves the catalog's exact hosts), RSS and other protocol
  connectors, catalog criteria, and updates on load and on a schedule (assumptions, FR-004,
  FR-005, FR-012). From 7 user stories and 24 requirements to 5 and 22.
- `/speckit-clarify` on 2026-10-23 (spec, Clarifications): an update calls only reads (FR-013);
  a widget receives only the response headers its operation declares, never cookie headers
  (FR-002); the request log keeps parameter values as sent, cut to a maximum length, never
  credentials (FR-009); kept data is discarded when the approval is removed, a connector is
  turned off or a new version of the widget is loaded (FR-010); the host ignores system network
  proxies and connects directly (FR-007).
- Still a default, not asked: the first catalog connector is GitHub public data (FR-021).
- "Command-line tool", "JSON", "HTTPS", "GET" and "schema" name the interfaces users and
  authors work with, as in features 001 and 002, not the implementation; the plan decides the
  formats, the contract and every number.
