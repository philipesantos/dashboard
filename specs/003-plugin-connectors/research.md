# Research: Plugin Connectors

Decisions behind [plan.md](plan.md), each with what it rests on. Spike results come from
throwaway projects outside the repository, built on Linux on 2026-10-23; the plan's first tasks
repeat the parts that matter on all three systems. An independent review of the plan against
constitution v2.3.0 changed R1, R2, R5, R8, R10 and R12 to R18 and added R20; the review's
findings are listed in [checklists/requirements.md](checklists/requirements.md).

## R1. The connector format

**Decision**: a connector is a JSON document checked by a published schema,
`wit/connector.schema.json` ([draft](contracts/connector.schema.json)), and parsed by the host
into typed structs that refuse unknown fields, as the manifest is (feature 002, R2). One file per
connector, at most 64 KiB. Its fields:

- `connector_version` (`1`), `id`, `version` (SemVer), `author`, `description`.
- `reserves` (catalog connectors only, required there): the domains the connector reserves,
  reviewed with it, which must cover every host it declares (FR-005, R16).
- `destinations`: a map from a short name to an origin, `https://<host>[:<port>]`, with no
  path, query, fragment or credentials. A local connector may also use
  `http://localhost:<port>` or `http://127.0.0.1:<port>` / `http://[::1]:<port>` (R6).
- `operations`: a map from an operation name to its declaration: `kind` (`read` or `change`),
  `method`, `destination` (a key of `destinations`), `path`, `query`, `headers`, `body`,
  `parameters` and `response` (`format`: `json` or `text`; `headers`: the names passed to the
  widget).
- Fields named `credential`, `credentials` or `auth`, at any level, are refused as
  `credentials-not-supported`, not as an unknown field, so authors read that credentials arrive
  with feature 005 (FR-017). The format leaves room for them: feature 005 adds a field, and a
  connector without it keeps meaning what it means now.

Fixed request headers are limited to `Accept` and `Accept-Language`, plus names that start with
`X-` in catalog connectors only: some servers read `X-HTTP-Method-Override`, `X-Original-URL`
and the like to change a request's method or path, which would let a local connector's read act
as a change; the catalog's review checks each `X-` header. The host refuses `Authorization`,
`Cookie`, `Proxy-*`, `Host`, `Content-*`, `Transfer-Encoding`, `Connection`, `Upgrade`, `TE`
and `User-Agent`, which it sets or forbids itself. Every fixed value (query values, headers,
body fields) is public: `permissions` shows it and the log records it, so a key written there is
visibly no secret (Principle IV). Response headers passed to the widget are any names except
`Set-Cookie` and `Cookie` (clarification 2).

**Rationale**: data the host reads can do only what the host does with it; a schema lets
authors check a connector before shipping it, and the same typed-parsing approach already
guards the manifest. Named destinations let several operations share one origin and let the
host show the full list before approval.

**Alternatives considered**: OpenAPI (large, describes far more than the host runs, and lets
an author describe any request; a subset would be a format of its own anyway); a connector as a
WIT interface per service (code generation per connector, and still a declaration to check).

## R2. Where connectors come from in this feature

**Decision**:

- **Catalog**: `connectors/catalog/<name>.json` in the repository, id `catalog/<name>`,
  embedded in the host binary at build time (`include_str!` through one list in
  `runtime/src/catalog.rs`). A connector is a catalog connector only if it is in that list
  (FR-005). The catalog starts with `catalog/github` (R16).
- **Local** (development only): `local/<name>`, read from `<connectors dir>/<name>.json`, where
  the directory is `--connectors <dir>` or the default development one (R14). The file's own
  `id` must match its name. Its origin is the file's canonical path, and an approval belongs to
  that origin (FR-018), as a package's does in feature 002. `Runtime` reads local connectors
  only when built with an explicit development option, which the CLI sets and the Phase 3 app
  will not; without it a `local/` id is `destination-not-verified` (FR-018).
- **Repository ids** (`github.com/<owner>/<repository>`) are well-formed in manifests and
  connector files, so the refusal can say why: `destination-not-verified`, naming that
  connectors from repositories arrive with installation in Phase 3 (FR-004).
- A local file that claims a `catalog/` id is `destination-reserved` (FR-005).

**Rationale**: building the catalog into the host makes "the catalog the host was built with"
literal: no file next to the binary can add to it, and every change to it goes through a
reviewed pull request (Principle IX). Local files keep the development loop of feature 002.

