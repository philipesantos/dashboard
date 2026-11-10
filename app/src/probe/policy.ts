// The content policy's cases (spec 008 FR-001 and FR-002; contracts/probe.md). Each tries
// something the release policy forbids, and holds only when the engine refuses it.
// Part of the security core (constitution, Principle IX).

import {
  type Case,
  type Context,
  REMOTE,
  expectThrow,
  expectViolation,
  flag,
  violation,
} from "./case";

/** Whether contracts/content-policy.md records the `style-src-elem 'unsafe-inline'` allowance. */
const STYLE_ALLOWANCE = false;

function append<T extends HTMLElement>(element: T): T {
  document.body.append(element);
  return element;
}

const injected: Case[] = [
  {
    id: "script-inline-element",
    async run(context) {
      const script = document.createElement("script");
      script.textContent = "window.__probeInlineElement = 1;";
      append(script);
      const verdict = await expectViolation(
        context,
        ["script-src"],
        flag("__probeInlineElement"),
      );
      script.remove();
      return verdict;
    },
  },
  {
    id: "script-inline-attribute",
    async run(context) {
      const holder = append(document.createElement("div"));
      // A same-origin image that does not exist, so `onerror` fires and only script-src-attr
      // can stop it.
      holder.innerHTML =
        '<img src="/__probe-missing.png" alt="" onerror="window.__probeInlineAttribute = 1">';
      const verdict = await expectViolation(
        context,
        ["script-src"],
        flag("__probeInlineAttribute"),
      );
      holder.remove();
      return verdict;
    },
  },
  {
    id: "script-eval",
    async run() {
      return expectThrow(() => eval("1"));
    },
  },
  {
    id: "script-function",
    async run() {
      return expectThrow(() => new Function("return 1")());
    },
  },
  {
    // Tauri's freezePrototype (research R9): pollution of Object.prototype cannot reach the API
    id: "prototype-frozen",
    async run() {
      const frozen = Object.isFrozen(Object.prototype);
      return { held: frozen, detail: frozen ? "Object.prototype is frozen" : "Object.prototype is writable" };
    },
  },
  {
    id: "script-timeout-string",
    async run(context) {
      setTimeout("window.__probeTimeoutString = 1;" as unknown as () => void, 0);
      return expectViolation(context, ["script-src"], flag("__probeTimeoutString"));
    },
  },
];

const loads: Case[] = [
  {
    id: "load-script-remote",
    async run(context) {
      const script = document.createElement("script");
      script.src = `${REMOTE}/probe.js`;
      append(script);
      const verdict = await expectViolation(context, ["script-src"]);
      script.remove();
      return verdict;
    },
  },
  {
    id: "load-style-remote",
    async run(context) {
      const link = document.createElement("link");
      link.rel = "stylesheet";
      link.href = `${REMOTE}/probe.css`;
      document.head.append(link);
      const verdict = await expectViolation(context, ["style-src"]);
      link.remove();
      return verdict;
    },
  },
  {
    id: "load-style-import",
    async run(context) {
      const style = document.createElement("style");
      style.textContent = `@import url("${REMOTE}/import.css");`;
      document.head.append(style);
      const verdict = await expectViolation(context, ["style-src"]);
      style.remove();
      return verdict;
    },
  },
  {
    id: "load-image-remote",
    async run(context) {
      const image = document.createElement("img");
      image.src = `${REMOTE}/probe.png`;
      append(image);
      const verdict = await expectViolation(context, ["img-src"]);
      image.remove();
      return verdict;
    },
  },
  {
    id: "load-font-remote",
    async run(context) {
      const font = new FontFace("ProbeRemote", `url(${REMOTE}/probe.woff2)`);
      await font.load().catch(() => undefined);
      return expectViolation(context, ["font-src"]);
    },
  },
  {
    id: "load-media-remote",
    async run(context) {
      const audio = document.createElement("audio");
      audio.src = `${REMOTE}/probe.mp3`;
      audio.load();
      return expectViolation(context, ["media-src", "default-src"]);
    },
  },
  {
    id: "load-image-data",
    async run(context) {
      let loaded = false;
      const image = document.createElement("img");
      image.addEventListener("load", () => (loaded = true));
      image.src = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///ywAAAAAAQABAAACAUwAOw==";
      append(image);
      const verdict = await expectViolation(context, ["img-src"], () => loaded);
      image.remove();
      return verdict;
    },
  },
];

