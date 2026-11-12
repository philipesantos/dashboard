# Feature Specification: Plugin Installation and the Installation Dialog

**Feature Branch**: `010-plugin-installation`

**Created**: 2026-11-12

**Status**: Draft

**Input**: User description: "Itens 'Diálogo de instalação com permissões em linguagem clara' e
'Instalação de plugin a partir do repositório do autor' da Fase 3 do guia, juntos numa feature
(decisão do mantenedor em 2026-11-12). Instalação a partir de uma release fixada (tag e hash),
nunca de um arquivo; id do plugin = repositório (github.com/autor/repo, um plugin por
repositório); identificador permanente do repositório guardado contra repo-jacking; a instalação
também oferece os conectores que o widget exige, com as mesmas regras; arquivos locais só no modo
de desenvolvimento do CLI. Compilação na instalação num processo separado com limites do SO, e
revisão da decisão 0001 (plugins no processo do host) antes de o app instalar um plugin que não
construiu. O diálogo: permissões em linguagem clara, obrigatórias aprovadas em bloco, opcionais
ligadas uma a uma; para cada conector, os destinos, os parâmetros que cada operação envia e se o
tipo dela (leitura ou alteração) foi revisado no catálogo ou só declarado pelo dono do serviço;
alerta para combinações perigosas (dados de uma conta conectada que podem ir para outro destino),
que roda em toda mudança de concessão, sobre o conjunto de permissões que o plugin terá depois da
mudança. Critérios herdados da feature 007 (docs/design/host-surfaces.md): HS-10 a HS-15 e HS-17
a HS-23, cada um com a sua verificação; as linhas do threat model que esperam a instalação
(compilação na instalação, repo-jacking, conector de repositório sem autorização do dono,
HS-10/HS-11). Os comandos que a janela passa a chamar entram pela lista revisada da feature 008
(window-surface.toml), e as proteções da janela da 008 continuam valendo."

## Clarifications

### Session 2026-11-12

- Q: Which Phase 3 items does this feature cover? → A: The installation dialog and installation
  from the author's repository together, so the dialog grants something real from the start. The
  repository connectors, disabling and uninstalling, the grid and the official widgets stay
  separate items.
- Q: Does this feature bring the host dialog for connecting, reconnecting and disconnecting
  accounts? → A: No. A widget with an account connector installs, with its account use shown and
  judged by the warning, and its calls answer `account-not-connected` until a later feature
  brings that dialog (FR-024).

### Session 2026-11-12 (after the plan's independent review)

- Q: The compile process runs with the user's rights and reads an attacker's bytes. Is it
  sandboxed? → A: Yes, on the three systems: it can read its input, write its output and nothing
  else, with no file, network or keychain access (FR-029). Its output is still trusted as compiled
  code, as when the app compiled in its own process; that residual goes to the threat model.
- Q: Installed plugins still run in the app's process, beside the keychain and OAuth tokens. Is
  that kept? → A: Kept for this feature; moving plugin execution out of the app's process is a
  feature of its own, right after this one and before the alpha.
- Q: Principle VII lets the development tool grant only "until the host app has these dialogs".
  → A: Constitution v2.7.0 (MINOR): the development tool keeps granting to local packages in its
  own record, with no end date, and Principle IX lists the installer, its compile process and the
  host dialogs in the security core.
- Q: How does a person tell an installed widget from one with a look-alike name? → A: The frame of
  every installed widget shows the repository owner (`github.com/owner`), drawn by the host;
  bundled widgets show none (FR-030). Criterion HS-02 changes first, as feature 007's rule
  allows.

## User Scenarios & Testing *(mandatory)*

The people in these stories are:

- the **person** who uses the dashboard;
- a plugin **author**, who publishes a widget from their own repository;
- the **maintainer**, who owns the host and reviews the security core;
- an **attacker**, who controls a plugin, a repository, or a name that once belonged to someone
  else's repository.

Terms used throughout:

- **Release**: a published version of a plugin in its author's repository, named by a tag and
  carrying the plugin package.
- **Pinned release**: a release as the host first saw it: its tag and the digest of its package.
  The same tag with other bytes is not the same release.