**Alternatives considered**: a catalog directory read at run time (anyone who can write next to
the binary extends the catalog); fetching the catalog from the network (a second
distribution channel to secure, before signatures exist in Phase 6).

## R3. Parameters and how values reach the request

**Decision**:

- Types: `text` (`max_length` 1–2048 characters, required; optional `pattern` or `values`, a
  list of allowed strings), `integer` (64-bit, `minimum` and `maximum` required) and `flag`
  (true/false). Each parameter is `required` (the default) or not.
- Placeholders are `{name}` and must name a declared parameter. In `path`, a placeholder is a
  whole segment (`/repos/{owner}/{repo}`); in `query`, a whole value; in `body` (allowed only
  for changes, and for catalog reads, R4), a whole JSON string value, replaced by the typed
  value. Literal path segments may not contain `{`, `}`, `?` or `#`.
- Encoding, always by the host: path values are percent-encoded except RFC 3986 unreserved
  characters, and the values `""`, `.` and `..` are refused, because servers resolve dot
  segments even when encoded; query values are form-encoded; an absent optional parameter drops
  its query pair or body field and may not fill a path segment.
- Checks per call, before anything is sent: every argument names a declared parameter, none
  twice, every required one is present, each value has its type and bounds. A `pattern` must
  match the whole value; patterns use the `regex` crate, which runs in linear time, with a
  compiled-size limit of 64 KiB, checked when the connector loads.
- Any failure is `value-outside-type`, naming the parameter and the rule.

**Rationale**: whole-value placeholders and host-side encoding make it impossible for a value
to add a path segment, a query pair, a header or a second request, which is what FR-003
requires. Dot segments were found while designing the GitHub connector: `{repo}` = `..` would
turn `/repos/a/../releases` into another endpoint. `regex` rules out patterns that take
exponential time on a hostile value.

**Alternatives considered**: free-form URL templates (a value could carry `/` or `?`);
JSON Schema for parameters (far more than three types need).

## R4. Reads, changes and what the host can check

**Decision**: an operation's `kind` says whether it changes data in the service (FR-002,
clarification 1).

- Outside the catalog, a read must be `GET` or `HEAD` and declare no `body`; any other read is
  `invalid-connector`, naming the operation and saying it must be declared as a change.
- A catalog connector may declare a `POST` read with a body, such as a GraphQL query, because
  its review checks the kind (Principle IX).
- In this feature, `update` may call only reads; a call to a change is refused as
  `change-needs-user-action` (FR-013). Changes are parsed, shown in `permissions` and approved,
  so the format is complete before Phase 3 calls them.
- `permissions` marks every operation's kind as `reviewed` (catalog) or `declared by the
  developer` (local); Phase 3 adds `declared by the owner` for verified domains.

**Rationale**: the host cannot see what a server does with a request; it can check the shape of
the request. A body-less `GET` is the shape every HTTP stack treats as safe, and it bounds what
a read can carry to typed values in the URL.

## R5. The HTTP stack

**Decision**: `ureq` 3.5.0, pinned exactly (`=3.5.0`), without default features, with
`rustls-no-provider` only; `rustls` 0.23 without default features, with the `ring` provider; the
roots from `rustls-native-certs` (R8). The `network` module is the only code that touches
`ureq`, enforced by Clippy (R20), and configures each agent:

| Setting | Value | Why |
| --- | --- | --- |
| `https_only` | `true` (an agent of its own for local loopback connectors) | FR-007 |
| `proxy` | `None` | Clarification 5; the default reads `HTTPS_PROXY` and friends |
| `max_redirects` | `0` | Redirects are checked by the host (R7) |
| cookies | Not compiled in (`cookies` feature off) | FR-007 |
| compression | Not compiled in (`gzip`, `brotli` off) | No `Accept-Encoding`, so the size limit applies to what arrives |
| `http_status_as_error` | `false` | An error status is a normal answer (edge cases) |
| `user_agent` | `dashboard/<host version>` | Set by the host, never by a connector |
| `max_response_header_size` | 16 KiB | R10 |
| timeouts | Per call, the smaller of the call limit and the update's remaining time | R10 |
| resolver | `PublicOnly` (R6), through `ureq::unversioned::resolver::Resolver` | Connects only to addresses the host checked |
| connection pool | One agent per update, dropped at its end | No state shared between widgets or updates |
| certificates | `RootCerts::Specific` with the system's roots, `rustls`' own verifier | R8 |

**Spike**: with the platform verifier the tree was 29 crates, none of them `tokio`; with the
system roots instead (R8) it is 27, `cargo deny check licenses` passes once ISC is allowed, and
the roots load (121 on the Linux machine). The custom resolver
compiled against 3.5.0, refused `https://localhost:9/` as `host not found` before any
connection, `https_only` refused `http://example.com/`, and with `HTTPS_PROXY` set the request
still went to the resolver for the target, not to the proxy.

