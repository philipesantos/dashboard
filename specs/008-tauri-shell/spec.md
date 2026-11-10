# Feature Specification: The Desktop Shell, with Minimal Capabilities and a Restrictive CSP

**Feature Branch**: `008-tauri-shell`

**Created**: 2026-11-10

**Status**: Draft

**Input**: User description: "Item 'Shell Tauri com capabilities mínimas e CSP restritiva' da Fase 3
do guia. O shell desktop que hospeda a UI do host: capabilities mínimas do Tauri (só os comandos e
plugins que a UI do host usa; sai o opener:default e o comando greet do template), CSP restritiva
no lugar do "csp": null de hoje (nada de origem remota, sem script inline, fontes e assets
empacotados), navegação e novas janelas bloqueadas, devtools só em build de desenvolvimento. Motor
da webview: nativo no Windows (WebView2) e no macOS (WKWebView); no Linux, Chromium embarcado
(CEF, via o runtime CEF do Tauri) no lugar do WebKitGTK, decisão do mantenedor de 2026-11-10
porque o WebKitGTK não é confiável. Isso exige emendar a constituição (a tabela de decisões fixas
diz "Rust + Tauri 2, instaladores leves" e "UI roda na webview do Tauri") e um teste rápido do
runtime CEF no Linux antes de fixar a escolha; o projeto passa a entregar as correções de
segurança do Chromium. Critério herdado da feature 007: HS-26, autofill e salvamento de senha
desligados no motor de cada sistema. Fecha a pergunta aberta do threat model sobre a CSP. A
decisão 0001 (runtime de plugins em processo separado) deve ser revista no plano da Fase 3, no
mais tardar."

## Clarifications

### Session 2026-11-10

- Q: Does this feature only lock down today's shell, or also connect the plugin runtime and define
  the host commands for widgets? → A: Lock down only. The shell ends with a reviewed list of almost
  no host commands. The runtime and each later command arrive with the feature that needs them,
  each added to the list.
- Q: What happens on Linux if the embedded engine fails the trial? → A: Linux keeps the system
  WebKitGTK, with every protection and test of this feature, recorded in the constitution as
  temporary. The embedded engine is tried again with each release of Tauri's runtime for it.
- Q: How fast must a release follow a Chromium security fix? → A: Critical and high fixes ship
  within 14 days of the release of Tauri's embedded-engine runtime that carries them; other fixes
  ship with the next regular release. This applies from the first public release.

## User Scenarios & Testing *(mandatory)*

The people in these stories are:

- the **person** who uses the dashboard;
- the **maintainer**, who owns the host and decides what it may do;
- a **contributor**, who adds a host function the UI needs;
- an **attacker**, who controls something that reaches the window: widget content from a
  plugin, a string from an external service, or a link the person is shown.

Terms used throughout:

- **The window** is the app's single window, where the host UI runs.
- **The engine** is what draws the window: the system webview on Windows and macOS, and an
  embedded Chromium on Linux.
- **A host command** is a function of the app's native side that the window may call.
- **The content policy** is the list of what the window may load and run.

### User Story 1 - The window runs only what the app shipped (Priority: P1)

The person opens the dashboard. Everything the window runs, its scripts, styles, fonts and
images, comes from inside the installed app. If anything that reaches the window tries to load a
script, a style, a font or an image from the internet, or to run a script written into the page,
the window refuses it and keeps working.

**Why this priority**: the host UI draws text from plugins and external services. If a single
string could become script, or could make the window fetch a URL, an attacker could act as the
host or send the person's data away. Today the app ships with no content policy at all.

**Independent Test**: in a built app on each system, a test page tries each forbidden load and
each forbidden script. Every attempt is refused, and the demo UI still works.

**Acceptance Scenarios**:

1. **Given** the built app, **When** the window starts, **Then** it runs under a content policy
   that allows scripts, styles, fonts and images only from the app's own bundle, and connections
   only to the host.
2. **Given** content that tries to run a script written into the page, or to evaluate text as
   code, **When** the window processes it, **Then** nothing runs, and the window keeps working.
3. **Given** content that tries to load a script, style, font, image or frame from any internet
   address, **When** the window processes it, **Then** nothing is fetched.
4. **Given** a refused load or script, **When** it happens in a development build, **Then** the
   developer sees which rule refused it.

---

### User Story 2 - The window reaches only the host functions it needs (Priority: P1)