- **Permanent repository identifier**: the identifier the code host gives a repository, which
  survives renames and is never reused for another repository, unlike its name.
- **Host dialog**: a dialog only the host draws, with the form of `docs/design/host-surfaces.md`
  (HS-10 to HS-23), in which every trust decision is made (Principle VII).
- **Grant change**: anything that changes what a plugin may do: approving an installation,
  approving an update, or turning an optional permission or connector on.
- **Dangerous combination**: a set of grants under which data read as a connected account, or
  from the user's own machine or local network, can reach another destination.

### User Story 1 - The person installs a widget from its author's repository (Priority: P1)

The person types the address of a widget's repository (`github.com/owner/repo`) into a host
control. The host finds the repository's latest release, downloads its package, checks that the
manifest names that same repository, and compiles it away from the app, under limits. It then
opens the installation dialog: who is asking, by the manifest's name and the repository id, and
what the widget needs, in plain language. The person approves the required permissions together
and turns optional ones on one at a time, or cancels. On approval the widget appears in the
window; on cancel nothing is kept.

**Why this priority**: this is the first time the app runs a plugin it did not build, and the
first trust decision made in the app rather than in the development tool.

**Independent Test**: publish a test widget with one required permission and one optional
connector as a release of a test repository; install it in the app; check the dialog shows the
name, the id, the required permission and the connector off; approve; check the widget renders
and the optional connector answers "not granted". Install it again, cancel, and check nothing new
is kept.

**Acceptance Scenarios**:

1. **Given** a repository whose latest release carries a valid package whose manifest names that
   repository, **When** the person asks to install it, **Then** the host downloads and compiles it
   and opens the installation dialog before anything runs.
2. **Given** the installation dialog, **When** the person approves, **Then** the required
   permissions are granted together, the optional ones the person turned on are granted, the rest
   stay off, and the widget runs.
3. **Given** the installation dialog, **When** the person cancels, presses Escape or closes it,
   **Then** nothing is granted, the package and its compiled form are discarded, and the widget
   does not appear.
4. **Given** a package whose manifest id is not the repository it was downloaded from, **When**
   the person asks to install it, **Then** installation is refused, naming both ids, and no
   dialog asks for approval.
5. **Given** an attempt to install from a file, a URL other than a repository, or a host other
   than `github.com`, **When** the person submits it, **Then** the app refuses it, naming that
   plugins install only from their author's repository.

---

### User Story 2 - The dialog tells the person what they are deciding, in plain language (Priority: P1)

For every permission, the dialog says in a sentence the host wrote what the widget can do with
it, never in the plugin's words. For every connector, it shows its id and version, where its
destinations come from, every destination host, each operation the widget calls with the
parameters it sends, and its kind, read or change, marked as reviewed in the catalog or only
declared. For an account connector, it says which account's data the widget reads. When the
grants the plugin would hold afterwards form a dangerous combination, the dialog warns, naming
the account and the destinations the data can reach.

**Why this priority**: an approval is only as good as what the person understood; the warning is
what Principle III promises the user before approval.

**Independent Test**: install test widgets that declare each permission kind, a catalog connector
with read and change operations, and an account connector beside a second destination; check
each line of the dialog against the manifest and the catalog, and that the warning appears only
for the dangerous combination.

**Acceptance Scenarios**:

1. **Given** a widget that declares `storage` with a quota, **When** the dialog opens, **Then** it
   states that the widget keeps data on this computer, up to the quota in human units.
2. **Given** a widget with a catalog connector, **When** the dialog opens, **Then** it lists every
   destination host and each operation the widget calls, with its parameters and kind, marked as
   reviewed.
3. **Given** a widget that reads as a connected account and can send parameters to another
   destination, **When** the dialog opens, **Then** it shows the warning naming the account's
   service and every other destination.
4. **Given** a widget whose grants form no dangerous combination, **When** the dialog opens,
   **Then** no warning appears.
5. **Given** a plugin name, author or operation name holding markup, bidirectional controls or 500
   characters, **When** the dialog shows it, **Then** it is drawn as escaped text, bounded, and the
   repository id is shown in full (HS-13).