**Rationale**: the host calls are synchronous (Wasmtime without async, feature 001), and `ureq`
is synchronous, so a connector call is a plain blocking call with its own timeout, and it works
the same from the CLI and from a blocking thread of the Phase 3 app. Every default that matters
here (proxy, redirects, compression, cookies, user agent) is set explicitly and tested. The
resolver API sits in `ureq`'s `unversioned` module, which may change in a minor release: hence
the exact pin, the single module that uses it, and the hostile cases that break if an upgrade
changes its behavior.

**Alternatives considered**:

- `hyper` 1.x over a connection the host opens itself: the most proven HTTP/1.1 code, but async,
  which brings a `tokio` runtime into every call and fails when called from inside another
  runtime (the Tauri app's).
- `reqwest` blocking: runs its own `tokio` thread, and its proxy, redirect and cookie behavior
  would all need overriding.
- An HTTP/1.1 client written over `httparse`: the smallest dependency, but chunked encoding,
  header limits and connection handling become security-core code to review.
- `native-tls`: three TLS stacks with three behaviors to test, each with the operating system's
  own certificate fetching (R8); `rustls` with its own verifier behaves the same on every
  system.

## R6. Which addresses a connector may reach

**Decision**: the `PublicOnly` resolver asks the system to resolve the destination host, keeps
only addresses the host classifies as public, and fails with `non-public-address` when none is
left; the connection is made only to the addresses it returned, so a name cannot change between
the check and the connection.

The classifier is a table in `runtime/src/network/address.rs` built from the IANA IPv4 and IPv6
Special-Purpose Address Registries. Not public: unspecified, loopback, private (10/8,
172.16/12, 192.168/16), shared (100.64/10), link-local (169.254/16, fe80::/10), unique local
(fc00::/7), the IETF protocol block (192.0.0/24), documentation ranges, benchmarking
(198.18/15), multicast, reserved (240/4), broadcast, `0/8`, and the IPv6 forms that embed an
IPv4 address (IPv4-mapped, IPv4-compatible, NAT64 `64:ff9b::/96`, 6to4 `2002::/16`, Teredo
`2001::/32`), which are judged by the address they embed. Every row has a unit test.

Destinations are domain names: an IP address is refused when the connector loads
(`invalid-connector`). The one exception is a local connector in development (FR-018), whose
destination may be `localhost`, `127.0.0.1` or `[::1]`, without HTTPS, on any port; for it the
resolver accepts loopback addresses only, never another private address.

**Rationale**: checking the addresses the connection uses, at every connection, defeats DNS
answers that point at the local network and DNS rebinding. `std`'s `is_global` is unstable, and
an explicit table is what a reviewer can check line by line.

## R7. Redirects

**Decision**: `ureq` follows none; the host does, at most 5 times per call. The `Location` is
resolved against the current URL with the `url` crate. A redirect is followed only when its
target's scheme, host and port are exactly one of the origins the same connector declares;
otherwise the call fails as `redirect-not-allowed`, naming the target, and nothing is sent to
it. A followed redirect goes through the same resolver and checks as the first request. A read
keeps its method on 307 and 308 and becomes `GET` on 301, 302 and 303 (changes are not called in
this feature).

**Rationale**: a redirect is a destination chosen by the server, so it gets every check a
destination gets, and the approval already showed every origin of the connector (FR-014).

## R8. Certificates

**Decision**: `rustls`' own verifier (`rustls-webpki`), with the roots the operating system
trusts, read by `rustls-native-certs` (the system store on Windows, the keychain on macOS, the
certificate bundle on Linux), passed to `ureq` as `RootCerts::Specific`. Valid means: for the
destination's name, within its validity period, chaining to one of those roots (FR-007). No
revocation is checked online; a stapled OCSP response is not required. The roots load once per
process.

**Rationale**: the review found that the system verifier (`rustls-platform-verifier` 0.7.1) lets
the operating system fetch revocation and certificate data on its own: on Windows its chain
engine retrieves URLs with a 10 s timeout (only its test engine sets "cache only"), and on
macOS its own source says it does not stop online revocation checks or root downloads. Those
requests go to addresses written in the server's certificate, through the operating system's
HTTP stack and proxy, with no address check and past the call's time limit: traffic outside the
host proxy, against Principle III. It also pulls `webpki-root-certs`, whose license
(CDLA-Permissive-2.0) fails `cargo deny`. With `rustls-webpki`, every byte of a connection goes
to the address the host checked, the same code runs on the three systems, and the tests run that
code with only the roots swapped (R15). Giving up online revocation is the trade: most clients
skip it, and the roots stay those the system trusts.

**Alternatives considered**: the system verifier (above); Mozilla's roots bundled
(`webpki-roots`): independent of the system, but CDLA-licensed and aging with the binary;
an amendment allowing the operating system's certificate fetches outside the proxy (they cannot
be tested in CI and break the address rule).