const connections: Case[] = [
  {
    id: "connect-fetch",
    async run(context) {
      await fetch(`${REMOTE}/`).catch(() => undefined);
      return expectViolation(context, ["connect-src"]);
    },
  },
  {
    id: "connect-xhr",
    async run(context) {
      try {
        const request = new XMLHttpRequest();
        request.open("GET", `${REMOTE}/`);
        request.send();
      } catch {
        // A refusal may throw; the violation decides.
      }
      return expectViolation(context, ["connect-src"]);
    },
  },
  {
    id: "connect-websocket",
    async run(context) {
      try {
        new WebSocket("wss://probe.invalid/").close();
      } catch {
        // A refusal may throw; the violation decides.
      }
      return expectViolation(context, ["connect-src"]);
    },
  },
  {
    id: "connect-eventsource",
    async run(context) {
      try {
        new EventSource(`${REMOTE}/`).close();
      } catch {
        // A refusal may throw; the violation decides.
      }
      return expectViolation(context, ["connect-src"]);
    },
  },
];

const embedding: Case[] = [
  {
    id: "embed-iframe",
    async run(context) {
      const frame = document.createElement("iframe");
      frame.src = `${REMOTE}/`;
      append(frame);
      const verdict = await expectViolation(context, ["frame-src", "child-src"]);
      frame.remove();
      return verdict;
    },
  },
  {
    id: "embed-object",
    async run(context) {
      const object = document.createElement("object");
      object.data = `${REMOTE}/probe.swf`;
      append(object);
      const verdict = await expectViolation(context, ["object-src"]);
      object.remove();
      return verdict;
    },
  },
  {
    id: "worker-blob",
    async run(context) {
      const url = URL.createObjectURL(
        new Blob(["self.postMessage(1);"], { type: "text/javascript" }),
      );
      try {
        new Worker(url).terminate();
      } catch {
        // A refusal may throw; the violation decides.
      }
      const verdict = await expectViolation(context, ["worker-src", "child-src", "script-src"]);
      URL.revokeObjectURL(url);
      return verdict;
    },
  },
  {
    id: "base-element",
    async run(context) {
      const base = document.createElement("base");
      base.href = `${REMOTE}/`;
      document.head.prepend(base);
      // Measured before the removal: a base that applied would move relative URLs off the app.
      const applied = document.baseURI.startsWith(REMOTE);
      base.remove();
      return expectViolation(context, ["base-uri"], () => applied);
    },
  },
  {
    id: "form-remote-submit",
    mayLeave: true,
    async run(context) {
      const form = document.createElement("form");
      form.method = "post";
      form.action = `${REMOTE}/submit`;
      append(form);
      form.submit();
      const verdict = await expectViolation(context, ["form-action"]);
      form.remove();
      if (!(await context.stayed())) {
        return { held: false, detail: `${verdict.detail}; the page left` };
      }
      return verdict;
    },
  },
];

/** Waits up to `ms` for `find` to return an element. */
async function waitFor(
  context: Context,
  find: () => Element | null,
  ms = 2000,
): Promise<Element | null> {
  for (let waited = 0; waited < ms; waited += 50) {
    const found = find();
    if (found) return found;
    await context.wait(50);
  }
  return find();
}

function pressEscape(): void {
  document.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
}

/**
 * The demo's dialog and select, opened and closed, so any `<style>` element a component inserts
 * at run time shows as a violation (FR-002).
 */
const runtimeStyle: Case = {
  id: "style-runtime-element",
  async run(context) {
    const trigger = await waitFor(context, () =>
      [...document.querySelectorAll("button")].find(
        (button) => button.textContent?.trim() === "Novo relatório",
      ) ?? null,
    );
    if (!(trigger instanceof HTMLElement)) {
      return { held: false, detail: "the demo's dialog button was not found" };
    }
    trigger.click();
    const dialog = await waitFor(context, () => document.querySelector('[role="dialog"]'));
    pressEscape();

    const select = document.getElementById("plan");
    select?.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    const listbox = await waitFor(context, () => document.querySelector('[role="listbox"]'));
    pressEscape();
    await context.wait(300);

    if (!dialog || !listbox) {
      return {
        held: false,
        detail: `the demo's ${dialog ? "select" : "dialog"} did not open`,
      };
    }
    const seen = violation(context, "style-src");
    if (seen && !STYLE_ALLOWANCE) {
      return {
        held: false,
        detail: `violation ${seen}: a component inserts a style element, and no allowance is recorded`,
      };
    }
    return { held: true, detail: "dialog and select opened and closed with no style violation" };
  },
};

export const policyCases: Case[] = [
  ...injected,
  ...loads,
  ...connections,
  ...embedding,
  runtimeStyle,
];