---

### User Story 3 - Updates and later grant changes never widen access silently (Priority: P1)

The person checks an installed widget for updates. The host accepts only a newer release from
the repository with the permanent identifier it kept at installation. An update that needs no new
grant installs after the person confirms it; one that needs more opens the dialog again,
showing everything the new version would hold, with what changed marked. Every grant change, an optional one turned on in the dialog included, runs
the dangerous combination check on what the plugin will hold afterwards, so a plugin cannot ask
for little and gain the rest one grant at a time.

**Why this priority**: an update is where a trusted name can be taken over (repo-jacking) or a
plugin can escalate step by step.

**Independent Test**: install a test widget; publish a release that adds a required connector;
check the dialog lists every permission, connector and destination with the new connector marked
as added, and that the old version keeps running until it is approved. Delete and recreate the repository under the same name, publish a release, and check
the update is refused naming the identity change. In the dialog, turn on an optional connector
whose addition completes a dangerous combination and check the warning appears.

**Acceptance Scenarios**:

1. **Given** an installed widget, **When** a newer release asks for nothing new, **Then** it
   installs after the person confirms, with the grants unchanged.
2. **Given** an installed widget, **When** a newer release requires a permission or connector it
   did not, or changes what an operation sends or where, **Then** the dialog opens showing every
   permission, connector and destination of the new version with what changed marked, and until it
   is approved the installed version keeps running.
3. **Given** the repository's name now belongs to a repository with another permanent identifier,
   **When** the host checks for updates, **Then** every release from it is refused, naming the
   identity change, and the installed widget keeps its version and grants.
4. **Given** a release whose tag the host already pinned now carries other bytes, **When** the host
   sees it, **Then** it is refused as a changed release.
5. **Given** the installation or update dialog, **When** the person turns on an optional connector
   and that completes a dangerous combination, **Then** the warning appears in the dialog before
   anything is approved, and turning it off again removes it.
6. **Given** a release older than or equal to the installed one, **When** offered as an update,
   **Then** it is refused.

---

### User Story 4 - A hostile package cannot hurt the host while it is installed (Priority: P1)

An attacker publishes a package crafted to be slow or memory-hungry to compile, or one that is
malformed, oversized or tampered with. The host compiles it in a separate process under
operating-system limits on time and memory, so the app stays responsive, and refuses it if a limit
is hit. The host loads only compiled forms it produced itself, bound to the package's digest.

**Why this priority**: decision 0001 accepted the compile-time risk only while the maintainer
alone loaded plugins; installation ends that condition.

**Independent Test**: install hostile packages from test repositories: one that exhausts the
compile time limit, one that exhausts memory, one over the size cap, one whose compiled form is
altered on disk after installation; check each is refused or recompiled, and the window keeps
answering input throughout.

**Acceptance Scenarios**:

1. **Given** a package crafted to compile slowly, **When** it is installed, **Then** compilation
   stops at the time limit, the installation is refused naming the limit, and the app stays
   responsive.
2. **Given** a package crafted to use more memory than the limit while compiling, **When** it is
   installed, **Then** the compiling process is stopped and the installation refused; the app's
   own memory is unaffected.
3. **Given** a compiled form on disk that no longer matches what the host produced, **When** the
   widget loads, **Then** the host refuses that compiled form and compiles the package again, or
   refuses the widget, and never runs the altered form.
4. **Given** a package over the size cap, or a release with no package, **When** it is installed,
   **Then** it is refused before compiling.

---

### User Story 5 - Host dialogs cannot be imitated, hijacked or rushed (Priority: P2)

Every host dialog covers the chrome and every frame, makes the rest of the window inert, ignores
input for a short moment after it appears, defaults to the safe choice, keeps focus inside, and
works with the keyboard and a screen reader on the three systems. A widget on the dashboard cannot
answer it, cover it or click through it.

**Why this priority**: these are the properties that tell a real host dialog from a picture of one
(feature 007); the installation dialog is the first dialog that grants anything.