The window can call only the host commands the host UI uses, each one on a reviewed list with
the reason it exists. Anything else the window asks for is refused. A contributor who adds a
host command adds it to the list in the same change, and CI fails if the list and the app
disagree.

**Why this priority**: the window is the most exposed part of the app, because it draws content
the app does not control. Every host command it can reach is something an attacker who got
script into it could use. The template's leftovers, a sample command and the right to open
links and files in other apps, are reachable today and used by nothing.

**Independent Test**: list what the built window may call, and compare it with the reviewed
list. Then, from the window, call a command that is not on it and a removed permission, such as
opening a link in the browser. Both are refused.

**Acceptance Scenarios**:

1. **Given** the built app, **When** its capabilities are listed, **Then** they equal the
   reviewed list exactly, and the list gives a reason for each entry.
2. **Given** a host command or permission added to the app but not to the list, or listed but
   not in the app, **When** CI runs, **Then** it fails and names the difference.
3. **Given** the window, **When** it asks to open a link, a file or another app, **Then** the
   request is refused. This holds for the right the template granted to open links in the
   browser.

---

### User Story 3 - The window stays the dashboard (Priority: P1)

Whatever the window shows, it never leaves the app. A link, a script or a dropped file cannot
send the window to another page, and nothing in it can open a second window or a popup. In a
release build, nobody can open the engine's developer tools.

**Why this priority**: a window that navigated to an attacker's page would still look like the
app, with the host's chrome around a page the host does not control. Developer tools in a
release build would let anyone run script in the host's window.

**Independent Test**: in a built release app on each system, try every way of navigating away
and of opening a window, and try to open the developer tools. Each is refused, and the window
still shows the dashboard.

**Acceptance Scenarios**:

1. **Given** the window, **When** a link, a script, a form or a dropped file or URL tries to
   load another page, **Then** the window stays on the app's own page.
2. **Given** the window, **When** content tries to open a new window, a popup or a tab,
   **Then** nothing opens.
3. **Given** a release build, **When** the person presses the shortcut for developer tools or
   uses the context menu, **Then** no developer tools open. A development build still has them.

---

### User Story 4 - On Linux, the app ships its own engine (Priority: P2)

On Linux, the app draws its window with an embedded Chromium it ships and keeps up to date,
instead of the system's WebKitGTK. Windows and macOS keep their system webview. The person on
Linux gets the same app, with the same protections, whatever their distribution ships. Before
the project commits to this, a quick trial confirms that the embedded engine can do everything
this feature requires.

**Why this priority**: the maintainer decided on 2026-11-10 that WebKitGTK is not reliable
enough for the host's window. This is a change to the constitution's fixed decisions, and it
makes the project responsible for the engine's security updates on Linux. It comes after
Stories 1 to 3 because those protections are needed on every system whichever engine draws the
window.

**Independent Test**: install the Linux build on a system without WebKitGTK and open it. The
dashboard works, and Stories 1, 2, 3 and 5 hold. The trial's report shows each requirement met
by the embedded engine, with the installer size and start-up time beside the other systems'.

**Acceptance Scenarios**:

1. **Given** the Linux build, **When** it is installed on a system without WebKitGTK, **Then** it
   starts and shows the dashboard.
2. **Given** the trial of the embedded engine, **When** it ends, **Then** its report states, for
   each requirement of this feature, whether the engine meets it, and the installer size and
   start-up time on Linux compared with Windows and macOS.
3. **Given** the trial shows the engine cannot meet a requirement, **When** the maintainer
   decides, **Then** Linux keeps the system WebKitGTK for now, with every protection and test of
   this feature, and the constitution records it as temporary. The embedded engine is tried
   again with each release of Tauri's runtime for it.
4. **Given** the constitution, **When** this feature merges, **Then** its fixed decisions name
   the engine of each system, and the amendment records why.

---

### User Story 5 - No engine offers to fill or save a password (Priority: P2)

No field in the window, whatever its label, is offered a saved password or saved as one, on any
system. Feature 007's criterion HS-26 assigned this to the shell.

**Why this priority**: a widget can draw a text field labelled "password". If the engine offered
the person's saved password for it, or offered to save what they type, the engine would be
helping a fake login. The widget's own form already has no password field (HS-06).

**Independent Test**: on each system, open a test form with a field named and labelled as a
password. No autofill offer appears, and after submitting, no offer to save appears. The
engine's settings read as off.

