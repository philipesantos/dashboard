// The host commands' cases (spec 008 FR-007; contracts/probe.md). The window may call no host
// command, so each call must be refused.
// Part of the security core (constitution, Principle IX).

import { invoke } from "@tauri-apps/api/core";

import type { Case, Verdict } from "./case";

async function refused(command: string, args: Record<string, unknown>): Promise<string | null> {
  try {
    await invoke(command, args);
    return null;
  } catch (error) {
    return String(error);
  }
}

export const commandCases: Case[] = [
  {
    // Registered, and granted by nothing: the ACL must refuse it. A "not found" would mean the
    // case tested nothing.
    id: "command-ungranted",
    async run(): Promise<Verdict> {
      const error = await refused("shell_probe_ungranted", {});
      if (error === null) return { held: false, detail: "the command ran" };
      const byAcl = /not allowed/i.test(error) && !/not found/i.test(error);
      return {
        held: byAcl,
        detail: byAcl ? `refused by the ACL: ${error}` : `refused, but not by the ACL: ${error}`,
      };
    },
  },
  {
    id: "command-opener",
    async run(): Promise<Verdict> {
      const error = await refused("plugin:opener|open_url", { url: "https://probe.invalid/" });
      return error === null
        ? { held: false, detail: "the opener opened a URL" }
        : { held: true, detail: `refused: ${error}` };
    },
  },
  {
    id: "command-core",
    async run(): Promise<Verdict> {
      const error = await refused("plugin:event|listen", {
        event: "probe",
        target: { kind: "Any" },
        handler: 0,
      });
      return error === null
        ? { held: false, detail: "the window could listen to host events" }
        : { held: true, detail: `refused: ${error}` };
    },
  },
];
