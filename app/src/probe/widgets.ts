// The widget cases (specs/009-declarative-widget-ui/research.md, R14): the frames of the probe's
// hostile widgets, drawn in the real page, beside the showcase.
// Part of the security core (constitution, Principle IX).

import { actWidget, listWidgets, renderWidget, type FrameInfo, type Rendered } from "@/widgets/api";

import type { Case, Context, Verdict } from "./case";

/** How long a frame gets to reach the state a case expects. */
const DRAW_MS = 5000;
const POLL_MS = 50;

/** The frames the page draws, by the name the host drew, listed once. */
let listed: Promise<Map<string, FrameInfo>> | undefined;
function frames(): Promise<Map<string, FrameInfo>> {
  listed ??= listWidgets().then((all) => new Map(all.map((frame) => [frame.name, frame])));
  return listed;
}

async function handleOf(name: string): Promise<string> {
  const frame = (await frames()).get(name);
  if (!frame) throw new Error(`no frame named ${name}`);
  return frame.handle;
}

function section(handle: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(`section[data-widget="${handle}"]`);
}

/** The frame once it shows `view`, or null when it did not within `DRAW_MS`. */
async function frameIn(context: Context, handle: string, view: string): Promise<HTMLElement | null> {
  for (let waited = 0; waited <= DRAW_MS; waited += POLL_MS) {
    const found = section(handle);
    if (found?.dataset.view === view) return found;
    await context.wait(POLL_MS);
  }
  return null;
}

/** What is wrong with a frame that should show `view` and, in its body, the failure's `kind`. */
async function shows(context: Context, name: string, view: string, kind: string): Promise<string | null> {
  const handle = await handleOf(name);
  const frame = await frameIn(context, handle, view);
  if (!frame) return `${name}: ${section(handle)?.dataset.view ?? "no frame"}, not ${view}`;
  const body = frame.querySelector("[data-frame-body]")?.textContent ?? "";
  return body.includes(kind) ? null : `${name}: ${view}, but its text does not name ${kind}`;
}

function failureKind(rendered: Rendered): string {
  if ("error" in rendered) return rendered.error.kind;
  return "content" in rendered ? "content" : `refused ${rendered.refused}`;
}

