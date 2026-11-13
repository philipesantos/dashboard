// The widgets the dashboard shows, each in its frame (specs/009-declarative-widget-ui, US1).

import { useEffect, useState } from "react";

import { listWidgets, type FrameInfo } from "./api";
import { Frame } from "./Frame";

/** `version` changes when the host's list did, as after an installation: the list is read again. */
export function Widgets({ version = 0 }: { version?: number }) {
  const [frames, setFrames] = useState<FrameInfo[] | null>(null);
  const [failed, setFailed] = useState(false);
  /** Bumped when a frame's update was approved. */
  const [updated, setUpdated] = useState(0);

  useEffect(() => {
    listWidgets()
      .then(setFrames)
      .catch(() => setFailed(true));
  }, [version, updated]);

  if (failed) {
    return <p className="text-sm text-destructive">Os widgets não puderam ser carregados.</p>;
  }
  if (frames === null) return null;
  return (
    <section aria-label="Widgets" className="grid gap-4 md:grid-cols-2">
      {frames.map((frame) => (
        <Frame
          // A widget updated in place keeps its handle: its key changes, so its frame starts over
          key={`${frame.handle}-${updated}`}
          frame={frame}
          onChanged={() => setUpdated((count) => count + 1)}
        />
      ))}
    </section>
  );
}
