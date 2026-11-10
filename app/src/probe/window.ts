// The window's cases (spec 008 FR-009 to FR-011; contracts/probe.md): the page cannot navigate
// away, open a window, or bring up the engine's menu. A navigation that succeeds unloads the
// probe, so these run last, and a missing report fails the run.
// Part of the security core (constitution, Principle IX).

import { type Case, type Context, type Counters, REMOTE, type Verdict } from "./case";

/** Runs `action`, then holds if the handler `kind` refused something and the page stayed. */
async function refusedAndStayed(
  context: Context,
  kind: keyof Counters,
  action: () => void,
): Promise<Verdict> {
  const before = (await context.counters())[kind];
  action();
  const stayed = await context.stayed();
  const refusals = (await context.counters())[kind] - before;
  const parts = [
    refusals > 0 ? `${kind} handler refused ${refusals}` : `no ${kind} refusal`,
    stayed ? "the page stayed" : "the page left",
  ];
  return { held: refusals > 0 && stayed, detail: parts.join(", ") };
}

function clickLink(href: string, target?: string): void {
  const link = document.createElement("a");
  link.href = href;
  if (target) link.target = target;
  document.body.append(link);
  link.click();
  link.remove();
}

const navigation: Case[] = [
  {
    id: "navigate-location",
    mayLeave: true,
    run: (context) =>
      refusedAndStayed(context, "navigation", () => {
        location.href = `${REMOTE}/location`;
      }),
  },
  {
    id: "navigate-assign",
    mayLeave: true,
    run: (context) =>
      refusedAndStayed(context, "navigation", () => location.assign(`${REMOTE}/assign`)),
  },
  {
    id: "navigate-meta-refresh",
    mayLeave: true,
    run: (context) =>
      refusedAndStayed(context, "navigation", () => {
        const meta = document.createElement("meta");
        meta.httpEquiv = "refresh";
        meta.content = `0;url=${REMOTE}/refresh`;
        document.head.append(meta);
      }),
  },
  {
    id: "navigate-link",
    mayLeave: true,
    run: (context) => refusedAndStayed(context, "navigation", () => clickLink(`${REMOTE}/link`)),
  },
  {
    // Another bundled file: the app's own origin, but not its page
    id: "navigate-own-asset",
    mayLeave: true,
    run: (context) =>
      refusedAndStayed(context, "navigation", () => {
        location.href = new URL("/vite.svg", location.href).href;
      }),
  },
  {
    // The engine may refuse a cross-scheme load before the handler sees it; the page must stay
    id: "navigate-file-scheme",
    mayLeave: true,
    async run(context) {
      try {
        location.href = "file:///etc/hostname";
      } catch {
        // A refusal may throw; staying decides.
      }
      const stayed = await context.stayed();
      return { held: stayed, detail: stayed ? "the page stayed" : "the page left" };
    },
  },
];

/** Holds if the page stayed; the host adds that the app still has exactly one window. */
async function noSecondWindow(context: Context, action: () => void): Promise<Verdict> {
  const before = (await context.counters()).newWindow;
  action();
  const stayed = await context.stayed();
  const refusals = (await context.counters()).newWindow - before;
  return {
    held: stayed,
    singleWindow: true,
    detail: `${refusals > 0 ? `new-window handler refused ${refusals}` : "no new-window request reached the handler"}, ${stayed ? "the page stayed" : "the page left"}`,
  };
}

const windows: Case[] = [
  {
    id: "window-open",
    mayLeave: true,
    run: (context) => noSecondWindow(context, () => void window.open(`${REMOTE}/open`)),
  },
  {
    id: "window-target-blank",
    mayLeave: true,
    run: (context) => noSecondWindow(context, () => clickLink(`${REMOTE}/blank`, "_blank")),
  },
];

const contextMenu: Case = {
  id: "context-menu",
  async run() {
    const event = new MouseEvent("contextmenu", { bubbles: true, cancelable: true });
    document.body.dispatchEvent(event);
    return event.defaultPrevented
      ? { held: true, detail: "contextmenu was cancelled" }
      : { held: false, detail: "contextmenu was not cancelled, so the engine's menu would open" };
  },
};

export const windowCases: Case[] = [contextMenu, ...navigation, ...windows];
