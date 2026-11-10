// The shell probe's page side (spec 008 FR-018, specs/008-tauri-shell/contracts/probe.md).
//
// Bundled only in Vite's `probe` mode, which adds this module to index.html. It runs inside the
// real page of a release build, tries each forbidden action, and reports to the host through
// `shell_probe_report`, the only command the probe build grants. The host adds what the page
// cannot see (window count, engine settings), prints the report and exits.
//
// Part of the security core (constitution, Principle IX).

import { invoke } from "@tauri-apps/api/core";

import type { Case, Context, Counters, Verdict } from "./case";
import { policyCases } from "./policy";

/** Every case. The story tasks add theirs here. Cases that may unload the page run last. */
const cases: Case[] = [...policyCases].sort(
  (first, second) => Number(first.mayLeave ?? false) - Number(second.mayLeave ?? false),
);

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
