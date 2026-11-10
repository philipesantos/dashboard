// The shell probe's page side (spec 008 FR-018, specs/008-tauri-shell/contracts/probe.md).
//
// Bundled only in Vite's `probe` mode, which adds this module to index.html. It runs inside the
// real page of a release build, tries each forbidden action, and reports to the host through
// `shell_probe_report`, the only command the probe build grants. The host adds what the page
// cannot see (window count, engine settings), prints the report and exits.
//
// Part of the security core (constitution, Principle IX).

import { invoke } from "@tauri-apps/api/core";

/** The window handlers' refusals so far, as the host counts them. */
export interface Counters {
  navigation: number;
  newWindow: number;
  permission: number;
  download: number;
}

/** A case's verdict, as sent to the host. */
interface Verdict {
  held: boolean;
  detail: string;
  /** The case holds only if the app still has exactly one window (checked by the host). */
  singleWindow?: boolean;
}

/** What a case can observe while it runs. */
export interface Context {
  /** The `securitypolicyviolation` events since the case began, as "directive blocked-uri". */
  violations(): string[];
  /** The handlers' counters now. Diff two snapshots around the action. */
  counters(): Promise<Counters>;
  /** Waits, so late violations and handler calls arrive. */
  wait(ms: number): Promise<void>;
  /** Waits 1 s, then whether the page is still the app's own page. */
  stayed(): Promise<boolean>;
}

/** One forbidden action and how its refusal is detected. */
export interface Case {
  id: string;
  run(context: Context): Promise<Verdict>;
}

/** Every case. The story tasks add theirs here. */
const cases: Case[] = [];

const start = location.href;
let seen: string[] = [];
document.addEventListener("securitypolicyviolation", (event) => {
  seen.push(`${event.effectiveDirective} ${event.blockedURI}`);
});

const context: Context = {
  violations: () => [...seen],
  counters: () => invoke<Counters>("shell_probe_report"),
  wait: (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  async stayed() {
    await context.wait(1000);
    return location.href === start;
  },
};

async function runAll(): Promise<void> {
  const report: (Verdict & { id: string })[] = [];
  for (const probeCase of cases) {
    seen = [];
    try {
      report.push({ id: probeCase.id, ...(await probeCase.run(context)) });
    } catch (error) {
      report.push({ id: probeCase.id, held: false, detail: `the case threw: ${String(error)}` });
    }
  }
  await invoke("shell_probe_report", { cases: report });
  showHoldForm();
}

/**
 * The form for the manual autofill walk (quickstart §6), shown after the report. The host keeps
 * the window open only with DASHBOARD_PROBE_HOLD=1. Its button removes the form, which engines
 * treat as a submission, since `form-action 'none'` blocks a real one.
 */
function showHoldForm(): void {
  const form = document.createElement("form");
  form.style.cssText =
    "position:fixed;inset:auto 16px 16px auto;display:grid;gap:8px;padding:16px;background:Canvas;color:CanvasText;border:1px solid;z-index:2147483647";
  const label = document.createElement("label");
  label.textContent = "Senha";
  const field = document.createElement("input");
  field.type = "password";
  field.name = "password";
  field.autocomplete = "current-password";
  label.append(document.createElement("br"), field);
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = "Entrar";
  button.addEventListener("click", () => form.remove());
  form.append(label, button);
  document.body.append(form);
}

window.addEventListener("load", () => void runAll(), { once: true });
