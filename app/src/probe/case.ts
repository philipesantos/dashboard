// The probe's shared types and helpers (specs/008-tauri-shell/contracts/probe.md).
// Part of the security core (constitution, Principle IX).

/** The window handlers' refusals so far, as the host counts them. */
export interface Counters {
  navigation: number;
  newWindow: number;
  permission: number;
  download: number;
}

/** A case's verdict, as sent to the host. */
export interface Verdict {
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
  /** The action may unload the page if it is not refused, so it runs after the others. */
  mayLeave?: boolean;
  run(context: Context): Promise<Verdict>;
}

/** A host that never resolves, so the network can never be what refused a load. */
export const REMOTE = "https://probe.invalid";

/** How long a case waits for the engine to report a refusal. */
export const SETTLE_MS = 300;

/**
 * The first violation of one of `directives`, matched as a prefix of the effective directive,
 * so "script-src" also matches "script-src-elem" and "script-src-attr".
 */
export function violation(context: Context, ...directives: string[]): string | undefined {
  return context
    .violations()
    .find((entry) => directives.some((directive) => entry.startsWith(directive)));
}

/** Held when the engine reported a violation of one of `directives`. */
export async function expectViolation(
  context: Context,
  directives: string[],
  sideEffect?: () => boolean,
): Promise<Verdict> {
  await context.wait(SETTLE_MS);
  const seen = violation(context, ...directives);
  const happened = sideEffect?.() ?? false;
  if (seen && !happened) {
    return {
      held: true,
      detail: `violation ${seen}${sideEffect ? ", side effect absent" : ""}`,
    };
  }
  const parts = [seen ? `violation ${seen}` : `no ${directives.join(" or ")} violation`];
  if (happened) parts.push("the side effect happened");
  return { held: false, detail: parts.join("; ") };
}

/** Held when `action` threw. */
export function expectThrow(action: () => unknown): Verdict {
  try {
    action();
  } catch (error) {
    const reason = error instanceof Error ? `${error.name}: ${error.message}` : String(error);
    return { held: true, detail: `refused: ${reason.split("\n")[0].slice(0, 120)}` };
  }
  return { held: false, detail: "the call ran" };
}

/** A flag on `window` that injected script would set. */
export function flag(name: string): () => boolean {
  return () => (window as unknown as Record<string, unknown>)[name] !== undefined;
}
