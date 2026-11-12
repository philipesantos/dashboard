// A widget's form, drawn by the host (specs/009-declarative-widget-ui/research.md, R10; criteria
// HS-06 and HS-25). The engine validates nothing (`noValidate`, every field `type="text"`), so no
// bubble is drawn outside the page; the host shows errors inline. No field is a password, none
// takes a name or an autocomplete hint from content, and a select is the host's listbox, drawn
// inside the frame's body. Each field's id is the host's; the field's content id is only the key
// of its value.
//
// Part of the security core (constitution, Principle IX).

import { useContext, useId, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { Check, ChevronDown, LoaderCircle } from "lucide-react";

import type { Field } from "../api";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ActionContext } from "./action";

type FormProps = { form: { fields: Field[]; submit: { label: string; action: string } } };

/** A decimal number, with a point or a comma. */
const NUMBER = /^-?\d+(?:[.,]\d+)?$/;

export function Form({ form }: FormProps) {
  const { act, running } = useContext(ActionContext);
  const control = useId();
  const [values, setValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(form.fields.map((field) => [field.id, field.kind === "checkbox" ? "false" : ""])),
  );
  const [invalid, setInvalid] = useState<Set<string>>(new Set());
  const set = (id: string, value: string) => setValues((old) => ({ ...old, [id]: value }));

  const submit = (event: FormEvent) => {
    event.preventDefault();
    // Only this form's fields, each with its value or its empty one
    const sent = Object.fromEntries(
      form.fields.map((field) => [field.id, values[field.id] ?? (field.kind === "checkbox" ? "false" : "")]),
    );
    const wrong = new Set(
      form.fields
        .filter((field) => field.kind === "number" && sent[field.id] !== "" && !NUMBER.test(sent[field.id].trim()))
        .map((field) => field.id),
    );
    setInvalid(wrong);
    if (wrong.size === 0) act(form.submit.action, sent, control);
  };

  const working = running === control;
  return (
    <form noValidate autoComplete="off" onSubmit={submit} className="flex w-full flex-col gap-3">
      {form.fields.map((field, index) => (
        <FieldRow
          key={index}
          field={field}
          value={values[field.id] ?? ""}
          invalid={invalid.has(field.id)}
          onChange={(value) => set(field.id, value)}
        />
      ))}
      <div>
        <Button type="submit" aria-busy={working}>
          {working && <LoaderCircle aria-hidden className="animate-spin" />}
          <bdi>{form.submit.label}</bdi>
        </Button>
      </div>
    </form>
  );
}

type FieldProps = {
  field: Field;
  value: string;
  invalid: boolean;
  onChange: (value: string) => void;
};

function FieldRow({ field, value, invalid, onChange }: FieldProps) {
  const id = useId();
  const error = `${id}-error`;
  if (field.kind === "checkbox") {
    return (
      <div className="flex items-center gap-2 text-sm text-foreground">
        <input
          id={id}
          type="checkbox"
          autoComplete="off"
          checked={value === "true"}
          onChange={(event) => onChange(event.target.checked ? "true" : "false")}
          className="size-4 accent-primary"
        />
        <label htmlFor={id}>
          <bdi>{field.label}</bdi>
        </label>
      </div>
    );
  }
  return (
    <div className="flex flex-col gap-1.5 text-sm text-foreground">
      <label htmlFor={id} id={`${id}-label`}>
        <bdi>{field.label}</bdi>
      </label>
      {field.kind === "select" ? (
        <Listbox id={id} options={field.options ?? []} value={value} onChange={onChange} />
      ) : (
        <Input
          id={id}
          type="text"
          inputMode={field.kind === "number" ? "decimal" : undefined}
          autoComplete="off"
          autoCorrect="off"
          spellCheck={false}
          value={value}
          aria-invalid={invalid || undefined}
          aria-describedby={invalid ? error : undefined}
          onChange={(event) => onChange(event.target.value)}
        />
      )}
      {invalid && (
        <p id={error} className="text-destructive">
          Digite um número, como 3 ou 2,5.
        </p>
      )}
    </div>
  );
}

type ListboxProps = {
  id: string;
  options: string[];
  value: string;
  onChange: (value: string) => void;
};

/** The host's select: a button and, when open, a list of options in the page's flow. */
function Listbox({ id, options, value, onChange }: ListboxProps) {
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(0);
  const list = useRef<HTMLUListElement>(null);
  const listId = `${id}-list`;

  const choose = (index: number) => {
    onChange(options[index]);
    setOpen(false);
    document.getElementById(id)?.focus();
  };
  const openList = () => {
    setActive(Math.max(options.indexOf(value), 0));
    setOpen(true);
    requestAnimationFrame(() => list.current?.focus());
  };
  const keys = (event: KeyboardEvent<HTMLUListElement>) => {
    if (event.key === "ArrowDown") setActive((index) => Math.min(index + 1, options.length - 1));
    else if (event.key === "ArrowUp") setActive((index) => Math.max(index - 1, 0));
    else if (event.key === "Home") setActive(0);
    else if (event.key === "End") setActive(options.length - 1);
    else if (event.key === "Enter" || event.key === " ") choose(active);
    else if (event.key === "Escape") {
      setOpen(false);
      document.getElementById(id)?.focus();
    } else return;
    event.preventDefault();
  };

  return (
    <div className="flex flex-col gap-1">
      <Button
        id={id}
        type="button"
        variant="outline"
        className="justify-between font-normal"
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? listId : undefined}
        onClick={() => (open ? setOpen(false) : openList())}
      >
        <bdi className="truncate">{value === "" ? "Escolha" : value}</bdi>
        <ChevronDown aria-hidden />
      </Button>
      {open && (
        <ul
          ref={list}
          id={listId}
          role="listbox"
          tabIndex={0}
          aria-labelledby={`${id}-label`}
          aria-activedescendant={`${id}-option-${active}`}
          onKeyDown={keys}
          className="max-h-48 overflow-auto rounded-lg border border-input p-1 outline-none focus-visible:ring-3 focus-visible:ring-ring"
        >
          {options.map((option, index) => (
            <li
              key={index}
              id={`${id}-option-${index}`}
              role="option"
              aria-selected={option === value}
              onClick={() => choose(index)}
              onMouseEnter={() => setActive(index)}
              className={`flex cursor-default items-center justify-between gap-2 rounded-md px-2 py-1 ${
                index === active ? "bg-muted" : ""
              }`}
            >
              <bdi className="min-w-0 break-words">{option}</bdi>
              {option === value && <Check aria-hidden className="size-4" />}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