## R9. System network proxies

**Decision**: never used (clarification 5): `proxy(None)`, and the `win-system-proxy` and
`socks-proxy` features stay off. When a call fails as `unavailable`, its detail adds that the
host connects directly and does not use network proxies. A hostile case sets `HTTPS_PROXY`,
`HTTP_PROXY` and `ALL_PROXY` to a recording server and checks that it receives nothing.

**Rationale**: behind a proxy the host cannot see which address a name leads to, so FR-007's
address rule could not be enforced.

## R10. Limits

**Decision**: provisional values, in `Limits`, measured on the three systems before the feature
closes (as feature 001 did):

| Limit | Value |
| --- | --- |
| Update wall-clock time | 10 s (render keeps 1 s) |
| Update fuel and memory | The render's: 500 M fuel units, 32 MiB |
| One call's time (resolve, connect, TLS, response) | 5 s, cut to what is left of the update |
| Calls per update, every call counted (refused, not granted and failed included) | 20; past it, one summary entry in the log |
| Arguments per call | 16, names at most 32 characters, checked before logging |
| Response body per call | 1 MiB |
| Response bodies per update | 4 MiB |
| Response headers per call | 16 KiB |
| Request address (path and query) | 8 KiB |
| Request body | 64 KiB |
| Redirects per call | 5 |
| Update output (the data document) | 256 KiB |
| Rate per widget | 30 calls per minute |
| Rate per destination host, all widgets | 60 calls per minute |
| Request log | 2 MiB, plus one previous file of the same size |
| Parameter value in the log | 256 characters, then ` [truncated]` |

Rates are token buckets held by the `Runtime`, so they apply across widgets in one run of the
host. In the CLI each command is its own process; only the maintainer can start several, never a
plugin, and calls per update bound each one; the Phase 3 app, which stays running, gets the full
rule. Revocation does not depend on the process: before each send the call reads the grant
record again (R12).

**Rationale**: an update spends most of its time waiting, so its wall clock is far larger than
render's while its CPU budget stays the same; Wasmtime counts no fuel while the plugin waits in a
host call. 1 MiB covers a page of any JSON API a widget shows; four of them fit easily inside the
32 MiB memory cap, which also counts every response copied into the plugin.

## R11. Contract 1.1.0 and older v1 minors

**Decision**:

- `wit/v1/widget.wit` becomes `dashboard:widget@1.1.0` ([draft](contracts/widget-v1.1.wit)):
  `log` and `renderer` unchanged; a new `connectors` interface (the capability, whose error's
  first case is `not-granted`); a new `updater` interface (`update: func(config: string) ->
  result<string, string>`); and two worlds: `widget` (imports `log`, exports `renderer`, as in
  1.0.0) and `connected-widget` (imports `log` and `connectors`, exports `renderer` and
  `updater`).
- `wit/history/1.0.0/widget.wit` keeps the published 1.0.0 file, frozen. A test parses both and
  fails unless 1.1.0 contains every interface, function and type of 1.0.0 unchanged, so a minor
  version only adds.
- The host accepts a v1 plugin whose contract names all carry one version, with major 1 and not
  newer than its own (1.1.0); a newer minor is `contract-mismatch`, naming both versions
  (FR-016). Imports are linked against the 1.1.0 definitions: Wasmtime's linker matches an import
  of a compatible minor version, both ways (feature 002, R5). Exports are looked up by the exact
  names the component exports (`dashboard:widget/renderer@1.0.0`), and called as typed functions,
  whose types 1.x never changes.
