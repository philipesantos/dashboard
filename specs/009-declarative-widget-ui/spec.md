# Feature Specification: Declarative Widget UI and the Host Frame

**Feature Branch**: `009-declarative-widget-ui`

**Created**: 2026-11-10

**Status**: Draft

**Input**: User description: "Itens 'Biblioteca de UI declarativa: texto, lista, tabela, gráfico,
botão e formulário simples, sem campo de senha' e 'Moldura do host em cada widget com nome e ícone
do plugin; widgets não abrem janelas nem diálogos' da Fase 3 do guia, juntos numa feature (decisão
do mantenedor em 2026-11-10: a grade do dashboard vem depois, porque precisa de widgets de
verdade). O objetivo é o primeiro widget de verdade na janela: o host roda um plugin (o de exemplo,
empacotado com o app; instalação de terceiros fica para o item de instalação) e desenha o conteúdo
que o render dele devolve, dentro de uma moldura que só o host desenha, com o nome e o ícone vindos
do manifesto. O contrato v1 diz só que o render devolve 'um documento JSON de até 64 KiB'; esta
feature define o formato desse conteúdo (o vocabulário da UI declarativa: texto, lista, tabela,
gráfico, botão e formulário simples, sem campo de senha, estilo só por tom e variante dos papéis
compartilhados do DESIGN.md), com schema publicado para autores como o manifest.schema.json, e o
que o host faz com conteúdo inválido. Botão e formulário só disparam as ações declaradas do próprio
widget. Critérios herdados da feature 007 (docs/design/host-surfaces.md): HS-01 a HS-04 e HS-16
(moldura) e HS-05 a HS-09, HS-24 e HS-25 (biblioteca), cada um com o teste ou caso hostil que ele
nomeia; as linhas B4 do threat model na Fase 3 citam esses critérios. Os comandos do host que a
janela passa a chamar entram pela lista revisada da feature 008 (window-surface.toml, núcleo de
segurança), e as proteções da janela da 008 continuam valendo (o teste da janela segue verde). A
decisão 0001 (plugins no processo do host) continua até o item de instalação, que é o gatilho
dela."

## Clarifications

### Session 2026-11-10

- Q: The frame shows the plugin's name and icon, which HS-02 says come from the manifest, but the
  manifest has only an id, a version and an author. Where do they come from? → A: The manifest
  gains optional `name` and `icon` fields, an additive change to its schema. The icon is a small
  image carried in the manifest, under a size limit. Without them, the host shows the end of the
  id and an icon it generates. Installation and the store will reuse the same fields.
- Q: A button or form triggers only an action its widget declared. What does the action run, since
  contract v1 has only render and update? → A: A new contract minor, 1.4.0, that only adds an
  `actor` export (so named in the plan; the manifest field that lists them is `actions`). The
  manifest declares the action names. The host runs an action as it runs an
  update: it may write the widget's store and call its connectors, as granted, and it returns the
  widget's data, which the host keeps and gives to the next render. Then the host draws the
  widget again.
- Q: (constitution review) Are actions the "user action" that connector operations which change
  data have waited for? → A: No. In this feature an action may write its widget's store and call
  only operations that read. Operations that change data stay refused, as today, until a feature
  designs how the host shows and approves them.
- Q: (constitution review) Principle IV says plugins never receive passwords, OAuth tokens or AI
  provider keys, but a free-text field can receive a token the person pastes. How is it handled?
  → A: The host refuses form values that contain a secret it holds, and a PATCH amendment states
  that Principle IV covers the secrets the host holds or that pass through its credential flows,
  since text a person types cannot be recognised in general.

- Q: (plan review, 2026-11-11) The data an action returns would be kept on disk, up to 256 KiB
  per widget, with no storage permission or quota, and may hold what the person typed. Is that
  acceptable? → A: No. The host keeps an action's data in memory only, while the app runs; state
  that must survive a restart needs the `storage` permission and its approved quota.

