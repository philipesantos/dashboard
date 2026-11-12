// A widget's frame (specs/009-declarative-widget-ui/research.md, R10; criteria HS-03, HS-04,
// HS-16). The header is the host's: the plugin's name and icon, drawn from the manifest, and the
// host's reload button. The body holds the content and contains it: it paints nothing outside
// itself, scrolls past a fixed height, and starts its own stacking context, so nothing of the
// content reaches the header or the rest of the window.
//
// Part of the security core (constitution, Principle IX).

import { useCallback, useEffect, useRef, useState } from "react";
import { CircleX, Info, LoaderCircle, RotateCw } from "lucide-react";

import { actWidget, renderWidget, type Content as ContentTree, type Failure, type FrameInfo, type Rendered } from "./api";
import { ActionContext } from "./content/action";
import { Content } from "./content/Content";
import { Button } from "@/components/ui/button";

type View =
  | { state: "loading" }
  | { state: "content"; content: ContentTree }
  | { state: "error"; failure: Failure }
  | { state: "not-loaded"; failure: Failure };

/** What the host tells the person above the content: an action that failed or was refused. */
type Notice = { tone: "error" | "info"; text: string };

/** The host's words for each refusal of `widget_act`: nothing was sent to the widget. */
const REFUSALS: Record<string, Notice> = {
  "widget-busy": { tone: "info", text: "O widget ainda está trabalhando. Tente de novo em instantes." },
  "secret-in-form": {
    tone: "error",
    text: "Um campo tem uma chave ou um token que o app guarda, ou o app não conseguiu conferir. Nada foi enviado.",
  },
  "bad-values": { tone: "error", text: "O formulário passou dos limites do app. Nada foi enviado." },
  "undeclared-action": { tone: "error", text: "Este widget não tem essa ação. Nada foi enviado." },
};
const REFUSED: Notice = { tone: "error", text: "O app recusou o pedido. Nada foi enviado." };

export function Frame({ frame }: { frame: FrameInfo }) {
  const [view, setView] = useState<View>({ state: "loading" });
  /** The control whose action is running, or "render" while the host renders. */
  const [running, setRunning] = useState<string | null>(null);
  /** Whether a call to the host is in flight: one at a time, as the host runs them. */
  const busy = useRef(false);
  /** Why the last action failed or was refused, shown above the content it kept. */
  const [notice, setNotice] = useState<Notice | null>(null);

  const show = useCallback((rendered: Rendered) => {
    if ("content" in rendered) setView({ state: "content", content: rendered.content });
    else if ("error" in rendered) {
      const failure = rendered.error;
      setView({ state: failure.kind === "not-loaded" ? "not-loaded" : "error", failure });
    }
    // A refusal (the widget was busy) leaves what is drawn as it is
  }, []);

  const reload = useCallback(() => {
    // A render while another call runs would be refused as busy: the spinner already shows it
    if (busy.current) return;
    busy.current = true;
    setRunning("render");
    renderWidget(frame.handle)
      .then(show)
      .catch(() => setView({ state: "error", failure: { kind: "host-stopped", reason: "" } }))
      .finally(() => {
        busy.current = false;
        setRunning(null);
      });
  }, [frame.handle, show]);

  useEffect(reload, [reload]);

  // The first content drawn, for the probe's time check (SC-001)
  const drawn = useRef(false);
  useEffect(() => {
    if (view.state !== "content" || drawn.current) return;
    drawn.current = true;
    performance.mark(`widget-drawn-${frame.handle}`);
  }, [view.state, frame.handle]);

  const act = useCallback(
    (action: string, values: Record<string, string>, control: string) => {
      // A second press while a call runs is not sent, and the spinner stays with the first
      if (busy.current) {
        setNotice(REFUSALS["widget-busy"]);
        return;
      }
      busy.current = true;
      setRunning(control);
      actWidget(frame.handle, action, values)
        .then((rendered) => {
          if ("error" in rendered && rendered.error.kind === "action-failed") {
            setNotice({ tone: "error", text: failed(rendered.error) });
          } else if ("refused" in rendered) {
            setNotice(REFUSALS[rendered.refused] ?? REFUSED);
          } else {
            // A press made while this action ran was not sent: that notice stays true
            setNotice((shown) => (shown === REFUSALS["widget-busy"] ? shown : null));
            show(rendered);
          }
        })
        .catch(() => setNotice({ tone: "error", text: failed({ kind: "host-stopped", reason: "" }) }))
        .finally(() => {
          busy.current = false;
          setRunning(null);
        });
    },
    [frame.handle, show],
  );

  return (
    <section
      aria-labelledby={`${frame.handle}-name`}
      aria-busy={running !== null}
      data-widget={frame.handle}
      data-view={view.state}
      className="overflow-hidden rounded-xl border border-frame-border"
    >
      <header className="flex items-center gap-2 bg-frame-header px-3 py-2 text-frame-header-foreground">
        <Icon frame={frame} />
        <h2 id={`${frame.handle}-name`} className="min-w-0 flex-1 truncate text-sm font-medium">
          <bdi>{frame.name}</bdi>
          {frame.id && (
            <span className="ms-2 font-normal text-muted-foreground">
              <bdi>{frame.id}</bdi>
            </span>
          )}
        </h2>
        {running !== null && <LoaderCircle aria-hidden className="size-4 animate-spin text-muted-foreground" />}
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-label={`Recarregar ${frame.name}`}
          onClick={reload}
          className="text-frame-header-foreground"
        >
          <RotateCw aria-hidden />
        </Button>
      </header>
      <div
        data-frame-body
        className="isolate max-h-[28rem] overflow-auto bg-frame p-4 text-frame-foreground contain-layout contain-paint"
      >
        {notice && (
          <p
            role="status"
            data-notice={notice.tone}
            className={`mb-3 flex items-start gap-1.5 text-sm ${notice.tone === "error" ? "text-destructive" : "text-muted-foreground"}`}
          >
            {notice.tone === "error" ? (
              <CircleX aria-hidden className="mt-0.5 size-4 shrink-0" />
            ) : (
              <Info aria-hidden className="mt-0.5 size-4 shrink-0" />
            )}
            <span className="min-w-0 break-words">{notice.text}</span>
          </p>
        )}
        {body()}
      </div>
    </section>
  );

  function body() {
    switch (view.state) {
      case "loading":
        return <p className="text-sm text-muted-foreground">Carregando…</p>;
      case "content":
        return (
          <ActionContext.Provider value={{ act, running }}>
            <Content content={view.content} />
          </ActionContext.Provider>
        );
      case "error":
        return <Problem message="Este widget não pôde ser desenhado." failure={view.failure} />;
      case "not-loaded":
        return <Problem message="Este widget não foi carregado." failure={view.failure} />;
    }
  }
}

