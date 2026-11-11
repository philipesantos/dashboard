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