**Independent Test**: run the checks HS-10 to HS-15 and HS-17 to HS-23 name, with the showcase and
a hostile widget on the dashboard while the dialog is open.

**Acceptance Scenarios**:

1. **Given** an open dialog, **When** the pointer or keyboard targets the chrome or a frame,
   **Then** nothing happens, and the accessibility tree holds only the dialog (HS-10, HS-11,
   HS-22).
2. **Given** a dialog that just opened, **When** a click on the approve control or Enter arrives
   within the input delay, **Then** it does nothing; after the delay Enter takes the safe choice
   (HS-12, HS-14).
3. **Given** an open dialog, **When** the person presses Escape with focus on any control, **Then**
   the safe choice is taken and focus returns to the control that opened it (HS-19, HS-20).
4. **Given** a dialog with more permissions than fit a small window, **When** it opens, **Then** its
   body scrolls while the title, who is asking and the safe choice stay in view (HS-15).
5. **Given** forced colors or reduced motion, **When** a dialog opens, **Then** it keeps its form
   and a visible edge, and appears without animation (HS-17, HS-23).

---

### Edge Cases

- The code host is unreachable, rate-limits the app, or the download breaks off: installation
  fails, naming why, and nothing is kept.
- A repository with no release, or whose latest release has no package or several: refused,
  naming what was found.
- A repository renamed by its owner: the stored permanent identifier still matches, but the
  plugin id no longer does; updates are refused, naming the rename, and the person may install it
  again under the new id as a new plugin.
- The repository is deleted: updates fail, naming it; the installed version keeps running.
- The same repository asked for twice: the second request names that it is already installed and
  offers its updates instead.
- A widget declares a connector from a repository: installation is refused, naming that
  connectors from repositories arrive with the item that verifies their owners' domains; the
  widget is not installed without it, even when the connector is optional.
- A widget declares a `local/` connector: refused; local connectors exist only in the development
  tool.
- A widget declares a catalog connector major the host does not ship: refused before the dialog,
  naming the connector and major.
- The window closes, or the app quits, while the dialog is open: the safe choice is taken.
- Two installations at once: one dialog at a time; the second waits or is refused, never shown
  beneath the first.
- The person's grant record from the development tool exists: the app never reads it, and a
  plugin installed in the app gets nothing the development tool granted (feature 009, FR-024).
- A widget that is installed and also bundled with the app under the same id cannot exist: a
  repository id is never a `local/` one.

## Requirements *(mandatory)*

### Functional Requirements

#### Installing from a repository

- **FR-001**: The app MUST install plugins only from their author's repository on `github.com`,
  one plugin per repository, at a release; it MUST NOT install from a file, an arbitrary URL or
  any other host. Local packages stay in the development tool only.
- **FR-002**: Installation MUST start only from a host control in the chrome; no plugin, widget
  content or AI output MAY start, fill in or confirm it (Principle VII).
- **FR-003**: The host MUST check that the manifest id equals the repository the package was
  downloaded from, and refuse it otherwise.
- **FR-004**: At installation the host MUST record the release's tag, the digest of its package,
  and the repository's permanent identifier. The same tag with other bytes MUST be refused as a
  changed release. Updates MUST come only from the repository with the recorded permanent
  identifier.
- **FR-005**: The package MUST respect the existing size cap and be refused before compiling
  otherwise.
- **FR-006**: Installation MUST be all or nothing: a refusal, a failure or a cancel at any step
  keeps no package, compiled form, grant or data.

#### Compiling away from the app

- **FR-007**: The host MUST compile an installed package in a separate process under
  operating-system limits on time and memory, so that a hostile package cannot freeze the window
  or exhaust the app's memory, and MUST refuse a package that hits a limit, naming it.
- **FR-008**: The host MUST load only a compiled form it produced, bound to the package's digest
  and checked when loaded; one that does not match MUST NOT run.
- **FR-009**: Decision 0001 MUST be reviewed and its outcome recorded before the app installs a
  plugin it did not build: compiling moves out of the app's process in this feature; running
  stays in it until the next feature, which the record names with its reason.
