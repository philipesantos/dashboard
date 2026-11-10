// The window's cases (spec 008 FR-009 to FR-011; contracts/probe.md): the page cannot navigate
// away, open a window, or bring up the engine's menu. A navigation that succeeds unloads the
// probe, so these run last, and a missing report fails the run.
// Part of the security core (constitution, Principle IX).

import {
  type Case,
  type Context,
  type Counters,
  ON_WEBVIEW2,
  OTHER_APP_ORIGIN,
  REMOTE,
  type Verdict,
} from "./case";

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
    // The other system's form of the app's origin is not the app here (T028 review)
    id: "navigate-other-app-origin",
    mayLeave: true,
    run: (context) =>
      refusedAndStayed(context, "navigation", () => {
        location.href = `${OTHER_APP_ORIGIN}/`;
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

/**
 * Holds if the page stayed and, where the engine hands a scripted popup to the app, the handler
 * refused it. The host adds that it still has exactly one window, but a window the handler let
 * through would be the engine's own, which the host does not count, so the refusal is what
 * decides. The macOS and Linux engines refuse a popup with no user gesture before asking the
 * app; WebView2 asks (T028 review).
 */
async function noSecondWindow(context: Context, action: () => void): Promise<Verdict> {
  const before = (await context.counters()).newWindow;
  action();
  const stayed = await context.stayed();
  const refusals = (await context.counters()).newWindow - before;
  return {
    held: stayed && (!ON_WEBVIEW2 || refusals > 0),
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

const close: Case = {
  // WEBKITGTK-ONLY(window-close): WebKitGTK destroys the webview on window.close(); the page
  // must stay
  id: "window-close",
  mayLeave: true,
  async run(context) {
    window.close();
    const stayed = await context.stayed();
    return { held: stayed, detail: stayed ? "the page stayed" : "the page closed" };
  },
};

export const windowCases: Case[] = [contextMenu, ...navigation, ...windows, close];
