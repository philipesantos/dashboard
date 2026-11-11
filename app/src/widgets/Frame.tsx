// A widget's frame (specs/009-declarative-widget-ui/research.md, R10; criteria HS-03, HS-04,
// HS-16). The header is the host's: the plugin's name and icon, drawn from the manifest, and the
// host's reload button. The body holds the content and contains it: it paints nothing outside
// itself, scrolls past a fixed height, and starts its own stacking context, so nothing of the
// content reaches the header or the rest of the window.
//
// Part of the security core (constitution, Principle IX).

import { useCallback, useEffect, useRef, useState } from "react";
import { CircleX, LoaderCircle, RotateCw } from "lucide-react";

import { actWidget, renderWidget, type Content as ContentTree, type Failure, type FrameInfo, type Rendered } from "./api";
import { ActionContext } from "./content/action";
import { Content } from "./content/Content";
import { Button } from "@/components/ui/button";

type View =
  | { state: "loading" }
  | { state: "content"; content: ContentTree }
  | { state: "error"; failure: Failure }
  | { state: "not-loaded"; failure: Failure };

export function Frame({ frame }: { frame: FrameInfo }) {
  const [view, setView] = useState<View>({ state: "loading" });
  /** The control whose action is running, or "render" while the host renders. */
  const [running, setRunning] = useState<string | null>(null);
  /** Why the last action failed, shown above the content it kept. */
  const [actionFailure, setActionFailure] = useState<Failure | null>(null);

  const show = useCallback((rendered: Rendered) => {
    if ("content" in rendered) setView({ state: "content", content: rendered.content });
    else if ("error" in rendered) {
      const failure = rendered.error;
      setView({ state: failure.kind === "not-loaded" ? "not-loaded" : "error", failure });
    }
    // A refusal (the widget was busy) leaves what is drawn as it is
  }, []);

  const reload = useCallback(() => {
    setRunning("render");
    renderWidget(frame.handle)
      .then(show)
      .catch(() => setView({ state: "error", failure: { kind: "host-stopped", reason: "" } }))
      .finally(() => setRunning(null));
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
      setRunning(control);
      actWidget(frame.handle, action, values)
        .then((rendered) => {
          if ("error" in rendered && rendered.error.kind === "action-failed") {
            setActionFailure(rendered.error);
          } else {
            setActionFailure(null);
            show(rendered);
          }
        })
        .catch(() => setActionFailure({ kind: "host-stopped", reason: "" }))
        .finally(() => setRunning(null));
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
        {actionFailure && (
          <p role="status" className="mb-3 text-sm text-destructive">
            A ação não deu certo ({actionFailure.reason || actionFailure.kind}).
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