- **FR-029**: The compile process MUST be confined by the operating system so that it can read its
  input and write its output and nothing else: no file, network, keychain or other process. A
  breaking test MUST show it cannot open a file or a socket on each of the three systems.

#### The installation dialog

- **FR-010**: Before a plugin first runs, the host MUST show the installation dialog, which names
  who is asking (manifest name and repository id, as HS-13 states) and every permission and
  connector the plugin declares. The author the manifest declares is shown sanitised and bounded
  like the name, and labelled as declared by the plugin.
- **FR-030**: The frame of every installed widget MUST show its repository owner
  (`github.com/owner`), drawn by the host as escaped text; bundled widgets show none.
- **FR-011**: Every permission kind the host knows MUST have a sentence written by the host
  describing, in plain language, what the plugin can do with it. A permission kind with no
  sentence MUST fail the build.
- **FR-012**: For every connector the dialog MUST show what the development tool shows today
  (feature 003, FR-014): its id and version, where its destinations come from, every destination
  host, and each operation the widget calls with the parameters it sends and its kind, read or
  change, marked as reviewed in the catalog or as declared. For an account connector it MUST name
  the service whose account data the widget reads.
- **FR-013**: Required permissions and connectors MUST be approved together or not at all.
  Optional ones MUST start off and be turned on one at a time in the dialog.
- **FR-014**: The approval MUST cover exactly what the dialog showed: the plugin's declarations,
  the optionals the person turned on, and the warning shown for them. If any of these differs from
  what the host last showed, approving MUST fail and nothing is granted. An installation grants the
  required items and the optionals turned on in the dialog, and nothing left over from an earlier
  record.
- **FR-015**: Installation MUST refuse a widget that declares a connector from a repository or a
  `local/` connector, naming why, until the item that verifies repository connectors arrives.

#### The dangerous combination warning

- **FR-016**: On every grant change (installation, update approval, turning an optional
  permission or connector on, in the app or in the development tool) the host MUST judge the
  grants the plugin will hold after the change, not the change alone. A source of account data the
  plugin held earlier, while it could keep data in its store, MUST still count as a source until
  that store is cleared, so that data kept from an account cannot leave through a destination
  added later.
- **FR-017**: When those grants let data read as a connected account, or from the user's machine
  or local network, reach another destination, the dialog MUST warn, naming the source and every
  destination it can reach, before the change is approved.
- **FR-018**: The warning MUST NOT be suppressible by the plugin, and MUST NOT appear when no such
  flow exists.

#### Updates

- **FR-019**: The person MUST be able to check an installed plugin for a newer release. Only a
  release with a higher version from the recorded repository MAY be offered.
- **FR-020**: An update that requires no new grant MUST install after the person confirms it in a
  host dialog; one that requires a new grant, or changes a connector's destinations, operations,
  parameters or kind as feature 003 defines, MUST open the installation dialog showing every
  permission, connector and destination of the new version, with what changed marked (Principle
  III). Until approved, the installed version keeps running with its grants.

#### Host dialogs

- **FR-021**: Every host dialog in this feature MUST meet HS-10 to HS-15 and HS-17 to HS-23, each
  with the check its criterion names, and the threat-model rows that cite them MUST point to
  those checks.
- **FR-022**: The input delay of HS-12 MUST be long enough that a click or key press aimed at what
  was under the pointer or caret cannot answer the dialog, and is set and measured in the plan. It
  MUST start again when the window regains focus, and while input aimed at the dialog keeps
  arriving during it.
- **FR-023**: Closing the window or quitting the app with a dialog open MUST take the safe choice.

#### Accounts

- **FR-024**: A widget with an account connector MAY be installed: the dialog shows the account
  use (FR-012) and the warning (FR-017) as for any widget, but the app has no account to give it,
  so its calls through that connector answer `account-not-connected` until a later feature brings
  the host dialog for connecting accounts. The app MUST NOT read the development vault.

#### The window and the security suite

- **FR-025**: Every host command the window gains MUST be added to feature 008's reviewed list
  with its reason, and the window's protections MUST still hold, with the shell probe green on the
  three systems.
