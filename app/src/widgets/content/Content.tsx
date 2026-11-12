// Draws normalised widget content with the host's own components
// (specs/009-declarative-widget-ui/contracts/content-format.md, research R10). Every string is a
// text child inside <bdi>, never markup; no attribute takes a URL from content; nothing is
// positioned outside the flow, so the frame's body contains it all.
//
// Part of the security core (constitution, Principle IX).

import { useContext, useId } from "react";

import type { Component, Content as ContentTree } from "../api";
import { ActionContext } from "./action";
import { Chart } from "./Chart";
import { Form } from "./Form";
import { Button } from "@/components/ui/button";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import { buttonVariant, StatusIcon, textClass, toneClass } from "./styles";
import { LoaderCircle } from "lucide-react";

export function Content({ content }: { content: ContentTree }) {
  return (
    <div className="flex flex-col gap-3">
      {content.body.map((component, index) => (
        <Part key={index} component={component} />
      ))}
    </div>
  );
}

function Part({ component }: { component: Component }) {
  switch (component.type) {
    case "text":
      return (
        <p
          data-tone={component.tone.name}
          className={`flex items-start gap-1.5 ${textClass(component.tone, component.variant)}`}
        >
          <StatusIcon tone={component.tone} />
          <bdi className="min-w-0 break-words">{component.text}</bdi>
        </p>
      );
    case "list":
      return (
        <ul className="divide-y divide-border text-sm">
          {component.items.map((item, index) => (
            <li
              key={index}
              data-tone={item.tone.name}
              className={`flex items-start gap-1.5 py-1.5 ${toneClass[item.tone.name] ?? toneClass.default}`}
            >
              <StatusIcon tone={item.tone} />
              <bdi className="min-w-0 break-words">{item.text}</bdi>
            </li>
          ))}
        </ul>
      );
    case "table":
      return (
        <Table>
          <TableHeader>
            <TableRow>
              {component.columns.map((column, index) => (
                <TableHead key={index} className="text-muted-foreground">
                  <bdi>{column}</bdi>
                </TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {component.rows.map((row, index) => (
              <TableRow key={index}>
                {row.map((cell, column) => (
                  <TableCell key={column} className="whitespace-normal">
                    <bdi>{cell}</bdi>
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      );
    case "chart":
      return <Chart chart={component} />;
    case "button":
      return <ActionButton label={component.label} action={component.action} variant={component.variant} />;
    case "form":
      // A form of another shape at the same place starts empty, keeping nothing typed for the
      // one before
      return <Form key={formKey(component)} form={component} />;
    case "stack":
      return (
        <div className="flex flex-wrap items-start gap-2">
          {component.children.map((child, index) => (
            <Part key={index} component={child} />
          ))}
        </div>
      );
    case "placeholder":
      return <Placeholder />;
    default:
      return <Placeholder />;
  }
}

/** What tells one form from another: its action and each field's id and kind. */
function formKey(form: Extract<Component, { type: "form" }>): string {
  return JSON.stringify([form.submit.action, form.fields.map((field) => [field.id, field.kind])]);
}

/** A component this host does not draw: the host's placeholder, which names nothing of it. */
function Placeholder() {
  return (
    <p className="rounded-lg border border-dashed border-border px-2 py-1.5 text-sm text-muted-foreground">
      Este componente não existe nesta versão do app.
    </p>
  );
}

function ActionButton({ label, action, variant }: { label: string; action: string; variant: string }) {
  const { act, running } = useContext(ActionContext);
  const control = useId();
  const working = running === control;
  return (
    <Button
      type="button"
      variant={buttonVariant[variant] ?? "default"}
      aria-busy={working}
      onClick={() => act(action, {}, control)}
    >
      {working && <LoaderCircle aria-hidden className="animate-spin" />}
      <bdi>{label}</bdi>
    </Button>
  );
}