- The permission registry records the minor version that introduced each interface: `log` 1.0,
  connectors 1.1. A widget whose manifest declares a connector but whose contract is older than
  1.1 is `contract-too-old`, naming 1.1.0.
- Widgets that declare connectors must export `updater`; one that does not is `missing-updater`.
- The plugins that exist today on v1 (`hello-widget`, `probe`, `quiet`) keep building from the
  frozen 1.0.0 copy, unchanged, so the whole existing suite runs as 1.0.0 plugins on the 1.1.0
  host (SC-006). New plugins build from `wit/v1`.

**Spike needed at the start of implementation**: a 1.0.0 component instantiated with the 1.1.0
linker, its renderer called through the exact-name lookup. Feature 002 already linked a 1.1.0
import against a 1.0.0 definition; the export lookup is plain Wasmtime API.

**Rationale**: two worlds keep render-only widgets as they are while connected widgets must
implement `update`. Looking up exports by the name the component has, instead of through
generated 1.1.0 bindings, is what makes older minors run without a binding per version.

**Alternatives considered**: one world with an optional export (the Component Model has none,
so every widget would implement an empty `update`); a host binding per minor version (grows with
every feature); a new major version (every plugin rebuilt).

## R12. Linking the connectors capability

**Decision**: one `connectors` interface serves every connector, so the plugin's connectors
follow Principle II's rule for capabilities that share an import (constitution v2.3.0):

- At link time (load step 9), the host builds the plugin's connector table from the link plan:
  one executor per declared connector that is granted, holding the connector's checked
  declaration and the operations the manifest declares for it. An optional connector that is off
  gets no executor: it is not in the table.
- The interface is linked to the real implementation over that table when the table has an
  entry, and to the stand-in, which answers `not-granted` to every call, when the plugin declares
  connectors and none is granted.
- A call first looks its connector up in the table; a miss, whether the connector is off or not
  declared at all, answers `not-granted`, sends nothing and is logged as not granted, whatever
  else is wrong with the call. That path is the connector's stand-in.
- Before it sends, and again before it hands a response to the widget, a call re-reads the
  grant record (under its lock, as feature 002 reads it), so a change made by any process
  applies; when its connector is no longer granted, it answers `not-granted` and drops a
  response that arrived meanwhile (FR-015). This check can only take away: nothing at call time
  adds an executor to the table, and a connector turned on during an update applies from the
  plugin's next call, when it is linked again, as in feature 002.
- Only then does `HostState` check whether the store runs `render` or `update`; a call during
  `render` to a connector in the table is `called-outside-update`, and nothing is resolved or
  sent (FR-010).
- A white-box test checks that the table's keys are exactly the granted connectors and that the
  stand-in is linked when none is granted, since no plugin can see the difference (Principle
  VIII).

**Rationale**: what a plugin can reach is decided once, from the plan, when it is linked, as
feature 002's linker decides it; a connector that was not granted is never built, rather than
built and guarded on every call. Connectors are data, so the Component Model cannot give each one
an import of its own.

**Alternatives considered**: the real implementation checking the live grants on every call (the
design feature 002 rejected, and Principle II now rules out); one interface per connector
(connectors are data, WIT is static); no optional connectors (loses the pattern feature 005
needs, a widget that shows more with an account connected).

## R13. The grant record, version 2

**Decision**: `record_version` 2. An entry gains `connectors`: for each connector the manifest
declares, `origin` (`catalog` or the local file's canonical path), `version`, `granted` (required
connectors always true; optional ones true only when on), `destinations` (every origin of the
connector, sorted) and `operations` (for each operation the widget declares, the SHA-256 of its
canonical declaration: kind, method, destination origin, path, query, headers, body, parameters
and response). Version 1 records are read as version 2 with no connectors and written back as
version 2.

At load (step 7), each declared connector is compared with the entry: a required connector, or
an optional one that is on, whose destinations or operation digests differ, or whose origin
differs, is `needs-new-approval`, naming what changed (a destination added, an operation
changed, an operation newly declared); `approve` records the new state. A connector that only
added operations the widget does not declare digests the same and needs nothing (FR-014).