- **FR-026**: The security suite MUST gain a breaking case for each rule in FR-001 to FR-008,
  FR-013 to FR-020, FR-022, FR-023, FR-029 and FR-030, and each MUST fail the build when its rule
  is broken (Principle VIII). Where no automated check can reach what a criterion states on a
  system (a trusted key press, the accessibility tree, forced colors), the plan says so and the
  review step covers it.
- **FR-027**: The threat-model rows waiting on installation (a package crafted to compile slowly,
  repo-jacking, a connector from a repository without its owner's authorization, HS-10 and HS-11,
  look-alike names) MUST name the tests that now cover them. The distribution boundary MUST state
  what this feature provides and what waits for signatures, and the threat model MUST gain rows
  for its residuals: a name trusted at first installation, an author account taken over that
  ships an update asking for nothing new, no revocation yet, the compile process's output trusted
  as code, and the host's own requests to GitHub.
- **FR-028**: The author documentation MUST explain how to publish a release the app can install.

### Key Entities

- **Installed plugin**: a plugin the app installed: its repository id, permanent repository
  identifier, pinned release (tag and package digest), version, compiled form and grants.
- **Pinned release**: a release's tag and its package digest as recorded at installation.
- **Compiled form**: the host's compilation of a package, bound to its digest; host data no plugin
  can reach.
- **Grant change**: installation, update approval, or an optional permission or connector turned
  on; each is judged on the grants that follow it.
- **Dangerous combination**: a set of grants under which data from a connected account, or from the
  user's machine or local network, can reach another destination.
- **Plain-language description**: the host's sentence for a permission kind; never plugin text.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A person installs a widget from its repository address to its first render in under
  1 minute on a normal connection, with exactly one dialog asking for approval.
- **SC-002**: 100% of the new hostile cases are refused on Windows, macOS and Linux.
- **SC-003**: While a hostile package compiles until it hits a limit, the window keeps answering
  input within 100 ms on all three systems.
- **SC-004**: Every permission kind and every connector property the manifest can declare appears
  in the dialog; a test fails if one is missing.
- **SC-005**: The dangerous combination warning appears for every grant change that completes such a
  combination and for none that does not, across every combination the tests enumerate.
- **SC-006**: Every check HS-10 to HS-15 and HS-17 to HS-23 names passes, its tests in CI and its
  review steps walked on the three systems, keyboard and screen reader included.
- **SC-007**: No update from a repository whose permanent identifier changed, and no changed
  release under a pinned tag, ever installs.

## Assumptions

- One person uses the app on a machine, as in features 002 to 009.
- The repository address is typed by the person; a plugin store and search arrive in Phase 6.
- Repository ids accept `github.com` only, as feature 002 decided.
- Package signatures, which prove who published a release, arrive in Phase 6; the pinned release
  and the permanent identifier are what this feature relies on.
- Turning optional permissions on and off after installation, disabling and uninstalling are the
  guide's separate item; this feature turns optional ones on in the installation dialog and runs
  the dangerous combination check wherever an optional grant is turned on.
- Connectors from repositories, protocol connectors and scheduled updates are the guide's
  separate item, which feature 003 named; until it arrives, installation offers only catalog
  connectors.
- Installed widgets appear in the window as the showcase does; placing and resizing them is the
  dashboard grid item.
- The numbers (compile time and memory limits, input delay) are set in the plan and measured on
  the three systems, as features 001 to 009 did.
- The installer, its compile process, the grant and installed records, the host dialogs and the
  catalog are security core (Principle IX, v2.7.0) and get line-by-line review.
- Installed plugins run in the app's process, as bundled ones do; running them in a process of
  their own, away from the keychain, is the next feature, before the alpha (decision 0003).
- Local connectors are refused in the app, so in this feature the dangerous combination warning is
  exercised through account connectors; the local-network source is judged the same way once a
  feature lets the app reach the user's network.
- Out of scope: the plugin store and signatures (Phase 6); the host dialog for connecting
  accounts; repository, protocol and local-service connectors; disabling, uninstalling and later
  optional toggles; the dashboard grid; the official widgets.
