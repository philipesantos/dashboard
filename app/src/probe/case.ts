// The probe's shared types and helpers (specs/008-tauri-shell/contracts/probe.md).
// Part of the security core (constitution, Principle IX).

/** The window handlers' refusals so far, as the host counts them. */
export interface Counters {
  navigation: number;
  newWindow: number;
  permission: number;
  download: number;
  /** Widget actions the host ran (feature 009), so a case can tell a refusal ran nothing. */
  actions: number;
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
  /**
   * The action may unload or block the page if it is not refused, so it runs after the others.
   * Either way the report never arrives, and the host fails the run.
   */
  mayLeave?: boolean;
  run(context: Context): Promise<Verdict>;
}

/**
 * Whether the page runs in WebView2: the app is served from `http://tauri.localhost` there, and
 * from `tauri://localhost` on macOS and Linux. Engine behaviour that differs by system is
 * decided by this, never by the browser's identity string.
 */
export const ON_WEBVIEW2 = location.protocol === "http:";

/** The other system's form of the app's origin, which this system must refuse. */
export const OTHER_APP_ORIGIN = ON_WEBVIEW2 ? "tauri://localhost" : "http://tauri.localhost";

/** A host that never resolves, so the network can never be what refused a load. */
export const REMOTE = "https://probe.invalid";

/**
 * How long a case waits for the engine to report a refusal. Engines report some refusals late,
 * when a load they deferred starts (WKWebView's media, after a first CI run held and a second
 * did not), so a case polls for it up to this long, and a case that holds returns early.
 */
export const SETTLE_MS = 2000;
const POLL_MS = 50;

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
  let seen = violation(context, ...directives);
  for (let waited = 0; !seen && waited < SETTLE_MS; waited += POLL_MS) {
    await context.wait(POLL_MS);
    seen = violation(context, ...directives);
  }
  // A side effect lands in the same task as the action or never, but give it a moment
  if (sideEffect) await context.wait(POLL_MS);
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
