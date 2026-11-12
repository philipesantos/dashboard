// The installation cases (specs/010-plugin-installation/research.md, R11, R16): driven in the
// real page through the chrome's add control and the host dialog, against the probe's release
// source, which serves the embedded releases as `github.com/dashboard-fixtures/<name>`.
// Part of the security core (constitution v2.7.0, Principle IX).

import type { Case, Context, Verdict } from "./case";

const POLL_MS = 50;
/** How long an installation gets, from the add button to the frame's content (SC-001's ceiling is
 * a minute; the probe's budget for the whole run is less, so it waits at most this). */
const INSTALL_MS = 20000;

/** Sets an input's value as typing would, so React sees it. */
function type(input: HTMLInputElement, value: string): void {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

async function until<T>(context: Context, ms: number, find: () => T | null | undefined): Promise<T | null> {
  for (let waited = 0; waited <= ms; waited += POLL_MS) {
    const found = find();
    if (found) return found;
    await context.wait(POLL_MS);
  }
  return null;
}

/** The frame of an installed widget, by the owner its header shows. */
function installedFrame(owner: string): HTMLElement | null {
  for (const frame of document.querySelectorAll<HTMLElement>("section[data-widget]")) {
    if (frame.querySelector("[data-frame-source]")?.textContent === owner) return frame;
  }
  return null;
}

/** Installs `address` through the chrome and the dialog, approving as soon as the dialog lets the
 * approve button be pressed. Returns the time from the add button to the frame's first drawn state
 * (its content, or the host's state for content that is not in the widget format, as with
 * `install-basic`, whose component is a test fixture), or what went wrong. */
async function installThroughThePage(
  context: Context,
  address: string,
): Promise<{ ms: number; view: string } | { problem: string }> {
  const input = document.querySelector<HTMLInputElement>("[data-add-address]");
  const add = document.querySelector<HTMLButtonElement>("[data-add-widget]");
  if (!input || !add) return { problem: "the chrome has no add control" };
  type(input, address);
  await context.wait(POLL_MS);
  const start = performance.now();
  add.click();
  const approve = await until(context, INSTALL_MS, () => {
    const button = document.querySelector<HTMLButtonElement>("[data-host-dialog] [data-approve]");
    return button && !button.disabled ? button : null;
  });
  if (!approve) {
    const status = document.querySelector("[data-add-status]")?.textContent ?? "";
    return { problem: `no dialog to approve; the chrome says: ${status}` };
  }
  approve.click();
  const owner = "github.com/dashboard-fixtures";
  const drawn = await until(context, INSTALL_MS, () => {
    const frame = installedFrame(owner);
    return frame && frame.dataset.view !== "loading" && frame.dataset.view !== "not-loaded" ? frame : null;
  });
  if (!drawn) return { problem: `the installed widget was not drawn (${installedFrame(owner)?.dataset.view ?? "no frame"})` };
  return { ms: Math.round(performance.now() - start), view: drawn.dataset.view ?? "" };
}

export const installCases: Case[] = [
  {
    // SC-001, without the network: from the add button to the installed widget's first content,
    // through the real installer, compile process and dialog. T054 records the time per system.
    id: "install-on-time",
    async run(context): Promise<Verdict> {
      const outcome = await installThroughThePage(context, "github.com/dashboard-fixtures/install-basic");
      if ("problem" in outcome) return { held: false, detail: outcome.problem };
      return {
        held: outcome.ms <= 10000,
        detail: `installed and drawn (${outcome.view}) ${outcome.ms} ms after the add button (limit 10000 ms)`,
      };
    },
  },
  {
    // HS-02, FR-030: the installed widget's header shows its repository's owner after its name,
    // from the host's record; the bundled showcase's shows none
    id: "frame-owner-shown",
    async run(): Promise<Verdict> {
      const problems: string[] = [];
      const installed = installedFrame("github.com/dashboard-fixtures");
      if (!installed) problems.push("no frame shows the owner github.com/dashboard-fixtures");
      const header = installed?.querySelector("header h2");
      const name = header?.firstElementChild?.textContent;
      if (installed && name !== "Instalação básica") problems.push(`the name drawn first is ${name}`);
      for (const frame of document.querySelectorAll<HTMLElement>("section[data-widget]")) {
        const title = frame.querySelector("header h2")?.firstElementChild?.textContent;
        if (title === "Vitrine" && frame.querySelector("[data-frame-source]")) {
          problems.push("the bundled showcase shows a source");
        }
      }
      return {
        held: problems.length === 0,
        detail: problems.length === 0 ? "the owner is drawn after the installed widget's name, and the showcase has none" : problems.join("; "),
      };
    },
  },
  {
    // SC-003: while a package just under the cap compiles, the window keeps answering: no gap
    // between animation frames, or between 10 ms timers, above 100 ms, until the dialog opens
    id: "window-responsive-while-compiling",
    async run(context): Promise<Verdict> {
      const input = document.querySelector<HTMLInputElement>("[data-add-address]");
      const add = document.querySelector<HTMLButtonElement>("[data-add-widget]");
      if (!input || !add) return { held: false, detail: "the chrome has no add control" };
      type(input, "github.com/dashboard-fixtures/compile-heavy");
      await context.wait(POLL_MS);
      let frameGap = 0;
      let timerGap = 0;
      let last = performance.now();
      let lastTimer = last;
      let watching = true;
      // The interval in which the dialog appears is the page drawing it, not the compile: it is
      // not counted, and the watch stops there
      const frame = (now: number) => {
        if (!watching || dialog()) return;
        frameGap = Math.max(frameGap, now - last);
        last = now;
        requestAnimationFrame(frame);
      };
      const tick = () => {
        if (!watching || dialog()) return;
        const now = performance.now();
        timerGap = Math.max(timerGap, now - lastTimer);
        lastTimer = now;
        setTimeout(tick, 10);
      };
      requestAnimationFrame(frame);
      setTimeout(tick, 10);
      const start = performance.now();
      add.click();
      const opened = await until(context, INSTALL_MS, () => dialog());
      watching = false;
      const ms = Math.round(performance.now() - start);
      await context.wait(100);
      focusedAtOpen = document.activeElement?.hasAttribute("data-safe-choice") ?? false;
      if (!opened) return { held: false, detail: "the dialog never opened" };
      const worst = Math.round(Math.max(frameGap, timerGap));
      return {
        held: worst <= 100,
        detail: `the dialog opened after ${ms} ms; the largest gap was ${worst} ms (frames ${Math.round(frameGap)}, timers ${Math.round(timerGap)}; limit 100)`,
      };
    },
  },
  {
    // HS-10: the dialog or its scrim is what is hit over the chrome and over every frame
    id: "dialog-covers-window",
    async run(): Promise<Verdict> {
      const shown = dialog();
      if (!shown) return { held: false, detail: "no dialog is open" };
      const targets: [string, Element | null][] = [["the chrome", document.querySelector("#root header")]];
      for (const frame of document.querySelectorAll("section[data-widget]")) targets.push([`frame ${(frame as HTMLElement).dataset.widget}`, frame]);
      const missed: string[] = [];
      for (const [name, element] of targets) {
        if (!element) continue;
        const box = element.getBoundingClientRect();
        const x = Math.min(Math.max(box.left + box.width / 2, 1), innerWidth - 1);
        const y = Math.min(Math.max(box.top + box.height / 2, 1), innerHeight - 1);
        const hit = document.elementFromPoint(x, y);
        if (!hit || !(hit.closest("[data-host-dialog]") || hit.closest("[data-slot=dialog-overlay]"))) missed.push(name);
      }
      return missed.length === 0
        ? { held: true, detail: `the dialog or its scrim is hit over the chrome and ${targets.length - 1} frames` }
        : { held: false, detail: `not covered: ${missed.join(", ")}` };
    },
  },
  {
    // HS-11: the rest of the window is inert: nothing outside the dialog takes focus
    id: "dialog-makes-window-inert",
    async run(): Promise<Verdict> {
      const root = document.getElementById("root");
      const problems: string[] = [];
      if (!root?.hasAttribute("inert")) problems.push("the app's root is not inert");
      const outside = [...document.querySelectorAll<HTMLElement>("#root button, #root input")];
      const took = outside.filter((element) => {
        element.focus();
        return document.activeElement === element;
      });
      if (took.length > 0) problems.push(`${took.length} controls outside took focus`);
      dialog()?.querySelector<HTMLElement>("[data-safe-choice]")?.focus();
      return problems.length === 0
        ? { held: true, detail: `the root is inert; none of ${outside.length} controls outside took focus` }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-12: input during the delay does nothing, and the delay starts over when the window
    // regains focus; after it, the dialog takes input
    id: "dialog-input-delay",
    async run(context): Promise<Verdict> {
      const problems: string[] = [];
      window.dispatchEvent(new Event("focus"));
      await context.wait(POLL_MS);
      const shown = dialog();
      if (!shown) return { held: false, detail: "no dialog is open" };
      if (shown.hasAttribute("data-armed")) problems.push("the delay did not start over on focus");
      const approve = shown.querySelector<HTMLButtonElement>("[data-approve]");
      if (!approve?.disabled) problems.push("approve is not disabled during the delay");
      shown.querySelector<HTMLElement>("[data-safe-choice]")?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Enter", bubbles: true }),
      );
      approve?.click();
      await context.wait(POLL_MS);
      if (!dialog()) problems.push("input during the delay closed the dialog");
      await context.wait(1200);
      if (!dialog()?.hasAttribute("data-armed")) problems.push("the delay did not end");
      if (dialog()?.querySelector<HTMLButtonElement>("[data-approve]")?.disabled) problems.push("approve stayed disabled");
      return problems.length === 0
        ? { held: true, detail: "Enter and a click during the delay did nothing; focus started it over; it ended after a second" }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-13: who is asking, by its name and its id in full, drawn as text
    id: "dialog-names-who-asks",
    async run(): Promise<Verdict> {
      const asking = dialog()?.querySelector("[data-asking]");
      const id = asking?.querySelector("[data-asking-id]")?.textContent;
      const problems: string[] = [];
      if (id !== "github.com/dashboard-fixtures/compile-heavy") problems.push(`the id drawn is ${id}`);
      const elements = [...(asking?.querySelectorAll("*") ?? [])].filter((element) => element.tagName !== "BDI");
      if (elements.length > 0) problems.push(`the asking line holds ${elements.map((e) => e.tagName).join(", ")}`);
      if (!asking?.textContent?.includes("Compilação pesada")) problems.push("the name is not drawn");
      return problems.length === 0
        ? { held: true, detail: "the name, the id in full and the author are drawn as text" }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-15: the dialog fits the window; its title and its safe choice stay in view, only its
    // body scrolls
    id: "dialog-scrolls-inside",
    async run(): Promise<Verdict> {
      const shown = dialog();
      if (!shown) return { held: false, detail: "no dialog is open" };
      const inView = (element: Element | null) => {
        const box = element?.getBoundingClientRect();
        return !!box && box.top >= 0 && box.bottom <= innerHeight && box.left >= 0 && box.right <= innerWidth;
      };
      const problems: string[] = [];
      if (!inView(shown)) problems.push("the dialog is larger than the window");
      if (!inView(shown.querySelector("[data-slot=dialog-title]"))) problems.push("the title is out of view");
      if (!inView(shown.querySelector("[data-safe-choice]"))) problems.push("the safe choice is out of view");
      const body = shown.querySelector<HTMLElement>("[data-dialog-scroll]");
      if (!body || getComputedStyle(body).overflowY !== "auto") problems.push("the body does not scroll");
      return problems.length === 0
        ? { held: true, detail: "the dialog fits; title and safe choice in view; the body scrolls" }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-17: in forced colors the dialog keeps an edge. The engines of macOS and Linux never match
    // forced-colors, so the stylesheet's rule is read; Windows high contrast is a review step
    id: "dialog-forced-colors",
    async run(): Promise<Verdict> {
      const found = rules().some(
        ({ rule, media }) =>
          media.some((condition) => condition.includes("forced-colors: active")) &&
          rule.selectorText.includes("[data-host-dialog]") &&
          rule.style.borderStyle !== "" &&
          rule.style.borderStyle !== "none",
      );
      return found
        ? { held: true, detail: "a forced-colors rule gives the host dialog a border" }
        : { held: false, detail: "no forced-colors rule draws the host dialog's edge" };
    },
  },
  {
    // HS-18: focus started on the safe choice, and Tab and Shift+Tab wrap inside the dialog
    id: "dialog-focus-trapped",
    async run(context): Promise<Verdict> {
      const shown = dialog();
      if (!shown) return { held: false, detail: "no dialog is open" };
      const problems: string[] = [];
      if (!focusedAtOpen) problems.push("focus did not start on the safe choice");
      const tabbable = [...shown.querySelectorAll<HTMLElement>("button:not([disabled]), [role=switch]:not([disabled]), input")];
      const first = tabbable[0];
      const last = tabbable[tabbable.length - 1];
      if (!first?.hasAttribute("data-safe-choice")) problems.push("the safe choice is not first in tab order");
      last?.focus();
      last?.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", bubbles: true, cancelable: true }));
      await context.wait(POLL_MS);
      if (document.activeElement !== first) problems.push("Tab past the last control left the dialog");
      first?.dispatchEvent(new KeyboardEvent("keydown", { key: "Tab", shiftKey: true, bubbles: true, cancelable: true }));
      await context.wait(POLL_MS);
      if (document.activeElement !== last) problems.push("Shift+Tab past the first control left the dialog");
      first?.focus();
      return problems.length === 0
        ? { held: true, detail: `focus started on the safe choice; Tab wraps among ${tabbable.length} controls` }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-21: a dialog, named by its title and described by who is asking and what is decided
    id: "dialog-announced",
    async run(): Promise<Verdict> {
      const shown = dialog();
      const problems: string[] = [];
      if (shown?.getAttribute("role") !== "dialog") problems.push(`role is ${shown?.getAttribute("role")}`);
      const label = document.getElementById(shown?.getAttribute("aria-labelledby") ?? "");
      const description = document.getElementById(shown?.getAttribute("aria-describedby") ?? "");
      if (!label?.textContent?.startsWith("Instalar")) problems.push("no name from the title");
      if (!description?.textContent?.includes("github.com/dashboard-fixtures/compile-heavy")) problems.push("no description of who is asking");
      return problems.length === 0
        ? { held: true, detail: "role dialog, named by its title, described by who is asking" }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-22: everything outside the dialog is hidden from assistive technology
    id: "dialog-hides-the-rest",
    async run(): Promise<Verdict> {
      const shown = dialog();
      const visible = [...document.body.children].filter(
        (element) =>
          !element.contains(shown) &&
          element.tagName !== "SCRIPT" &&
          element.getAttribute("aria-hidden") !== "true" &&
          !element.hasAttribute("inert"),
      );
      return visible.length === 0
        ? { held: true, detail: "every part of the page outside the dialog is aria-hidden or inert" }
        : { held: false, detail: `still exposed: ${visible.map((element) => element.tagName + (element.id ? `#${element.id}` : "")).join(", ")}` };
    },
  },
  {
    // HS-23: the dialog's and the scrim's animations exist only for those who did not ask for
    // reduced motion; reduced motion itself is a review step
    id: "dialog-reduced-motion",
    async run(): Promise<Verdict> {
      const shown = dialog();
      const scrim = document.querySelector("[data-slot=dialog-overlay]");
      const loose = [shown, scrim].flatMap((element) =>
        [...(element?.classList ?? [])].filter((name) => /(^|:)(animate-|fade-|zoom-)/.test(name) && !name.startsWith("motion-safe:")),
      );
      return loose.length === 0
        ? { held: true, detail: "every animation class of the dialog and its scrim is motion-safe" }
        : { held: false, detail: `animated whatever the setting: ${loose.join(" ")}` };
    },
  },
  {
    // HS-19, HS-20: Escape takes the safe choice, from a control that is not the safe one, and
    // focus returns to the control that opened the dialog
    id: "dialog-escape-safe",
    async run(context): Promise<Verdict> {
      const shown = dialog();
      const approve = shown?.querySelector<HTMLButtonElement>("[data-approve]");
      if (!approve) return { held: false, detail: "no approve button" };
      approve.focus();
      approve.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
      const closed = await until(context, 2000, () => (dialog() ? null : true));
      const problems: string[] = [];
      if (!closed) problems.push("Escape did not close the dialog");
      await context.wait(POLL_MS);
      if (installedNamed("Compilação pesada")) problems.push("Escape installed the widget");
      if (!document.activeElement?.hasAttribute("data-add-widget")) {
        problems.push(`focus went to ${document.activeElement?.tagName ?? "nothing"}, not the add button`);
      }
      if (document.getElementById("root")?.hasAttribute("inert")) problems.push("the root stayed inert");
      return problems.length === 0
        ? { held: true, detail: "Escape on approve took the safe choice; focus is back on the add button" }
        : { held: false, detail: problems.join("; ") };
    },
  },
  {
    // HS-14: on a dialog that just opened, Enter after the delay, with focus where it starts,
    // takes the safe choice
    id: "dialog-enter-safe",
    async run(context): Promise<Verdict> {
      const input = document.querySelector<HTMLInputElement>("[data-add-address]");
      const add = document.querySelector<HTMLButtonElement>("[data-add-widget]");
      if (!input || !add) return { held: false, detail: "the chrome has no add control" };
      type(input, "github.com/dashboard-fixtures/compile-heavy");
      await context.wait(POLL_MS);
      add.click();
      const shown = await until(context, INSTALL_MS, () => dialog());
      if (!shown) return { held: false, detail: "the dialog never opened" };
      await until(context, 3000, () => (dialog()?.hasAttribute("data-armed") ? true : null));
      (document.activeElement ?? shown).dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
      const closed = await until(context, 2000, () => (dialog() ? null : true));
      await context.wait(300);
      const problems: string[] = [];
      if (!closed) problems.push("Enter did not close the dialog");
      if (installedNamed("Compilação pesada")) problems.push("Enter installed the widget");
      return problems.length === 0
        ? { held: true, detail: "Enter after the delay, with focus where it started, took the safe choice" }
        : { held: false, detail: problems.join("; ") };
    },
  },
];

/** The host dialog that is open, if one is. */
function dialog(): HTMLElement | null {
  return document.querySelector<HTMLElement>("[data-host-dialog]");
}

/** Whether a frame is drawn under `name`. */
function installedNamed(name: string): boolean {
  return [...document.querySelectorAll("section[data-widget] header h2")].some(
    (header) => header.firstElementChild?.textContent === name,
  );
}

/** Whether focus was on the safe choice right after the compile case's dialog opened. */
let focusedAtOpen = false;

/** Every style rule of the page's stylesheets, with the media conditions around it. */
function rules(): { rule: CSSStyleRule; media: string[] }[] {
  const found: { rule: CSSStyleRule; media: string[] }[] = [];
  const walk = (list: CSSRuleList, media: string[]) => {
    for (const rule of list) {
      if (rule instanceof CSSMediaRule) walk(rule.cssRules, [...media, rule.conditionText]);
      else if (rule instanceof CSSStyleRule) {
        found.push({ rule, media });
        if (rule.cssRules.length > 0) walk(rule.cssRules, media);
      } else if ("cssRules" in rule) walk((rule as CSSGroupingRule).cssRules, media);
    }
  };
  for (const sheet of document.styleSheets) {
    try {
      walk(sheet.cssRules, []);
    } catch {
      // A stylesheet the page may not read is not the app's
    }
  }
  return found;
}