**Acceptance Scenarios**:

1. **Given** each system's engine, **When** the app starts, **Then** form autofill and password
   saving are off.
2. **Given** a field named and labelled as a password, **When** the person focuses it, types and
   submits, **Then** nothing is offered and nothing is saved.

---

### User Story 6 - The maintainer knows the engine is current (Priority: P3)

The maintainer can see which engine version each release ships on Linux, and learns when a
security update for it is due. From the first public release, critical and high Chromium
security fixes ship within 14 days of the release of Tauri's embedded-engine runtime that
carries them; other fixes ship with the next regular release.

**Why this priority**: with an embedded engine, nobody else patches it. Without a check, an old
engine with known holes would ship quietly.

**Independent Test**: the release notes name the embedded engine's version. A scheduled check
reports when the shipped version is behind a security release.

**Acceptance Scenarios**:

1. **Given** a release, **When** the maintainer reads its notes, **Then** they name the version
   of the engine it embeds on Linux.
2. **Given** a Chromium security release newer than the embedded one, **When** the scheduled
   check runs, **Then** it reports the gap and its severity.
3. **Given** a release of Tauri's embedded-engine runtime that carries a critical or high
   Chromium fix, **When** 14 days pass, **Then** a release of the app ships it.

---

### Edge Cases

- **A component library needs inline styles.** Some UI components set styles on elements at run
  time. The policy allows only what the components need, and the plan records each allowance
  and why. A style attribute cannot run script.
- **The development build.** It loads from a local development server with hot reload, which
  needs more than the release policy allows. The development policy differs from the release one
  only where the server needs it, and the release policy is what the tests check.
- **A plugin string that looks like a URL.** It is drawn as text (criterion HS-24). Nothing the
  window shows can open it (HS-07), because the window has no right to open links.
- **The person really wants to open a link**, for example a plugin's repository in the install
  dialog. That is a host feature for later. It goes through a host command that opens only
  addresses the host built itself, never one from plugin content, and it arrives with the
  feature that needs it.
- **Dropping a file onto the window.** It does not navigate and does not open the file.
  Installing from a file is not supported (plugins install only from repositories).
- **The engine itself is missing or broken** on Windows (no WebView2 runtime). The installer
  brings or requires it, as Tauri does today; the app explains the problem instead of opening an
  empty window.
- **A refused load in the release build.** It is refused silently, and the app keeps working.
  Nothing about it reaches the person as an error dialog.
- **A Linux distribution without the libraries the embedded engine needs.** The installer
  declares them, and the app does not start with a half-drawn window.

## Requirements *(mandatory)*

### Functional Requirements

#### The content policy

- **FR-001**: The release app MUST run its window under a content policy that allows scripts,
  styles, fonts and images only from the app's own bundle, and connections only to the host's
  own channel. It MUST forbid: script written into the page or in attributes, evaluating text as
  code, plugins and embedded objects, frames, any internet origin, a changed base address, and
  form submission anywhere.
- **FR-002**: The policy MAY allow inline style attributes only where the UI components need
  them, and the plan MUST record each allowance and the reason.
- **FR-003**: A development build MAY relax the policy only where the local development server
  needs it, and the plan MUST list each relaxation. The tests check the release policy.
- **FR-004**: A refused load or script MUST NOT stop the window from working. In a development
  build, the refusal MUST be visible to the developer with the rule that refused it.

#### Capabilities and host commands

- **FR-005**: The window MUST have only the capabilities and host commands that the host UI uses.
  The template's sample command and its right to open links and files in other apps MUST be
  removed.
- **FR-006**: The repository MUST hold a reviewed list of the window's capabilities and host
  commands, each with its reason. CI MUST fail when the built app and the list disagree, naming
  each difference.
- **FR-007**: A request from the window for a capability or command not on the list MUST be
  refused.
- **FR-008**: This feature adds no host command beyond what today's host UI needs. Every later
  host command, including those that connect the plugin runtime, arrives with the feature that
  needs it and enters the reviewed list in the same change.

#### Navigation, windows and developer tools

- **FR-009**: The window MUST never leave the app's own page. Navigation started by a link, a
  script, a form, a redirect or a dropped file or URL MUST be refused.
- **FR-010**: Content MUST NOT open a new window, a popup or a tab.
- **FR-011**: Developer tools MUST be unavailable in release builds, by shortcut, by context menu
  and by any other means. Development builds keep them.