export const widgetCases: Case[] = [
  {
    // SC-001: the showcase is drawn within a second of the page's load
    id: "showcase-on-time",
    async run(context): Promise<Verdict> {
      const name = `widget-drawn-${await handleOf("Vitrine")}`;
      for (let waited = 0; waited <= DRAW_MS; waited += POLL_MS) {
        const [mark] = performance.getEntriesByName(name, "mark");
        if (mark) {
          const ms = Math.round(mark.startTime);
          return { held: ms <= 1000, detail: `the showcase was drawn ${ms} ms after the page began loading` };
        }
        await context.wait(POLL_MS);
      }
      return { held: false, detail: "the showcase was not drawn" };
    },
  },
  {
    // FR-005: content over a bound, a wrong type, a render that traps and a refused manifest each
    // show the host's state, naming the failure; the showcase beside them still draws
    id: "content-error-shown",
    async run(context): Promise<Verdict> {
      const problems: string[] = [];
      const note = (problem: string | null) => problem && problems.push(problem);
      note(await shows(context, "ui-invalid", "error", "over-bound"));
      note(await shows(context, "ui-crash", "error", "render-failed"));
      note(await shows(context, "ui-badmanifest", "not-loaded", "not-loaded"));

      // The action makes the next render return a number where a text goes; the frame's reload
      // button draws it
      const invalid = await handleOf("ui-invalid");
      const acted = await actWidget(invalid, "wrong-type", {});
      if (failureKind(acted) !== "bad-shape") problems.push(`wrong-type answered ${failureKind(acted)}`);
      section(invalid)?.querySelector<HTMLButtonElement>("header button")?.click();
      await context.wait(POLL_MS);
      note(await shows(context, "ui-invalid", "error", "bad-shape"));

      const showcase = await frameIn(context, await handleOf("Vitrine"), "content");
      if (!showcase) problems.push("the showcase is not drawn beside them");
      return problems.length === 0
        ? { held: true, detail: "over-bound, bad-shape, render-failed and not-loaded each shown; the showcase drawn" }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // FR-017: while one widget's action loops until the time limit, the window and the other
    // widgets keep answering
    id: "window-responsive",
    async run(context): Promise<Verdict> {
      const showcase = await handleOf("Vitrine");
      const spin = actWidget(await handleOf("ui-invalid"), "spin", {});
      await context.wait(300);
      const problems: string[] = [];

      section(showcase)?.scrollIntoView({ block: "start" });
      await context.wait(100);
      const reload = section(showcase)?.querySelector<HTMLElement>("header button");
      const box = reload?.getBoundingClientRect();
      const hit = box && document.elementFromPoint(box.x + box.width / 2, box.y + box.height / 2);
      if (!reload || !hit || !reload.contains(hit)) problems.push("the showcase's reload button is not hit");

      const began = performance.now();
      const rendered = await renderWidget(showcase);
      const answered = Math.round(performance.now() - began);
      if (!("content" in rendered)) problems.push(`the showcase answered ${failureKind(rendered)}`);
      if (answered > 1000) problems.push(`the showcase answered after ${answered} ms`);

      const spun = await spin;
      if (failureKind(spun) !== "action-failed") problems.push(`the spin ended as ${failureKind(spun)}`);
      return problems.length === 0
        ? { held: true, detail: `during the spin the showcase answered in ${answered} ms; the spin ended as action-failed` }
        : { held: false, detail: problems.join("; ") };
    },
  },
];

/** The strings `ui-strings` puts in every component (plugins/hostile/ui-strings/src/lib.rs). */
const HOSTILE = [
  "<script>window.__probeInjected = 1</script>",
  '<img src=x onerror="window.__probeInjected = 1">',
  '<b>negrito</b> <a href="https://probe.invalid/">link</a>',
  "\u202Egnp.exe",
  "https://probe.invalid/caminho?q=1",
  "javascript:window.__probeInjected = 1",
  "file:///etc/passwd",
  "C:\\Windows\\System32\\drivers\\etc\\hosts",
  "/home/user/.ssh/id_ed25519",
  "dashboard://settings",
  "tauri://localhost/index.html",
  "mailto:someone@probe.invalid",
];
const SENTINEL = "SENTINELA";

/** The frame whose host-drawn name passes `test`, in view, with its header and body. */
async function frameWhere(
  context: Context,
  test: (name: string) => boolean,
): Promise<{ frame: FrameInfo; section: HTMLElement; header: HTMLElement; body: HTMLElement }> {
  const frame = [...(await frames()).values()].find((each) => test(each.name));
  if (!frame) throw new Error("no such frame");
  const found = await frameIn(context, frame.handle, "content");
  if (!found) throw new Error(`${frame.name}: not drawn`);
  found.scrollIntoView({ block: "start" });
  await context.wait(100);
  const header = found.querySelector<HTMLElement>("header");
  const body = found.querySelector<HTMLElement>("[data-frame-body]");
  if (!header || !body) throw new Error(`${frame.name}: no header or body`);
  return { frame, section: found, header, body };
}

type Box = { left: number; top: number; right: number; bottom: number };

function boxOf(element: Element): Box {
  const { left, top, right, bottom } = element.getBoundingClientRect();
  return { left, top, right, bottom };
}

/** Whether `element` clips what it holds: any overflow but visible, or paint containment. */
function clips(element: Element): boolean {
  const style = getComputedStyle(element);
  return (
    style.overflowX !== "visible" ||
    style.overflowY !== "visible" ||
    /paint|strict|content/.test(style.contain)
  );
}

/** What of `element` can paint: its box, cut by every ancestor that clips. */
function visibleBox(element: Element): Box | null {
  let box = boxOf(element);
  for (let parent = element.parentElement; parent; parent = parent.parentElement) {
    if (!clips(parent)) continue;
    const cut = boxOf(parent);
    box = {
      left: Math.max(box.left, cut.left),
      top: Math.max(box.top, cut.top),
      right: Math.min(box.right, cut.right),
      bottom: Math.min(box.bottom, cut.bottom),
    };
  }
  return box.right > box.left && box.bottom > box.top ? box : null;
}

/** The content elements whose painted part lies outside `body`, named. */
function outside(body: HTMLElement): string[] {
  const limit = boxOf(body);
  const found: string[] = [];
  for (const element of body.querySelectorAll("*")) {
    const box = visibleBox(element);
    if (!box) continue;
    if (
      box.left < limit.left - 1 ||
      box.top < limit.top - 1 ||
      box.right > limit.right + 1 ||
      box.bottom > limit.bottom + 1
    ) {
      found.push(element.tagName.toLowerCase());
    }
  }
  return found;
}

/** The element hit at (x, y), when the point is in the viewport. */
function hitAt(x: number, y: number): Element | null {
  if (x < 0 || y < 0 || x >= innerWidth || y >= innerHeight) return null;
  return document.elementFromPoint(x, y);
}

/** The text nodes in `root` whose whole text is `text`. */
function textNodes(root: Element, text: string): Text[] {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const found: Text[] = [];
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    if (node.textContent === text) found.push(node as Text);
  }
  return found;
}