Load step 8 prunes connectors too: an entry for a connector the manifest no longer declares
is removed, so declaring it again in a later version needs a new approval, as feature 002 does
for permissions (its FR-012).

**Rationale**: digests per operation make "naming what changed" exact without storing the
declarations twice, and a connector-wide list of destinations is what bounds redirects, so a new
one needs approval even when no declared operation uses it.

## R14. Kept data and the request log

**Decision**: both are host data in the development data directory,
`<data dir>/dev.philsantos.dashboard/development/`, or `--data <dir>`; plugins have no file
access.

- Kept data: `widget-data/<sha256 of id and origin>.json`, written atomically:
  `{ id, origin, digest, approval: { digest, connectors_on }, fetched_at_ms, data, last_update:
  { at_ms, outcome, error } }`. Render gets `data`, `fetched_at_ms` and `last_update`, or no data
  (FR-011). The file is discarded when `remove` runs, when `optional … off` turns a connector off,
  and whenever a load finds that the package digest, the approval or the set of connectors that
  are on no longer matches what the file recorded (clarification 4), so a change made by another
  process is also caught.
- Request log: `requests.jsonl`, one JSON object per call, appended under a file lock; when it
  passes 2 MiB it is renamed to `requests.1.jsonl`, replacing the older one (FR-009). Each entry:
  time, plugin id, connector, operation, kind, method, destination host, path, arguments (each
  value cut to 256 characters), outcome (status, or the rule and detail), bytes sent and
  received, duration; a followed redirect gets an entry of its own. The connector and operation
  names a plugin passes are cut to 64 characters before they are written, and past an update's
  call limit one summary entry counts the calls refused, so one update cannot push earlier
  entries out. `runtime requests` escapes every text a plugin or a connector wrote, as feature
  001 escapes log messages. The host never puts credentials in it; in this feature there are
  none to put.
- Times are integer milliseconds since the Unix epoch, so no date library is needed.

**Rationale**: the same file discipline as the grant record (lock, atomic rename) and the same
place, so the CLI and the tests can point everything at one temporary directory.

## R15. Testing without the internet, without weakening the rules

**Decision**: a `test-network` cargo feature, never in `default`, turned on only by the runtime's
own dev-dependency on itself, compiles `Runtime::with_test_network`. It takes:

- a resolver map from names to IP addresses, standing in for DNS; the port always comes from
  the destination, so the exact-port rule is tested;
- the socket addresses that count as public for the test, so a server on `127.0.0.1:<port>`
  passes as a public destination while `127.0.0.1` on any other port, `10.0.0.1`, `::1` and the
  rest stay refused;
- the test CA's certificate as the only root, given to the same `rustls-webpki` verifier the
  host always uses (R8);
- extra catalog connectors, so the hostile cases can exercise catalog rules (HTTPS, `POST`
  reads, reserved hosts) with connectors written in the test.

`tests/common/servers.rs` runs HTTPS servers (`rustls` server side, certificates from `rcgen`,
names under `.test`) and plain HTTP loopback servers, each recording every request it receives,
and scripted per path: status, headers, body size, delay, redirect. Cases that need no seam
(local connectors on loopback, parameters, limits, render-time calls, changes, optional off, the
log, kept data, the proxy variables through the CLI) use the production network.

The library refuses to compile with `test-network` outside debug builds (`compile_error!` on
`all(feature = "test-network", not(debug_assertions))`), and CI's tests run in debug.

**Spike**: a crate with a dev-dependency on itself that turns a feature on: its integration tests
saw the feature, its release binary did not. The binary built for the tests did see it, so the
CLI must offer no option that reaches the seam.

**Rationale**: every rule is tested on the real path: same resolver code, same classifier, same
TLS verifier, only the inputs (names, roots) differ. The seam cannot reach a release build: the release CLI and
the Phase 3 app never enable the feature, and the CLI has no option that reaches it.

## R16. The first catalog connector and the example

**Decision**: `catalog/github` ([draft](contracts/catalog-github.json)), destination
`https://api.github.com`, three reads without an account: `get-repository`, `list-releases`
and `list-issues`, with the `Accept` and `X-GitHub-Api-Version` headers fixed and `Link`,
`ETag`, `X-RateLimit-Remaining` and `X-RateLimit-Reset` passed to the widget. It reserves
`github.com` (`"reserves": ["github.com"]`), so `api.github.com`, `uploads.github.com` and every
other host under it belong to the catalog (FR-005). An explicit, reviewed list needs no public
suffix list: the reviewer checks that it names whole domains of the service.