- **FR-012**: Dropping a file or URL on the window MUST NOT open, load or read it.

#### The engine on each system

- **FR-013**: The window MUST be drawn by WebView2 on Windows and WKWebView on macOS, and by an
  embedded Chromium that the app ships on Linux. The Linux app MUST NOT need WebKitGTK.
- **FR-014**: Before the Linux engine is fixed, a trial MUST check that the embedded engine meets
  FR-001 to FR-012 and FR-016 on Linux, and builds and passes the security tests in CI. The trial
  MUST record the installer size and start-up time beside Windows and macOS. If the engine
  fails a requirement, Linux keeps the system WebKitGTK with every protection of this feature,
  recorded as temporary, until the embedded engine passes a later trial.
- **FR-015**: The constitution MUST be amended before this feature's plan passes its Constitution
  Check. The amendment updates the fixed decisions to name each system's engine, states why, and
  records the project's duty to ship the embedded engine's security fixes within the deadline
  of User Story 6. It goes through the independent review that follows every amendment.
- **FR-016**: Each engine's form autofill and password saving MUST be off, on every system
  (criterion HS-26 in `docs/design/host-surfaces.md`).
- **FR-017**: Each Linux release MUST name the embedded engine's version in its notes. A
  scheduled check, at least weekly, MUST report when that version is behind a Chromium security
  release, with its severity. From the first public release, a critical or high fix MUST ship
  within 14 days of the release of Tauri's embedded-engine runtime that carries it.

#### Security tests and the threat model

- **FR-018**: Every rule in FR-001, FR-005 to FR-012 and FR-016 MUST have a breaking test that
  runs against the built app on Windows, macOS and Linux in CI and fails if the rule is broken
  (Principle VIII).
- **FR-019**: The threat model MUST close its open question about the content policy, describe
  the window as a boundary in its own right, and gain a row for each attack these rules stop:
  injected script, loads from the internet, navigation away, new windows, misuse of host
  commands, developer tools, and engine autofill. Each row cites its breaking test.
- **FR-020**: The demo UI MUST keep working as it does today on all three systems, with its
  bundled fonts and their licenses.

### Key Entities

- **Content policy**: the rules the engine enforces on what the window loads and runs, one for
  release builds and one for development builds, with each allowance and its reason.
- **Capability list**: the reviewed list of what the window may call: each host command and
  each engine permission, with its reason.
- **Engine**: what draws the window on each system, with its version and, on Linux, whether it
  is current with Chromium's security releases.
- **Engine trial report**: for the Linux engine, each requirement met or not, the installer size
  and the start-up time, compared with the other systems.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On each of the three systems, 100% of the forbidden actions in FR-001 and FR-009 to
  FR-012 are refused in the built app, in CI, and the demo still works afterwards.
- **SC-002**: The window can reach zero capabilities and host commands outside the reviewed list.
  Adding one without listing it fails CI by name.
- **SC-003**: The Linux build starts and shows the dashboard on a system without WebKitGTK.
- **SC-004**: The trial report gives the installer size and the cold start-up time of each system,
  and states whether the embedded engine meets each requirement.
- **SC-005**: Every release's notes name the embedded engine's version. A security release newer
  than it is reported within one week of its publication, and from the first public release no
  critical or high fix waits more than 14 days after Tauri's runtime carries it.
- **SC-006**: On each system, a password-shaped field draws no autofill offer and no offer to
  save.
- **SC-007**: The threat model has no open question about the content policy, and each new row
  names a breaking test that runs in CI.
- **SC-008**: The demo UI looks and behaves as before on all three systems.

## Assumptions

- **Decision 0001 is revisited with installation.** Its trigger is "before the app can install a
  plugin it did not build itself", which is the Phase 3 installation item. This feature does not
  bring the plugin runtime into the app (FR-008), so it does not change where plugins run.
- **One window.** The app keeps a single window. Host dialogs are drawn inside it (feature 007).
- **The installer formats stay** what the app produces today on each system. The plan checks
  which Linux formats can carry the embedded engine.
- **Mobile is out of scope.**
- **Updates of the app itself** (the signed updater) are Phase 6. This feature records the
  engine version and reports when it is behind; delivering updates to installed apps comes with
  the updater.
- **The look does not change.** Feature 007's visual system and criteria apply unchanged.
