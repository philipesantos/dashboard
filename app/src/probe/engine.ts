// What the engine itself would grant or draw (spec 008 FR-021; contracts/probe.md): web
// permissions, downloads and the engine's own dialogs. Each must be refused, or return at once
// with the replaced value.
// Part of the security core (constitution, Principle IX).

import { type Case, type Context, ON_WEBVIEW2, type Verdict } from "./case";

/** `promise`, or "pending" if it has not settled after `ms`. */
async function within<T>(promise: Promise<T>, ms: number): Promise<T | "pending"> {
  return Promise.race([
    promise,
    new Promise<"pending">((resolve) => setTimeout(() => resolve("pending"), ms)),
  ]);
}

async function permissionRefusals(context: Context, action: () => Promise<string>): Promise<Verdict> {
  const before = (await context.counters()).permission;
  const outcome = await within(action(), 3000);
  const refusals = (await context.counters()).permission - before;
  const handler = refusals > 0 ? `; the permission handler denied ${refusals}` : "";
  if (outcome === "pending") {
    return { held: false, detail: `no answer within 3 s, so an engine prompt may be open${handler}` };
  }
  const held = outcome.startsWith("refused");
  return { held, detail: `${outcome}${handler}` };
}

const permissions: Case[] = [
  {
    id: "permission-camera",
    run: (context) =>
      permissionRefusals(context, async () => {
        if (!navigator.mediaDevices?.getUserMedia) return "refused: the engine offers no getUserMedia";
        try {
          const stream = await navigator.mediaDevices.getUserMedia({ video: true });
          stream.getTracks().forEach((track) => track.stop());
          return "granted: the camera opened";
        } catch (error) {
          const name = error instanceof Error ? error.name : String(error);
          // CI gives WebView2 a fake camera, so a refusal there must be the permission's, not
          // a missing device; elsewhere the runners have no camera (T028 review)
          if (ON_WEBVIEW2 && name !== "NotAllowedError") {
            return `not refused by permission: ${name}`;
          }
          return `refused: ${name}`;
        }
      }),
  },
  {
    id: "permission-location",
    run: (context) =>
      permissionRefusals(
        context,
        () =>
          new Promise<string>((resolve) => {
            if (!navigator.geolocation) {
              resolve("refused: the engine offers no geolocation");
              return;
            }
            navigator.geolocation.getCurrentPosition(
              () => resolve("granted: a position came back"),
              (error) =>
                resolve(
                  error.code === error.PERMISSION_DENIED
                    ? "refused: PERMISSION_DENIED"
                    : `granted, then failed: code ${error.code}`,
                ),
              { timeout: 2500 },
            );
          }),
      ),
  },
  {
    id: "permission-notification",
    run: (context) =>
      permissionRefusals(context, async () => {
        if (typeof Notification === "undefined") return "refused: the engine offers no Notification";
        const answer = await Notification.requestPermission();
        return answer === "denied" ? "refused: denied" : `not denied: ${answer}`;
      }),
  },
];

const download: Case = {
  id: "download-link",
  mayLeave: true,
  async run(context) {
    const start = await context.counters();
    const before = start.download;
    const link = document.createElement("a");
    link.href = new URL("/vite.svg", location.href).href;
    link.download = "probe.svg";
    document.body.append(link);
    link.click();
    link.remove();
    const stayed = await context.stayed();
    const end = await context.counters();
    const refusals = end.download - before;
    const navigations = end.navigation - start.navigation;
    // WEBKITGTK-ONLY(probe-download-as-navigation): WebKitGTK treats the click as a navigation
    // first, which on_navigation refuses before any download starts; WKWebView hands it to
    // on_download (research R14). Either refusal holds.
    const by =
      refusals > 0
        ? `download handler refused ${refusals}`
        : navigations > 0
          ? `navigation handler refused ${navigations}, before any download`
          : "neither handler refused it";
    return {
      held: (refusals > 0 || navigations > 0) && stayed,
      detail: `${by}, ${stayed ? "the page stayed" : "the page left"}`,
    };
  },
};

/** Holds when `name` is no longer the engine's function and returns `expected` at once. */
function replacedDialog(id: string, name: "alert" | "confirm" | "prompt" | "print", expected: unknown): Case {
  return {
    id,
    // An engine dialog that was not replaced blocks the page until someone answers it
    mayLeave: true,
    async run() {
      const dialog = window[name] as (...args: unknown[]) => unknown;
      const native = /\[native code\]/.test(Function.prototype.toString.call(dialog));
      const start = performance.now();
      const value = dialog.call(window, "probe");
      const ms = Math.round(performance.now() - start);
      const held = !native && value === expected && ms < 500;
      return {
        held,
        detail: `${native ? "the engine's own" : "replaced"}; returned ${String(value)} in ${ms} ms`,
      };
    },
  };
}

const dialogs: Case[] = [
  replacedDialog("dialog-alert", "alert", undefined),
  replacedDialog("dialog-confirm", "confirm", false),
  replacedDialog("dialog-prompt", "prompt", null),
  replacedDialog("dialog-print", "print", undefined),
];

function isNative(value: unknown): boolean {
  return typeof value === "function" && /\[native code\]/.test(Function.prototype.toString.call(value));
}

/** No file picker (FR-021): a click on a file input is cancelled, and no picker API is left. */
const filePicker: Case = {
  id: "file-picker",
  async run() {
    const input = document.createElement("input");
    input.type = "file";
    document.body.append(input);
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    input.dispatchEvent(click);
    input.remove();
    const problems: string[] = [];
    if (!click.defaultPrevented) problems.push("a click on a file input was not cancelled");
    if (isNative(HTMLInputElement.prototype.click)) problems.push("input.click() is the engine's");
    if (isNative(HTMLInputElement.prototype.showPicker)) problems.push("showPicker() is the engine's");
    for (const name of ["showOpenFilePicker", "showSaveFilePicker", "showDirectoryPicker"]) {
      if ((window as unknown as Record<string, unknown>)[name] !== undefined) problems.push(`${name} exists`);
    }
    return problems.length === 0
      ? { held: true, detail: "file input clicks cancelled, no picker API" }
      : { held: false, detail: problems.join("; ") };
  },
};

/**
 * A frame with no `src` is the page's own origin and the content policy allows it; the engine's
 * dialogs must be replaced there too. Checked without calling them, since the engine's own would
 * block the page.
 */
const frameDialogs: Case = {
  id: "dialog-iframe",
  async run(context) {
    const frame = document.createElement("iframe");
    document.body.append(frame);
    await context.wait(100);
    const inner = frame.contentWindow as (Window & typeof globalThis) | null;
    const native = inner
      ? (["alert", "confirm", "prompt", "print"] as const).filter((name) => isNative(inner[name]))
      : [];
    frame.remove();
    if (!inner) return { held: true, detail: "the frame has no window" };
    return native.length === 0
      ? { held: true, detail: "the frame's dialogs are replaced" }
      : { held: false, detail: `the engine's own in the frame: ${native.join(", ")}` };
  },
};

export const engineCases: Case[] = [...permissions, download, ...dialogs, filePicker, frameDialogs];
