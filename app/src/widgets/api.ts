// The widget host's three commands, typed
// (specs/009-declarative-widget-ui/contracts/window-commands.md). What they return was checked
// and normalised by the host: the page never gets a plugin's raw output.
//
// Part of the security core (constitution, Principle IX).

import { invoke } from "@tauri-apps/api/core";

export type Tone = { name: string; icon?: string };

export type Item = { text: string; tone: Tone };

export type Series = { name: string; values: number[] };

export type Field = {
  id: string;
  label: string;
  kind: "text" | "number" | "checkbox" | "select";
  options?: string[];
};

export type Component =
  | { type: "text"; text: string; variant: string; tone: Tone }
  | { type: "list"; items: Item[] }
  | { type: "table"; columns: string[]; rows: string[][] }
  | { type: "chart"; kind: string; labels: string[]; series: Series[] }
  | { type: "button"; label: string; action: string; variant: string }
  | { type: "form"; fields: Field[]; submit: { label: string; action: string } }
  | { type: "stack"; children: Component[] }
  | { type: "placeholder"; name: string };

export type Content = { body: Component[] };

export type Failure = { kind: string; reason: string; path?: string };

export type Rendered = { content: Content } | { error: Failure } | { refused: string };

export type FrameInfo = {
  handle: string;
  name: string;
  icon: { size: number; rgba: number[] } | null;
  /** The plugin's id, only when another frame draws the same name. */
  id?: string;
  /** For a widget installed from a repository, its owner (`github.com/<owner>`), from the host's
   * installation record (criterion HS-02); absent for a bundled widget. */
  source?: string;
};

export function listWidgets(): Promise<FrameInfo[]> {
  return invoke<FrameInfo[]>("widgets_list");
}

export function renderWidget(handle: string): Promise<Rendered> {
  return invoke<Rendered>("widget_render", { handle });
}

export function actWidget(
  handle: string,
  action: string,
  values: Record<string, string>,
): Promise<Rendered> {
  return invoke<Rendered>("widget_act", { handle, action, values });
}