/** The host's words for an action that failed, with the host's reason. */
function failed(failure: Failure): string {
  return `A ação não deu certo (${failure.reason || failure.kind}).`;
}

/**
 * The host's error state (FR-005): its own message, then the failure's kind, where in the content
 * it is and the host's reason, each as text. Nothing here comes from the plugin's own error text.
 */
function Problem({ message, failure }: { message: string; failure: Failure }) {
  return (
    <div role="status" className="flex items-start gap-2 text-sm">
      <CircleX aria-hidden className="mt-0.5 size-4 shrink-0 text-destructive" />
      <div className="flex min-w-0 flex-col gap-1">
        <p className="text-frame-foreground">{message}</p>
        <p className="break-words text-xs text-muted-foreground">
          <bdi>{failure.kind}</bdi>
          {failure.path && (
            <>
              {" em "}
              <bdi>{failure.path}</bdi>
            </>
          )}
          {failure.reason && (
            <>
              {": "}
              <bdi>{failure.reason}</bdi>
            </>
          )}
        </p>
      </div>
    </div>
  );
}

/** The manifest's icon, decoded by the host, drawn as pixels; or the host's monogram. */
function Icon({ frame }: { frame: FrameInfo }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const icon = frame.icon;
  useEffect(() => {
    const context = canvas.current?.getContext("2d");
    if (!icon || !context || icon.rgba.length !== icon.size * icon.size * 4) return;
    context.putImageData(new ImageData(new Uint8ClampedArray(icon.rgba), icon.size, icon.size), 0, 0);
  }, [icon]);
  if (!icon) {
    return (
      <span aria-hidden className="flex size-5 items-center justify-center rounded-sm border border-frame-border text-xs font-semibold">
        {Array.from(frame.name)[0]?.toUpperCase()}
      </span>
    );
  }
  return <canvas ref={canvas} width={icon.size} height={icon.size} aria-hidden className="size-5 rounded-sm" />;
}