## User Scenarios & Testing *(mandatory)*

The people in these stories are:

- the **person** who uses the dashboard;
- a plugin **author**, who writes a widget and its content;
- the **maintainer**, who owns the host and reviews what the window may call;
- an **attacker**, who controls a plugin, or text from an external service that a plugin shows.

Terms used throughout:

- **Widget content**: what a widget's render returns, a document the host turns into UI. Only the
  host draws it, with the host's own components.
- **Component**: one kind of thing widget content can ask for: text, list, table, chart, button
  and simple form.
- **Tone** and **variant**: the only styling widget content can choose, by name, from the shared
  roles of the visual system (`DESIGN.md`, feature 007).
- **Frame**: the box the host draws around each widget, with a header (the plugin's name, icon and
  the host's actions) and a body (the widget content).
- **Declared action**: something a widget's button or form may ask the host to do for that widget.

### User Story 1 - The person sees a real widget on the dashboard (Priority: P1)

The person opens the app and sees a widget from a plugin, not a static demo: the plugin's content,
drawn by the host in the app's visual style, inside a frame that shows the plugin's name and icon.
The widget follows the light and dark themes like the rest of the app.

**Why this priority**: it is the first time a plugin is visible in the app. Everything later in
Phase 3, the grid, the official widgets and installation, builds on a widget drawn inside a frame.

**Independent Test**: open the app; the bundled example widget appears in its frame, with its name
and icon in the header and its content in the body, in both themes.

**Acceptance Scenarios**:

1. **Given** the app has started, **When** the main view opens, **Then** the bundled example
   widget is shown in a host frame within 1 second, with the plugin's name and icon in the header.
2. **Given** a widget showing content, **When** the person switches between light and dark,
   **Then** the widget's content and frame follow the theme, with the contrast of feature 007.
3. **Given** the bundled example widget, **When** it renders, **Then** it shows every component of
   the declarative UI, so the person and the reviewer see the whole vocabulary in the real app.

---

### User Story 2 - An author writes widget content from a published format (Priority: P1)

An author learns from the published schema and the docs what widget content can hold, writes it,
and checks it before running the widget. When content is wrong, the author sees why, in the frame
and in the plugin's log, and the rest of the dashboard keeps working.

**Why this priority**: the contract says only that render returns a JSON document. Without a
published format, no author can write a widget, and the host cannot tell valid content from
hostile content.

**Independent Test**: validate the docs' example content against the published schema; render a
widget that returns invalid content and check the error state and the log entry.

**Acceptance Scenarios**:

1. **Given** the published schema and the author docs, **When** an author writes content using
   every component, **Then** it validates against the schema and draws as the docs describe.
2. **Given** a widget whose render returns content that is not valid (not JSON, a shape the schema
   rejects, over a limit, or a render failure), **When** the host draws it, **Then** the frame shows
   a host-drawn error state with a short reason, the plugin's log gets one entry naming the
   problem, and every other widget is unaffected.
3. **Given** content that names a tone the host does not know, **When** it draws, **Then** the text
   draws in the default tone and the plugin's log gets one entry naming the tone (HS-09).
4. **Given** content with a component type the host does not know, **When** it draws, **Then** the
   host draws its own placeholder in its place, logs it once, and draws the rest of the content.

---

### User Story 3 - Widget content stays inside its frame and cannot pass for the host (Priority: P1)

Whatever a plugin returns, its content stays inside its frame's body. It cannot change the frame's
header, cover it, draw outside the body, run markup, open anything, or produce something that looks
and behaves like a host dialog.

**Why this priority**: Principle V. The frame is how the person tells the host from a plugin, and a
widget that escapes it can phish for credentials or trust decisions.

**Independent Test**: run the hostile widgets of the criteria (HS-01, HS-03, HS-04, HS-07, HS-16,
HS-24, HS-25) in CI and check each one's stated outcome.

**Acceptance Scenarios**:

1. **Given** a widget whose content is shaped like a frame header with another plugin's name,
   **When** it draws, **Then** the real header stays above it, unchanged, with the name from the
   manifest and the host's actions (HS-01).
2. **Given** a widget whose content is wider and taller than its frame, or overflows toward the
   header in every direction, **When** it draws, **Then** nothing of it is drawn outside the body,
   and the header stays fully visible and clickable (HS-03, HS-04).
3. **Given** a widget whose strings hold HTML, a script, an image tag, bidirectional overrides,
   URLs, file paths and custom schemes, in every component, **When** it draws, **Then** each shows
   as its literal characters, nothing runs, loads or opens, and nothing moves outside the string
   (HS-07, HS-24).
4. **Given** a widget that uses every tone and variant to look like a host dialog, placed beside
   another widget, **When** it draws, **Then** the chrome stays uncovered and the other widget keeps
   responding to the pointer and the keyboard (HS-16).
5. **Given** a widget with a select of many long options, strings everywhere a tooltip could come
   from, and an invalid form submitted, **When** the person uses them, **Then** nothing is drawn
   outside the frame's body (HS-25).
6. **Given** a plugin whose manifest name holds markup, bidirectional overrides, the longest
   name the manifest accepts, or only characters the frame removes, **When** its frame draws,
   **Then** the name is drawn as text, stripped, cut at the frame's limit or replaced by the
   host's placeholder (HS-02). A name with control characters or over the manifest's limit is
   refused when the plugin loads, and the frame shows the host's "could not load" state.

---

### User Story 4 - A widget's controls do only what that widget declared (Priority: P2)

A button or a simple form in a widget does only what that widget declared it may do, run by the
host for that widget. It can never grant a permission, install a plugin, connect an account,
answer a host dialog, or act for another widget. The form has no password field and asks for no
credential.

**Why this priority**: Principles V and VII. Controls make widgets useful, and they are also how a
hostile widget would try to reach the host.

**Independent Test**: run the hostile widget whose buttons name host commands and other widgets'
actions; each is refused and nothing changes. Check that the form refuses a password field.

**Acceptance Scenarios**:

1. **Given** a widget with a button for one of the actions its manifest declares, **When** the
   person presses it, **Then** the host runs that action for that widget, keeps the data it
   returns, and draws the widget again with it.
2. **Given** a widget with a simple form, **When** the person submits it, **Then** the host runs
   the form's declared action with the form's values, under the bounds of FR-004.
3. **Given** a widget whose buttons and forms name host commands, other widgets' actions, or
   actions its manifest did not declare, **When** the person presses them, **Then** the host refuses each,
   and no permission, install, account or dialog answer happens (HS-05).
4. **Given** widget content that asks for a password or masked field, or a field typed, named or
   hinted as a credential, **When** it draws, **Then** no such field exists, and every input has
   autocomplete off (HS-06).

---

### User Story 5 - Status reads without color (Priority: P2)

When a widget shows something as ok, a warning or an error, the person can tell which without
seeing color, because the host draws the status icon beside the text.

**Why this priority**: WCAG 1.4.1, and feature 007's criterion HS-08.

**Independent Test**: draw text in each status tone and check that the host's icon is beside it.

**Acceptance Scenarios**:

1. **Given** text in the ok, warning or error tone, **When** it draws, **Then** the host's icon for
   that tone is beside it, from the widget map (HS-08).

---

### User Story 6 - The window's surface grows only by reviewed entries (Priority: P3)

The commands the window now calls to show widgets and run their actions enter the reviewed list
of feature 008, each with its reason, and every protection of the window still holds.

**Why this priority**: feature 008 made every widening of the window's surface a reviewed change.
This is the first one.

**Independent Test**: run the surface test and the window's probe in CI on the three systems.

**Acceptance Scenarios**:

1. **Given** the commands this feature adds, **When** the surface test runs, **Then** each is
   listed in the reviewed list with its reason and granted to the window, and nothing else is.
2. **Given** the release build with widgets showing, **When** the window's probe runs, **Then**
   every case of feature 008 still holds on the three systems.
3. **Given** a command that takes a widget, **When** the window passes one the host is not
   showing, or a malformed one, **Then** the host refuses it.

---

### Edge Cases

- Content of exactly the contract's 64 KiB, and one byte over.
- Content nested very deeply, or with very many nodes, long strings, wide or long tables, or a
  chart with very many points, no points, or values that are not numbers.
- A render that fails, traps, runs out of time or memory: the frame shows the error state and the
  window stays responsive.
- A plugin with no icon, or an icon that is not an image, too large, or an SVG holding script or
  an external reference.
- A plugin whose name is empty or only whitespace, control or override characters.
- A button pressed many times quickly, or while its widget is still drawing.
- A form submitted empty, with values over a length limit, or while invalid: no engine validation
  bubble appears.
- A narrow frame: tables and charts stay inside the body, scrolling inside it if they must.
- Forced colors and reduced motion: the frame stays recognisable, and status icons stay visible.
- The plugin is replaced while its widget is showing (a development rebuild): the frame draws the
  new content or the error state, never a mix.

## Requirements *(mandatory)*

### Functional Requirements

#### Widget content

- **FR-001**: Widget content MUST be a document of the shape the published schema defines, at most
  64 KiB as the contract states, carrying the format's version, and built only from these
  components: text, list, table, chart, button and simple form, and a stack that groups them.
  There MUST be no link component and no password or masked field.
- **FR-002**: Widget content MUST choose styling only by tone and variant names from the widget map
  of feature 007. It MUST have no field for a color, a font, a size, a class or a style; such
  fields are ignored, and the content draws as without them (HS-09).
- **FR-003**: A tone the host does not know MUST draw as the default tone and add one entry to the
  plugin's log naming it. A component type the host does not know MUST draw as a host placeholder
  and add one log entry, and the rest of the content MUST draw.
- **FR-004**: The host MUST bound widget content: nesting depth, number of components, string
  length, table rows and columns, chart series and points. The bounds MUST be published with the
  schema. Content over a bound is invalid.
- **FR-005**: When content is invalid (not JSON, rejected by the schema, over a bound) or render
  fails (error, trap, time or memory limit), the frame MUST show a host-drawn error state with a
  short reason, the plugin's log MUST get one entry naming the problem, and no other widget or
  part of the window MAY be affected. No text of the plugin's error message MAY be drawn as
  anything but text.
- **FR-006**: The schema MUST be published for authors beside the manifest schema, and the host's
  reading of widget content MUST be tested to agree with it. The author docs MUST describe each
  component, tone, variant and bound, with examples that CI validates against the schema.

#### The frame

- **FR-007**: The host MUST draw a frame around each widget: a header with the plugin's name, its
  icon and the host's actions, and a body for the widget content. Widget content MUST never be
  drawn in the header or change it (HS-01).
- **FR-008**: The frame's name and icon MUST come from the plugin's manifest, never from widget
  content. The manifest gains optional `name` and `icon` fields; the icon is a small PNG under a
  published size limit, and any other image, an SVG included, is refused when the plugin loads.
  Without a name, the host shows the last part of the plugin's id; without
  an icon, an icon the host generates. The name MUST be drawn as text, with control and bidirectional override characters removed and
  a length limit, and a host placeholder when it is empty. An icon MUST be drawn as an image in a
  fixed box, never as markup, running and fetching nothing (HS-02). The host MUST decode the icon
  itself and hand the page only pixels to draw, so the window's content policy and
  its probe stay exactly as feature 008 left them: no new image source, no case removed.
- **FR-009**: The frame's body MUST contain and clip everything in it, so no part of widget
  content is drawn outside it, and the header MUST sit outside the body, so content can neither
  hide it nor take its clicks (HS-03, HS-04).
- **FR-010**: Widget content MUST NOT produce a layer above the chrome or the other frames, a scrim,
  or an inert window (HS-16).

#### Strings and what they can open

- **FR-011**: Every string in widget content MUST be drawn as text, never parsed as markup, with
  bidirectional overrides isolated to the string (HS-24).
- **FR-012**: No string from a plugin MAY reach anything that opens a link, a file or another app
  (HS-07).
- **FR-013**: No component MAY open a layer outside its frame's body: no popover, select list or
  tooltip outside it, and no engine UI such as a `title` tooltip, a validation bubble or a date
  picker. A select lists its options inside the frame (HS-25).

#### Controls

- **FR-014**: A button or form MUST trigger only an action its own widget's manifest declared, run
  by the host as that widget. The host MUST refuse any other name, including host commands and other widgets'
  actions, and MUST NOT grant a permission, install anything, connect an account or answer a host
  dialog because of a control (HS-05).
- **FR-015**: The simple form MUST have no password or masked field, every input MUST have
  autocomplete off, and no input MAY carry a credential type, name or hint (HS-06).
- **FR-025**: Before a form's values reach the plugin, the host MUST refuse them if any value
  contains a secret the host holds, in any encoding its credential checks already recognise, and
  log the refusal without the value (Principle IV, as amended).
- **FR-016**: Text in the ok, warning and error tones MUST carry the host's icon for that tone,
  from the widget map (HS-08).

#### Actions in the contract

- **FR-022**: Contract v1 MUST gain a minor version, 1.4.0, that only adds an `actor` export,
  following the contract's own rule: 1.3.0 frozen first, and the only-adds tests extended. Since
  an existing world cannot change, the export arrives in new worlds beside the six of 1.3.0, and
  what an action and a render receive is defined in new items, frozen comments staying as they
  are. The
  host runs a declared action with its name and the form's values, under the same limits as an
  update. An action may write the widget's store and call its connectors' operations that only
  read, each only as granted; an operation that changes data MUST stay refused, as it is today.
  An action returns the widget's data, which the host keeps **in memory, while the app runs**,
  and gives to the next render. Data an action returns is never written to disk: state that must
  survive a restart needs the `storage` permission and its approved quota. A plugin built for an
  earlier minor keeps running and has no actions.
- **FR-023**: The manifest MUST declare each action's name, and the host MUST refuse, before
  calling the plugin, any action name the manifest does not declare. The manifest schema and the
  author reference MUST describe `name`, `icon` and the declared actions, and the host's manifest
  parser MUST stay tested to agree with the schema.

#### Running widgets in the app

- **FR-017**: The app MUST run the bundled example widget with the runtime's existing limits on
  CPU, memory and time, without freezing the window while a widget renders.
- **FR-018**: The bundled example MUST need no permission that only a host dialog can grant
  (Principle VII); a widget whose required permissions are not granted MUST NOT run. Granting in
  the app arrives with the installation dialog.
- **FR-024**: The app MUST keep its own grant record, kept data and vault, apart from the
  development ones of the command-line tool, and MUST never read the development grant record or
  the development vault. Its grant record holds no permission until a host dialog grants one; the
  only entries before then are the start-up approvals of built-in plugins that declare none
  (Principle VII, v2.6.3). The app MUST
  run the runtime with development mode off, so no unverified local connector runs. Reloading a
  rebuilt bundled plugin reloads its code and never changes a grant.
- **FR-019**: Every host command the window gains MUST be added to the reviewed list of feature
  008 with its reason and granted to the window, and MUST refuse a widget the host is not showing
  or a malformed argument. The window's protections of feature 008 MUST still hold, with its probe
  green on the three systems.

#### Tests and records

- **FR-020**: Each criterion this feature runs (HS-01 to HS-09, HS-16, HS-24, HS-25) MUST have the
  test or hostile case it names, run in CI on Windows, macOS and Linux, and failing when the
  protection is removed (Principle VIII). So MUST every other refusal and limit of this feature:
  the content bounds (FR-004), the manifest's name and icon bounds (FR-008), undeclared and
  foreign action names (FR-014, FR-023), form values carrying a held secret (FR-025), an action
  calling an operation that changes data (FR-022), commands given a widget the host is not showing or a
  malformed argument (FR-019), an action's limits and grants and the absence of actions for an
  earlier minor (FR-022), the app's separate records and development mode off (FR-024), and
  repeated presses while a widget is drawing.
- **FR-021**: The threat model's rows MUST cite each of those tests: B4 for the frame and the
  library, B1 for the `actor` export and the manifest fields, B7 for the window's new commands.
  Its boundary text MUST be revised where actions change it (B1 says stores are written all or
  nothing in `update`; B2 says connectors are called only in `update`). Decision 0001 stays as it
  is, since its trigger is installing a plugin the app did not build; the plan MUST record that
  review, because the app now loads and compiles the bundled plugins in its own process.

### Key Entities

- **Widget content**: the document a render returns: the format's version and a tree of
  components, within the published bounds.
- **Component**: text, list, table, chart, button or simple form, each with its own fields, a tone
  or variant where it has styling, and nothing else.
- **Declared action**: a name the manifest declares, which a control in that widget's content may
  trigger; the host runs it through the plugin's `actor` export and keeps the data it returns.
- **Manifest name and icon**: optional manifest fields the frame shows, bounded and drawn by the
  host.
- **Frame**: the host-drawn header (name, icon, host actions) and body (widget content) of one
  widget.
- **Content error**: why a widget's content could not be drawn, shown in the frame and logged.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On opening the app, the bundled example widget appears in its frame within 1 second
  on the reference machine, on Windows, macOS and Linux.
- **SC-002**: Every criterion this feature runs has a check that passes in CI on the three
  systems, and each hostile case fails when its protection is removed.
- **SC-003**: Every example in the author docs validates against the published schema, checked in
  CI, so an author can write valid content from the docs alone.
- **SC-004**: All invalid-content fixtures (each kind of FR-005) show the error state in their
  frame, with one log entry each, while a valid widget beside them keeps drawing and responding.
- **SC-005**: The window's probe of feature 008 holds every case on the three systems with
  widgets showing, and the reviewed list holds exactly the commands this feature adds.
- **SC-006**: 100% of status-tone text draws with its icon.
- **SC-007**: Pressing the bundled example's action button shows its new state within 1 second,
  and every undeclared action name in the hostile fixture is refused without reaching the plugin.

## Assumptions

- **No grid yet.** Widgets appear one under another in the main view; the grid feature replaces
  that layout with adding, moving and resizing.
- **Only plugins built with the app run.** Installing other plugins, and the dialogs that grant
  permissions in the app, are the installation items of Phase 3. Until then the bundled example
  needs no granted permission (FR-018).
- **The bundled example is a showcase.** It shows every component, tone and variant, so it serves
  the person, the reviewer and authors as the reference widget.
- **Render runs as today.** The runtime's render, its limits and its log are unchanged; this
  feature adds the app's side, the content format and the `actor` export. Scheduled data
  updates through connectors in the app arrive with the official widgets.
- **The bundled example's actions need no permission.** It keeps its state in the data its
  actions return, which the host keeps in memory, so it shows a working action without a grant;
  its state starts over when the app restarts. A widget that needs durable state declares
  `storage`.
- **The look follows feature 007.** Components draw with the shared roles of `DESIGN.md`; the
  frame header uses its host-only roles.
- **Charts are simple.** A line or bar chart of a few series, drawn by the host, with a text
  alternative; richer charts can come later as new components.
- **The demo page stays** as the visual system's review page until the dashboard replaces it.
