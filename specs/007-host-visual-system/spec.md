# Feature Specification: Host Visual System and the Design of the Host Frame and Dialogs

**Feature Branch**: `007-host-visual-system`

**Created**: 2026-11-10

**Status**: Draft

**Input**: User description: "Primeira feature da Fase 3 do guia: o sistema visual do host em
DESIGN.md e o design da moldura e dos diálogos do host. Junta os dois primeiros itens da Fase 3.
(1) Sistema visual em DESIGN.md, no formato aberto do Google (ainda alpha; fixar a versão da
ferramenta): tokens de cor para os temas claro e escuro, tipografia, espaçamento, formas e
componentes da UI declarativa, cada um com a justificativa. O @theme do Tailwind é gerado a partir
dele, e o lint de contraste (WCAG AA) roda no CI. Como o build usa o DESIGN.md como código, ele
precisa sair do filtro de documentação do ci.yml (que hoje pula todo .md), senão uma mudança só
nele não roda o lint nem os testes. O ponto de partida é o tema Ardósia + Musgo que já está em
app/src/index.css (PR #16): neutros frios (matiz 250) e destaque cobalto, Bricolage Grotesque nos
títulos, Figtree no texto, IBM Plex Mono para ids e domínios, raio 0.625rem. (2) Design da
moldura e dos diálogos do host: o que só o host desenha, tokens reservados ao host que a UI dos
plugins não pode usar, acessibilidade dos diálogos (teclado e leitor de tela) e
critérios testáveis de que um widget não consegue imitar o host. A stack continua Tauri 2 + React
na webview."

## Clarifications

### Session 2026-11-10

- Q: How does the person tell a real host dialog from widget content that imitates one? → A: By
  structure that widget content cannot produce:
  - a host dialog covers the app's chrome and every frame, where no widget content can draw;
  - while a dialog is open, the rest of the window is inert;
  - the dialog holds keyboard focus and is all that assistive technology sees.

  Trust decisions happen only in host dialogs (Principle VII), so a picture of a dialog inside a
  frame can take no decision. The threat model's plugin UI boundary (B4) names this form.
- Q: Is the Ardósia cobalt accent reserved for the host? → A: No, it is shared. Widget content
  can use the accent tone. Host-only tokens are those of the host's surfaces: chrome, frame,
  dialog, scrim, elevation above a frame and the host's focus style.

## User Scenarios & Testing *(mandatory)*

The people in these stories are:

- the **maintainer**, who owns the host and its look;
- a **contributor**, who changes the visual system;
- the **person** who uses the dashboard;
- a **plugin author**, whose widget content the host draws.

Terms used throughout:

- **The visual system** is `DESIGN.md` and everything generated from it.
- **Widget content** is the declarative UI a plugin returns and the host draws: text, list, table,
  chart, button and simple form, with no password field (Principle V).
- **Host surfaces** are what only the host draws:
  - the app's chrome;
  - each widget's frame, with the plugin's name and icon;
  - host dialogs, with their scrim;
  - anything raised above a frame;
  - the host's focus style.
- **Host-only tokens** are the tokens those surfaces use. Widget content can never resolve to one.

### User Story 1 - One source for the whole visual system (Priority: P1)

A contributor wants to change the accent color, a spacing step or the corner radius. They edit
`DESIGN.md`, run one command, and the app's theme follows in both light and dark. They never
edit a token value anywhere else. If they edit the generated theme by hand, or forget to
regenerate it, CI fails and names the difference.

**Why this priority**: every later Phase 3 surface draws from these tokens: the frame, the
dialogs, the dashboard grid and the declarative UI. A second source would let the host and widget
content drift apart, and the split between host-only and shared tokens would stop meaning
anything.

**Independent Test**: change one color role, one spacing step and the base radius in `DESIGN.md`
only, run the generator, build the app, and confirm that all three changes show and that CI
passes. Then hand-edit one generated value and confirm that CI fails naming it.

**Acceptance Scenarios**:

1. **Given** `DESIGN.md` with the starting theme, **When** the generator runs, **Then** the app's
   theme for light and dark matches the values `DESIGN.md` declares, and no token value is
   written by hand elsewhere in the app.
2. **Given** a generated theme that differs from what `DESIGN.md` produces, **When** CI runs,
   **Then** it fails and names each token that differs.
3. **Given** a `DESIGN.md` that breaks the format's rules, **When** CI runs the format's own
   checker at the pinned version, **Then** it fails with that checker's message.
4. **Given** a change that touches only `DESIGN.md`, **When** CI runs, **Then** every job that a
   code change runs also runs, on Windows, macOS and Linux.

---

### User Story 2 - Text stays readable in both themes (Priority: P1)

The person uses the dashboard in light or dark mode and can read everything the host draws. That
includes muted text, a disabled control's label, an input's border and the focus ring. A
contributor who picks a color that fails WCAG AA learns it from CI before the change merges.

**Why this priority**: the starting theme was picked by eye. A muted gray or a border that fails
contrast in one theme would make the host hard to use for people with low vision. Once widgets
and dialogs build on it, that failure would spread to every one of them.

**Independent Test**: run the contrast check on the starting theme and record each pairing's
ratio in both themes. Then lower one pairing below its threshold and confirm that CI fails naming
the pairing, the theme, the ratio found and the ratio required.

**Acceptance Scenarios**:

1. **Given** `DESIGN.md`, **When** the contrast check runs, **Then** it checks every pairing of
   a foreground role on a background role that `DESIGN.md` declares, in light and in dark.
2. **Given** a text pairing below 4.5:1, or below 3:1 for text that `DESIGN.md` marks as large,
   **When** the check runs, **Then** it fails and names the pairing, the theme and both ratios.
3. **Given** an input's border, the focus indicator or a chart mark below 3:1 against the surface
   it sits on, **When** the check runs, **Then** it fails the same way (WCAG 1.4.11).
4. **Given** a color role that appears in no declared pairing, **When** the check runs, **Then**
   it fails, unless `DESIGN.md` marks the role as decorative and says why.

---

### User Story 3 - Widget content cannot use the host's surfaces (Priority: P1)

A plugin author builds a widget from the declarative UI. They can ask for a tone, such as
default, muted, accent, ok, warning or error, and a component variant. They cannot name a
color, a font, a size, a class or a token. Whatever they send, nothing in their widget resolves to
a host-only token. So no widget can paint the frame's header, a dialog's surface, the scrim or the
host's focus style.

**Why this priority**: Principle V keeps plugin UI inside the frame and away from fake dialogs.
If widget content could reach the host's own surface colors or elevation, it could draw
something that passes for the host inside its frame. This split is what the dialog and frame
criteria in User Story 4 rely on.

**Independent Test**: list every token that widget content can resolve to, through every tone
and every variant of every component. Confirm that none is host-only. Then map one tone to a
host-only token in a test build and confirm that the check fails naming the tone and the token.

**Acceptance Scenarios**:

1. **Given** the declarative UI's tones and variants, **When** the check lists what they resolve
   to, **Then** no host-only token is in that list.
2. **Given** widget content that names a tone the host does not know, **When** the host draws it,
   **Then** it uses the default tone and records the unknown name in the plugin's log.
3. **Given** widget content that carries a color, font, size, class or style value, **When** the
   host draws it, **Then** the value has no effect, because the declarative UI has no field that
   takes one.
4. **Given** a status shown with the ok, warning or error tone, **When** it is drawn, **Then**
   the host also shows an icon or text that names the status, so color is never the only sign
   (WCAG 1.4.1).

---

### User Story 4 - The frame and the dialogs are designed to be checked (Priority: P2)

The maintainer gets a written design of the app's chrome, the widget frame and the host dialogs.
It covers the frame's header with the plugin's name and icon, and the form every host dialog
shares. The design is written as numbered criteria that later Phase 3 features must
pass. Each criterion says how it is checked: an automated test, a hostile case, or a review step.
Each also names the later feature that runs it.

**Why this priority**: the frame, the install dialog and the other dialogs are separate Phase 3
items that come later. They need one design to build against and one list of checks to pass,
written before either exists, so that "a widget cannot imitate the host" is a test and not a
judgment.

**Independent Test**: for every criterion, a reviewer can name the test or review step that
checks it and the feature that will run it. Every Phase 3 row of the threat model about host
imitation, dialogs or the frame maps to at least one criterion.

**Acceptance Scenarios**:

1. **Given** the design, **When** a reviewer reads the frame's section, **Then** it states:
   - that the plugin's name and icon come from its manifest, drawn by the host as escaped text,
     with a length limit;
   - that the frame's header and its actions belong to the host;
   - that widget content is drawn only inside the frame's body and cannot cover the header or
     reach outside the frame.
2. **Given** the design, **When** a reviewer reads the dialogs' section, **Then** it states the
   form every host dialog shares:
   - drawn above the whole window, the chrome included, with the rest of the window inert
     behind a scrim;
   - which plugin or connector the decision is about, named by the host;
   - the safe choice as the default.
3. **Given** the criteria, **When** a later feature that draws the frame or a dialog is reviewed,
   **Then** it must pass every criterion that names it, or the criterion must be changed first.

---

### User Story 5 - The person recognizes a real host dialog (Priority: P2)

A real host dialog (install, permissions, connecting an account, confirming an action) always
covers the app's chrome and every widget. While it is open, nothing else in the window responds.
Widget content is confined to its frame's body, so a picture of a dialog inside a widget leaves
the chrome and the other widgets visible and working. That tells the person it is not the host.
And since trust decisions happen only in host dialogs, the picture's buttons can only trigger
that widget's own declared actions.

**Why this priority**: the token split makes imitation hard, but a widget's frame body can still
hold a convincing picture of a dialog. The person needs a sign that no plugin can produce, and
that sign is the dialog's structure. Nothing has to be learned or remembered.

**Independent Test**: from the design, confirm three things. Every property that marks a host
dialog is one that widget content cannot produce under the frame criteria. Each property has a
check in a later feature. The properties still hold with a screen reader and in forced-colors
mode.

**Acceptance Scenarios**:

1. **Given** any host dialog in the design, **When** it is drawn, **Then** it covers the chrome
   and every frame, and the rest of the window is inert until it closes.
2. **Given** widget content shaped like a dialog, **When** the host draws it, **Then** it stays
   inside its frame's body, the chrome stays uncovered, and the rest of the window keeps
   responding.
3. **Given** a button inside widget content, **When** the person activates it, **Then** it can
   trigger only that widget's own declared action, never a grant, an install or an account
   connection.

---

### User Story 6 - Host dialogs work with the keyboard and a screen reader (Priority: P3)

A person who uses only the keyboard, or a screen reader, can read and answer every host dialog.
When a dialog opens, focus moves into it and stays there until it closes. Escape always takes the
safe choice, never the risky one. Focus then returns to where it was. The screen reader
announces the dialog's name and description, and while the dialog is open nothing behind it is
reachable.

**Why this priority**: trust decisions happen only in host dialogs (Principle VII). A dialog that
some people cannot operate shuts them out of installing plugins and connecting accounts. If
assistive technology could still reach a widget behind a dialog, those people would lose the
defense in User Story 5.

**Independent Test**: for every dialog criterion about the keyboard or a screen reader, a
reviewer can name the check that a later feature runs. Each check uses the dialog with only a
keyboard, and with a screen reader on each of the three systems.

**Acceptance Scenarios**:

1. **Given** a host dialog opens, **When** the person presses Tab and Shift+Tab, **Then** focus
   moves only among the dialog's controls.
2. **Given** a host dialog, **When** the person presses Escape, **Then** the dialog takes its safe
   choice (deny or cancel) and closes. Escape never approves.
3. **Given** a host dialog closes, **When** focus returns, **Then** it lands on the control that
   opened the dialog, or on the window if that control is gone.
4. **Given** a screen reader, **When** a host dialog opens, **Then** the reader announces it as a
   dialog, with its name and description, and cannot reach anything behind it.

---

### Edge Cases

- **A pairing in the starting theme fails AA.** For example, the light theme's input border is
  close to white and is likely below 3:1. `DESIGN.md` then changes that value, keeping the hue,
  and records the change and the reason. The starting theme is a direction, not a fixed set of
  values.
- **The `DESIGN.md` format changes.** The format is alpha. Its checker is pinned to one version,
  and moving to a new one is a deliberate change that this feature's checks must pass.
- **Forced colors or high contrast mode** (for example on Windows). The system replaces the
  theme's colors. The dialog's form must still read, so it may not depend on color alone.
- **A font fails to load.** Every family in `DESIGN.md` has a fallback stack, and the fonts ship
  with the app, so this happens only if the bundle is broken.
- **A plugin's name or icon is long, empty or shaped to look like host text.** It is drawn as
  escaped text within the frame's length limit, and the frame shows a host-drawn placeholder
  when the name is empty.
- **Reduced motion.** Dialog and frame transitions follow the system's reduced-motion setting.
- **A role used only in one theme.** It still needs a value and its pairings in both, or the
  check fails.
- **A dialog's content runs longer than the window.** The dialog scrolls inside itself. The
  name of who is asking and the safe choice stay reachable without leaving the dialog.
- **A widget draws a convincing dialog inside its frame.** This cannot be prevented, but it
  cannot take a trust decision either. Whether the host writes the effect and destination on
  every control of a declared action that changes data is decided in the Phase 3 feature for
  declared actions.

## Requirements *(mandatory)*

### Functional Requirements

#### The visual system

- **FR-001**: The repository MUST hold the visual system in one `DESIGN.md` at its root, in
  Google's open `DESIGN.md` format. CI MUST check the file with the format's own checker, pinned
  to one version.
- **FR-002**: `DESIGN.md` MUST declare each of the following for light and dark, each with its
  rationale:
  - the color roles;
  - the status tones ok, warning and error;
  - the typography: families, a type scale and weights;
  - the spacing scale;
  - the radius scale;
  - the elevation;
  - the components of the declarative UI: text, list, table, chart, button and simple form.
- **FR-003**: The starting values MUST be the Ardósia + Musgo theme as merged in PR #16:
  - cool neutrals around hue 250 and a cobalt accent;
  - Bricolage Grotesque for headings, Figtree for text and IBM Plex Mono for ids and domains;
  - a base radius of 0.625rem.

  A value MAY change only where a check of this feature fails, and `DESIGN.md` MUST record each
  change and its reason.
- **FR-004**: One command MUST generate the app's theme from `DESIGN.md`: the Tailwind `@theme`
  and the light and dark token values. After this feature, no token value is written by hand in
  the app.
- **FR-005**: CI MUST fail when the generated theme differs from what `DESIGN.md` produces, and
  MUST name each token that differs.
- **FR-006**: CI's filter for documentation-only changes MUST treat `DESIGN.md` as code. A change
  to it alone runs every job on Windows, macOS and Linux, as the constitution's rule for
  Markdown used as code requires.
- **FR-007**: The fonts MUST ship inside the app, with nothing loaded from the network, so the
  restrictive CSP of the coming Tauri shell needs no font origin. Each font's license MUST allow
  shipping it in an Apache-2.0 app, and is recorded.
- **FR-008**: The generator MUST also produce a static preview page from `DESIGN.md`, with no
  script, for review. It shows both themes, the scales, the declarative UI's components, the
  status tones with their icons, and sketches of the frame and a dialog. CI MUST publish it with
  every build; it is not committed.

#### Contrast

- **FR-009**: `DESIGN.md` MUST declare every pairing of a foreground role on a background role
  that the host draws. Each pairing has a kind with a minimum ratio:
  - normal text: 4.5:1;
  - large text: 3:1;
  - component boundary or focus indicator: 3:1;
  - chart mark: 3:1.
- **FR-010**: CI MUST check every declared pairing in both themes. A failure MUST name the
  pairing, the theme, the ratio found and the ratio required.
- **FR-011**: Every color role MUST appear in at least one declared pairing, unless `DESIGN.md`
  marks it as decorative and says why.
- **FR-012**: The host MUST never show a status by color alone. Every use of the ok, warning or
  error tone MUST carry an icon or text, drawn by the host, that names the status.

#### Host-only tokens and widget content

- **FR-013**: `DESIGN.md` MUST mark every token as host-only or shared. Host-only MUST include
  at least the tokens of these surfaces:
  - the chrome;
  - the frame's border and header;
  - the dialog's surface;
  - the scrim;
  - any elevation above a frame;
  - the host's focus style.

  The accent color is shared, so widget content can use the accent tone.
- **FR-014**: Widget content MUST select styling only through a closed set of tones and of
  component variants. It MUST NOT carry a color, font, size, class or style value. A tone the
  host does not know MUST resolve to the default tone, and the host MUST record the name in the
  plugin's log.
- **FR-015**: An automated check MUST list every token that each tone and each variant of each
  component resolves to. The build MUST fail if any of those tokens is host-only, naming the tone
  or variant and the token.

#### Design of the frame and the dialogs

- **FR-016**: The feature MUST write the design of the app's chrome, the widget frame and the
  host dialogs as numbered criteria. Each criterion MUST state how it is checked (an automated
  test, a hostile case or a review step) and which later Phase 3 feature runs it.
- **FR-017**: The criteria MUST cover at least the following:
  - widget content is drawn only inside its frame's body and cannot cover the frame's header or
    reach outside the frame;
  - the plugin's name and icon come from its manifest and are drawn by the host as escaped text,
    with a length limit;
  - widget content cannot produce the form of a host dialog: a layer above the chrome and the
    frames, a scrim, or an inert window;
  - every host dialog covers the chrome and every frame, and names the plugin or connector the
    decision is about;
  - a control inside widget content can trigger only that widget's own declared actions, never
    a trust decision;
  - the safe choice is every dialog's default.
- **FR-018**: The criteria MUST state the keyboard and screen-reader behavior of host dialogs:
  - focus moves into the dialog and is kept inside it;
  - Escape takes the safe choice;
  - focus returns to the control that opened the dialog;
  - the dialog is announced with its name and its description;
  - everything behind the dialog is hidden from assistive technology while it is open.
- **FR-019**: A host dialog MUST be recognizable by
  properties that widget content cannot produce: covering the chrome and every frame, making the
  rest of the window inert, holding keyboard focus, and being all that assistive technology
  sees. None of these properties MAY depend on color alone, so they hold in forced-colors mode.
- **FR-020**: The threat model MUST cover this design:
  - The plugin UI boundary (B4) names the dialog form in FR-019.
  - It gains a Phase 3 row for widget content that imitates the host's styling. The mitigation
    is the host-only tokens (FR-013–FR-015) and the dialog form (FR-017, FR-019).
  - The existing Phase 3 rows on fake logins and on widgets opening dialogs point to the criteria
    that cover them.
- **FR-021**: This feature adds no hostile case to the security suite. The check in FR-015 is an
  ordinary test. Hostile cases that need a drawn frame or dialog arrive with the features that
  draw them, and those features must pass these criteria.

### Key Entities

- **Token**: a named value with a group (color, type, space, radius or elevation), a value per
  theme, a host-only or shared mark, and a rationale.
- **Color pairing**: a foreground role on a background role, with a kind (normal text, large
  text, component boundary, focus indicator or chart mark) and the minimum ratio that kind
  requires.
- **Tone**: a name widget content can ask for, such as default, muted, accent, ok, warning or
  error. Each tone maps to shared tokens only.
- **Component variant**: a named look of a declarative UI component, mapping to shared tokens
  only.
- **Host surface**: something only the host draws (chrome, frame, dialog, scrim or focus style),
  using host-only tokens.
- **Criterion**: a numbered, checkable statement about the chrome, the frame or a dialog. It
  records how it is checked and which later feature checks it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A contributor changes one color role, one spacing step and the base radius by
  editing only `DESIGN.md` and running one command. They see the change in the app in under five
  minutes.
- **SC-002**: The contrast check covers 100% of the declared pairings in both themes. It reports
  zero failures when the feature merges, and a deliberately broken pairing fails CI by name.
- **SC-003**: Zero host-only tokens are reachable from widget content. A seeded violation fails
  the build by name.
- **SC-004**: A change to `DESIGN.md` alone runs every CI job on all three systems.
- **SC-005**: 100% of the criteria name their check and the later feature that runs it. Every
  Phase 3 threat-model row about host imitation, dialogs or the frame maps to at least one
  criterion.
- **SC-006**: Apart from the contrast fixes `DESIGN.md` records, the app built from the generated
  theme looks the same as after PR #16, in light and dark.
- **SC-007**: Every CI run that builds publishes the preview page, and the maintainer can review
  the visual system in a browser without building the app.

## Assumptions

- **No appearance picker.** The person picks only light or dark, and the host follows the system
  setting by default. Picking an accent, a gray family or a radius is out of scope.
- **No window work.** This feature draws no frame or dialog in the app and changes no Tauri
  setting. The Tauri shell with minimal capabilities and a restrictive CSP, the dashboard grid,
  the frame and the dialogs are later Phase 3 features. Each of them must pass these criteria.
- **The demo app stays.** The React demo in `app/` keeps its shadcn/ui components. They read the
  generated theme instead of hand-written values. Building the declarative UI's components is
  the later "Biblioteca de UI declarativa" item. This feature defines their tokens, tones and
  variants only.
- **The format is alpha.** Google's `DESIGN.md` format may change. Pinning its checker keeps the
  build stable, and an upgrade is its own change.
- **Fonts and licenses.** Bricolage Grotesque, Figtree and IBM Plex Mono are under the SIL Open
  Font License 1.1, which allows shipping them in an Apache-2.0 app.
- **Constitution.** No amendment is expected. Principle V and the rule on Markdown used as code
  already cover this feature, and the stack stays Tauri 2 + React as fixed in v2.4.1.
