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
];
