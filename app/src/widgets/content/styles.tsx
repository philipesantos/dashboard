// The classes each tone and variant of widget content draws with, from the widget map
// (design/widget-map.generated.json). A .tsx module, so `pnpm --filter ./design check` scans it:
// no tone or variant can reach a host-only role (specs/009-declarative-widget-ui, research R10).

import { CircleCheck, CircleX, TriangleAlert, type LucideIcon } from "lucide-react";

import type { Tone } from "../api";

/** Text color by tone. A tone the host does not know was drawn as `default` already. */
export const toneClass: Record<string, string> = {
  default: "text-foreground",
  muted: "text-muted-foreground",
  accent: "text-primary",
  ok: "text-ok",
  warning: "text-warning",
  error: "text-destructive",
};

/** Typography by text variant. */
export const textVariantClass: Record<string, string> = {
  default: "text-sm",
  muted: "text-sm",
  heading: "font-heading text-base font-semibold",
};

/** The Button variant each widget button variant draws as. */
export const buttonVariant: Record<string, "default" | "secondary" | "outline" | "ghost" | "destructive"> = {
  primary: "default",
  secondary: "secondary",
  outline: "outline",
  ghost: "ghost",
  destructive: "destructive",
};

/** The status icons the widget map names (criterion HS-08). */
const statusIcons: Record<string, LucideIcon> = {
  "circle-check": CircleCheck,
  "triangle-alert": TriangleAlert,
  "circle-x": CircleX,
};

/** A text's classes: its variant's typography, and its tone's color, or the variant's. */
export function textClass(tone: Tone, variant: string): string {
  const type = textVariantClass[variant] ?? textVariantClass.default;
  const color = tone.name === "default" && variant === "muted" ? "muted" : tone.name;
  return `${type} ${toneClass[color] ?? toneClass.default}`;
}

/** The status icon of a tone, drawn beside its text, or nothing. */
export function StatusIcon({ tone }: { tone: Tone }) {
  const Icon = tone.icon ? statusIcons[tone.icon] : undefined;
  if (!Icon) return null;
  return <Icon aria-hidden className="mt-0.5 size-4 shrink-0" />;
}