The example widget `plugins/examples/github-releases` (contract 1.1.0, `connected-widget`)
updates the latest releases of the repository in its configuration and renders them. A fixture,
`plugins/fixtures/net-probe`, makes in `update` (or, when told, in `render`) the calls its
configuration lists and returns every answer, so each hostile case packs it with its own
manifest, as feature 002 did with `probe`.

**Rationale**: GitHub is the first provider of feature 005 and an official widget of Phase 3, so
this connector is the one feature 005 extends with an account.

## R17. CLI

**Decision** ([contracts/cli.md](contracts/cli.md)): `update <plugin>` runs the widget's update and
keeps its data; `run` renders connected widgets with their kept data; `permissions` shows
connectors, destinations, operations, the parameters and fixed values sent, where each kind
comes from, whether data read from this machine can reach another destination, and an approval
digest over all of it; `approve` of a widget that declares a connector requires
`--expect <digest>` and refuses, granting nothing, when the digest of what it would approve
differs (FR-014); `optional <plugin> <connector-id> on|off`; `requests` prints the request log;
`check-connector <file>` validates a connector for authors. `--connectors <dir>` and
`--data <dir>` join `--grants <path>` on every command that needs them.

**Rationale**: one verb per decision, as in feature 002; updates run only when the maintainer
asks (FR-012).

## R18. Dependencies

| Crate | Where | Why | In `Cargo.lock` | License |
| --- | --- | --- | --- | --- |
| `ureq` `=3.5.0` | runtime | HTTP/1.1 client (R5) | No | MIT OR Apache-2.0 |
| `rustls` 0.23 (`ring`) | runtime | TLS | No | Apache-2.0 OR ISC OR MIT |
| `rustls-native-certs` 0.8 | runtime | The system's roots (R8) | No | Apache-2.0 OR ISC OR MIT |
| `ring` 0.17 | runtime (through `rustls`) | Cryptography | No | Apache-2.0 AND ISC |
| `rustls-webpki`, `untrusted` | runtime (through `rustls`) | Certificate parsing | No | ISC |
| `url` 2 | runtime | Origins and redirect targets (R6, R7) | Yes | MIT OR Apache-2.0 |
| `regex` 1 | runtime | Parameter patterns (R3) | Yes | MIT OR Apache-2.0 |
| `rcgen` 0.14 (`ring`) | runtime, dev | Test certificates (R15) | No | MIT OR Apache-2.0 |

`deny.toml` gains `ISC` in its allowed licenses: it is a permissive license compatible with
distributing the client under Apache-2.0, and `ring`, `untrusted` and `rustls-webpki` use it.
`cargo deny check licenses` passed on the spike with this tree and that one change (it failed
with `rustls-platform-verifier`, through `webpki-root-certs`). `rustls` and `rcgen` are taken
without default features or with `ring`, so `aws-lc-rs` never enters the tree. Each crate must
pass `cargo deny` and `cargo audit` in CI.

## R19. Measuring

**Decision**: `runtime bench` adds the connector checks to `checks_us` (SC-007, under 10 ms) and
prints `render_with_data_us`, a render of a connected widget from kept data while its test
service would take 5 s (SC-004, under 1 s). `bench.yml` records them on the three systems into
`specs/003-plugin-connectors/performance.md`, with the limits of R10 as measured.

## R20. Keeping the review on what matters

**Decision**:

- The pull request template lists as security core `runtime/`, `wit/`, `connectors/`, the root
  `Cargo.lock` when a change moves a crate of the network stack (`ureq`, `ureq-proto`, `rustls`,
  `rustls-webpki`, `ring`, `rustls-native-certs`, `url`, `regex`), and as security tests
  `plugins/hostile/` and `plugins/fixtures/`.
- `clippy.toml` disallows `ureq` types outside `runtime/src/network/`, which allows them
  explicitly, so no other module can open a connection without the review noticing.
- Tests never read `specs/`: CI skips documentation-only changes, so a test that read the drafts
  could change without CI running. The schema tests read `wit/`, `connectors/` and `plugins/`.

**Rationale**: the TLS and HTTP code that enforces Principle III lives in dependencies, so a lock
file change is a change to the proxy; and the claim "only the network module opens connections"
holds only if a tool checks it.

