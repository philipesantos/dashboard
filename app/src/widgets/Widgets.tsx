// The widgets the dashboard shows, each in its frame (specs/009-declarative-widget-ui, US1).

import { useEffect, useState } from "react";

import { listWidgets, type FrameInfo } from "./api";
import { Frame } from "./Frame";

export function Widgets() {
  const [frames, setFrames] = useState<FrameInfo[] | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    listWidgets()
      .then(setFrames)
      .catch(() => setFailed(true));
  }, []);

  if (failed) {
    return <p className="text-sm text-destructive">Os widgets não puderam ser carregados.</p>;
  }
  if (frames === null) return null;
  return (
    <section aria-label="Widgets" className="grid gap-4 md:grid-cols-2">
      {frames.map((frame) => (
        <Frame key={frame.handle} frame={frame} />
      ))}
    </section>
  );
}