/** Sets an input's value as typing would, so React sees it. */
function type(input: HTMLInputElement, value: string): void {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

const isHeaderName = (name: string) => name.includes("Vitrine</b>");

export const containmentCases: Case[] = [
  {
    // HS-01, HS-02: the header holds the host's drawing of the name, as text, and only the
    // host's button; the content shaped like a header sits below it
    id: "frame-header-kept",
    async run(context): Promise<Verdict> {
      const { frame, header, body } = await frameWhere(context, isHeaderName);
      const problems: string[] = [];
      const title = header.querySelector("h2");
      if (title?.textContent !== frame.name) problems.push(`the header reads ${JSON.stringify(title?.textContent)}`);
      const elements = [...(title?.querySelectorAll("*") ?? [])].map((element) => element.tagName.toLowerCase());
      if (elements.join() !== "bdi") problems.push(`the name holds elements ${elements.join() || "none"}`);
      const buttons = header.querySelectorAll("button");
      if (buttons.length !== 1 || buttons[0].getAttribute("aria-label") !== `Recarregar ${frame.name}`) {
        problems.push(`the header has ${buttons.length} buttons`);
      }
      const fake = textNodes(body, "Vitrine")[0]?.parentElement;
      if (!fake) problems.push("the fake header is not drawn");
      else if (boxOf(fake).top < boxOf(header).bottom - 0.5) problems.push("the fake header is above the header's bottom");
      return problems.length === 0
        ? { held: true, detail: `the header reads ${JSON.stringify(frame.name)} as text, with the reload alone; the fake header below it` }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-04: the header is hit at its corners and centre, beside content wider and taller than
    // any frame
    id: "frame-header-hittable",
    async run(context): Promise<Verdict> {
      const { frame, header } = await frameWhere(context, (name) => name === "ui-overflow");
      const box = boxOf(header);
      const inset = 3;
      const points: [number, number][] = [
        [box.left + inset, box.top + inset],
        [box.right - inset, box.top + inset],
        [box.left + inset, box.bottom - inset],
        [box.right - inset, box.bottom - inset],
        [(box.left + box.right) / 2, (box.top + box.bottom) / 2],
      ];
      const missed = points.filter(([x, y]) => {
        const hit = hitAt(x, y);
        return !hit || !header.contains(hit);
      });
      return missed.length === 0
        ? { held: true, detail: `${frame.name}: its header is hit at its four corners and centre` }
        : { held: false, detail: `${frame.name}: missed at ${missed.map(([x, y]) => `${Math.round(x)},${Math.round(y)}`).join(" ")}` };
    },
  },
  {
    // HS-03: nothing of the oversized content is hit or painted outside the body
    id: "frame-clips",
    async run(context): Promise<Verdict> {
      const { frame, body } = await frameWhere(context, (name) => name === "ui-overflow");
      const box = boxOf(body);
      const problems: string[] = [];
      for (const fraction of [0.1, 0.5, 0.9]) {
        const x = box.left + (box.right - box.left) * fraction;
        const y = box.top + (box.bottom - box.top) * fraction;
        for (const [px, py] of [
          [box.left - 2, y],
          [box.right + 2, y],
          [x, box.top - 2],
          [x, box.bottom + 2],
        ]) {
          const hit = hitAt(px, py);
          if (hit && body.contains(hit)) problems.push(`content hit at ${Math.round(px)},${Math.round(py)}`);
        }
      }
      const out = outside(body);
      if (out.length > 0) problems.push(`${out.length} content elements paint outside the body (${[...new Set(out)].join(", ")})`);
      return problems.length === 0
        ? { held: true, detail: `${frame.name}: nothing hit beside the body, nothing painted outside it` }
        : { held: false, detail: problems.slice(0, 6).join("; ") };
    },
  },
  {
    // HS-24, HS-07: markup, addresses and paths are text; clicking them goes nowhere
    id: "content-is-text",
    async run(context): Promise<Verdict> {
      const { body } = await frameWhere(context, (name) => name === "ui-strings");
      const problems: string[] = [];
      const text = body.textContent ?? "";
      const missing = HOSTILE.filter((string) => !text.includes(string));
      if (missing.length > 0) problems.push(`not shown as text: ${missing.map((string) => JSON.stringify(string)).join(", ")}`);
      const named = body.querySelectorAll("script, img, b, a, iframe, object, embed, link");
      if (named.length > 0) problems.push(`${named.length} elements named by the content exist`);
      const before = await context.counters();
      const start = location.href;
      for (const string of HOSTILE) {
        for (const node of textNodes(body, string)) {
          const holder = node.parentElement;
          // A submit or action button runs the widget's own declared action, which is harmless
          if (holder && !holder.closest("button")) holder.click();
        }
      }
      await context.wait(1000);
      const after = await context.counters();
      if (after.navigation !== before.navigation || after.newWindow !== before.newWindow) {
        problems.push("a click reached the navigation or new-window handler");
      }
      if (location.href !== start) problems.push("the page left");
      if ((window as unknown as Record<string, unknown>).__probeInjected !== undefined) problems.push("a script ran");
      const violations = context.violations();
      if (violations.length > 0) problems.push(`violations: ${violations.join(", ")}`);
      return problems.length === 0
        ? { held: true, detail: `${HOSTILE.length} strings drawn as text; clicking them did nothing` }
        : { held: false, detail: problems.join("; ").slice(0, 300) };
    },
  },
  {
    // HS-24: each string is isolated, so an override in one cannot reorder what follows
    id: "strings-isolated",
    async run(context): Promise<Verdict> {
      const { body } = await frameWhere(context, (name) => name === "ui-strings");
      const problems: string[] = [];
      let checked = 0;
      for (const string of [...HOSTILE, SENTINEL]) {
        for (const node of textNodes(body, string)) {
          checked += 1;
          const holder = node.parentElement;
          if (holder?.tagName !== "BDI" || getComputedStyle(holder).unicodeBidi !== "isolate") {
            problems.push(`${JSON.stringify(string)} is not in an isolating element`);
          }
        }
      }
      const sentinel = textNodes(body, SENTINEL)[0]?.parentElement;
      const item = sentinel?.closest("li");
      if (!sentinel || !item) problems.push("the sentinel is not drawn");
      else {
        const start = boxOf(item).left + parseFloat(getComputedStyle(item).paddingLeft);
        if (Math.abs(boxOf(sentinel).left - start) > 24) problems.push("the sentinel moved from the start of its line");
      }
      return problems.length === 0
        ? { held: true, detail: `${checked} strings each in an isolating <bdi>; the sentinel kept its place` }
        : { held: false, detail: [...new Set(problems)].join("; ").slice(0, 300) };
    },
  },
  {
    // HS-16: beside a widget shaped like a host dialog, another widget's button is hit and takes
    // focus, and every frame's header and the demo's tabs are hit where they are
    id: "dialog-shaped-widget",
    async run(context): Promise<Verdict> {
      await frameWhere(context, (name) => name === "ui-dialog");
      const problems: string[] = [];
      const showcase = await frameWhere(context, (name) => name === "Vitrine");
      const count = [...showcase.body.querySelectorAll("button")].find((button) => button.textContent === "Contar");
      if (!count) problems.push("the showcase's Contar button is not drawn");
      else {
        count.scrollIntoView({ block: "center" });
        await context.wait(100);
        const box = boxOf(count);
        const hit = hitAt((box.left + box.right) / 2, (box.top + box.bottom) / 2);
        if (!hit || !count.contains(hit)) problems.push("the showcase's Contar button is not hit");
        count.focus();
        if (document.activeElement !== count) problems.push("the showcase's Contar button does not take focus");
      }
      for (const section of document.querySelectorAll<HTMLElement>("section[data-widget]")) {
        section.scrollIntoView({ block: "start" });
        await context.wait(50);
        const header = section.querySelector("header");
        if (!header) continue;
        const box = boxOf(header);
        const hit = hitAt((box.left + box.right) / 2, (box.top + box.bottom) / 2);
        if (!hit || !header.contains(hit)) problems.push(`${section.dataset.widget}'s header is covered`);
      }
      const tabs = document.querySelector('[role="tablist"]');
      if (tabs) {
        tabs.scrollIntoView({ block: "center" });
        await context.wait(50);
        const box = boxOf(tabs);
        const hit = hitAt((box.left + box.right) / 2, (box.top + box.bottom) / 2);
        if (!hit || !tabs.contains(hit)) problems.push("the demo's tabs are covered");
      }
      return problems.length === 0
        ? { held: true, detail: "another widget's button is hit and focused; every header and the tabs are hit" }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-25: with the long select open and an invalid form submitted, nothing leaves the body
    // and the engine validates nothing
    id: "nothing-outside-frame",
    async run(context): Promise<Verdict> {
      const { body } = await frameWhere(context, (name) => name === "ui-strings");
      const problems: string[] = [];
      let invalid = 0;
      const count = () => (invalid += 1);
      document.addEventListener("invalid", count, true);
      try {
        const amount = [...body.querySelectorAll<HTMLInputElement>('input[type="text"]')][1];
        if (!amount) problems.push("the number field is not drawn");
        else type(amount, "abc");
        body.querySelector<HTMLButtonElement>('form button[type="submit"]')?.click();
        await context.wait(100);
        if (!body.querySelector('[aria-invalid="true"]')) problems.push("the host showed no inline error");
        // Clicking its label in content-is-text may have opened it already
        const trigger = body.querySelector<HTMLButtonElement>('button[aria-haspopup="listbox"]');
        if (trigger?.getAttribute("aria-expanded") !== "true") trigger?.click();
        await context.wait(100);
        if (!body.querySelector('[role="listbox"]')) problems.push("the long select did not open");
        const out = outside(body);
        if (out.length > 0) problems.push(`${out.length} elements paint outside the body (${[...new Set(out)].join(", ")})`);
        trigger?.click();
      } finally {
        document.removeEventListener("invalid", count, true);
      }
      if (invalid > 0) problems.push(`${invalid} invalid events fired`);
      const forms = [...document.querySelectorAll("section[data-widget] form")];
      if (forms.some((form) => !(form as HTMLFormElement).noValidate)) problems.push("a form validates");
      if (document.querySelector("[data-frame-body] [title]")) problems.push("content has a title attribute");
      return problems.length === 0
        ? { held: true, detail: "the open select and the inline error stay in the body; no invalid event, every form noValidate, no title" }
        : { held: false, detail: problems.join("; ") };
    },
  },
];

/** What a credential field would carry in its name, autocomplete or hints. */
const CREDENTIAL = /pass|user|login|email|card|otp|token|secret|one-time|cc-/i;

export const controlCases: Case[] = [
  {
    // HS-05: the host refuses every action the widget did not declare, before the plugin runs
    id: "foreign-actions-refused",
    async run(context): Promise<Verdict> {
      const dialog = await handleOf("ui-dialog");
      const foreign = [
        "widget_act",
        "widgets_list",
        "plugin:opener|open_url",
        "shell_probe_report",
        "increment",
        "add",
        "spin",
        "DISMISS",
        "dismiss ",
        "",
      ];
      const problems: string[] = [];
      const before = (await context.counters()).actions;
      for (const name of foreign) {
        const answer = await actWidget(dialog, name, {});
        if (!("refused" in answer) || answer.refused !== "undeclared-action") {
          problems.push(`${JSON.stringify(name)} answered ${failureKind(answer)}`);
        }
      }
      // The dialog's buttons, pressed as a person would: each names an action it did not declare
      const { body } = await frameWhere(context, (name) => name === "ui-dialog");
      for (const button of body.querySelectorAll<HTMLButtonElement>("button")) {
        if (button.textContent === "Fechar") continue;
        button.click();
        await context.wait(150);
      }
      const ran = (await context.counters()).actions - before;
      if (ran !== 0) problems.push(`${ran} actions ran`);
      // The control: the action it declares does run
      const declared = await actWidget(dialog, "dismiss", {});
      const counted = (await context.counters()).actions - before;
      if (!("content" in declared) || counted !== 1) {
        problems.push(`the declared action answered ${failureKind(declared)}, counted ${counted}`);
      }
      return problems.length === 0
        ? { held: true, detail: `${foreign.length} foreign names and the dialog's buttons refused, none ran; dismiss ran` }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-06: no field of a widget looks like a credential field to the engine
    id: "form-no-credentials",
    async run(): Promise<Verdict> {
      const problems: string[] = [];
      const inputs = [...document.querySelectorAll<HTMLInputElement>("[data-frame-body] input")];
      for (const input of inputs) {
        const where = `${input.closest("section")?.getAttribute("data-widget")} ${input.id}`;
        if (input.type !== "text" && input.type !== "checkbox") problems.push(`${where} is type ${input.type}`);
        if (input.getAttribute("autocomplete") !== "off") problems.push(`${where} autocomplete is not off`);
        for (const attribute of ["name", "inputmode", "aria-autocomplete"]) {
          const value = input.getAttribute(attribute);
          if (value && CREDENTIAL.test(value)) problems.push(`${where} ${attribute}=${value}`);
        }
        if (input.name) problems.push(`${where} has a name`);
      }
      if (document.querySelector("[data-frame-body] textarea, [data-frame-body] select")) {
        problems.push("a field is a textarea or a native select");
      }
      const texts = inputs.filter((input) => input.type === "text").length;
      return problems.length === 0 && texts > 0
        ? { held: true, detail: `${inputs.length} inputs, ${texts} text entries, all type text with autocomplete off and no credential name` }
        : { held: false, detail: problems.join("; ").slice(0, 300) || "no text entry drawn" };
    },
  },
];
