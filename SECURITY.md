# Security Policy

Trust is the core of this project: it must be safe to install other people's plugins.
Security reports are welcome and handled with priority.

## Reporting a vulnerability

Report vulnerabilities privately through GitHub: open the repository's **Security** tab and
click **Report a vulnerability**. Please do not open a public issue.

Include what you found, how to reproduce it and what an attacker could do with it.

## Scope

Anything that breaks a rule of the [project constitution](.specify/memory/constitution.md) is
in scope, especially:

- plugin code running outside WebAssembly, or using a capability the user did not grant
- plugin network access outside the operations of the connectors it declares: a request the
  host did not check, a destination a connector does not declare, a private or local address
  reached, or traffic that bypasses the host's network proxy
- a connector outside the official catalog that claims a catalog service or its reserved
  domains
- a plugin reading or changing another plugin's storage, reaching anything outside its own
  store through it, writing past its approved quota, or keeping part of an update that failed
- a plugin reading passwords, tokens or AI provider keys, or a credential leaving the host's
  vault in the system's keychain other than in the header its connector declares, to that
  connector's own HTTPS destinations: in a response the widget gets, a file, a log, an error or
  the tool's output
- an account connected, replaced or used without the user: a forged or replayed sign-in, a
  widget acting as an account it was not approved to use, or an update widening what the
  account lets it do without a new approval
- a plugin drawing outside its frame or imitating a host dialog
- AI output triggering an action without confirmation in a host dialog

## Supported versions

The project is pre-alpha and has no released versions yet. Fixes land on the `master` branch.
