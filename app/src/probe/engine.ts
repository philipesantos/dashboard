// What the engine itself would grant or draw (spec 008 FR-021; contracts/probe.md): web
// permissions, downloads and the engine's own dialogs. Each must be refused, or return at once
// with the replaced value.
// Part of the security core (constitution, Principle IX).

import { type Case, type Context, type Verdict } from "./case";

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
          return `refused: ${error instanceof Error ? error.name : String(error)}`;
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
    const before = (await context.counters()).download;
    const link = document.createElement("a");
    link.href = new URL("/vite.svg", location.href).href;
    link.download = "probe.svg";
    document.body.append(link);
    link.click();
    link.remove();
    const stayed = await context.stayed();
    const refusals = (await context.counters()).download - before;
    return {
      held: refusals > 0 && stayed,
      detail: `${refusals > 0 ? `download handler refused ${refusals}` : "no download refusal"}, ${stayed ? "the page stayed" : "the page left"}`,
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

export const engineCases: Case[] = [...permissions, download, ...dialogs];
