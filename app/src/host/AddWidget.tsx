// The chrome's control for installing a widget (specs/010-plugin-installation/, US1; FR-002): an
// address field and a button, drawn by the host outside every frame. Only this control and a
// frame's update control open an installation; widget content cannot reach either. While the host
// fetches and compiles, the chrome says so; then the host dialog shows what is decided.
//
// Part of the security core (constitution v2.7.0, Principle IX).

import { useRef, useState, type FormEvent } from "react";
import { LoaderCircle, Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

import { installStart, type DialogModel, type Started } from "./api";
import { InstallDialog } from "./InstallDialog";

/** The host's words for each refusal of `install_start`. */
const REFUSALS: Record<string, string> = {
  "bad-address": "Digite o endereço de um repositório do GitHub, como github.com/dono/repositorio.",
  "already-installed": "Esse widget já está instalado.",
  "request-pending": "Já há uma instalação em andamento.",
};

/** What the chrome says about a request that did not reach the dialog. */
export function startedMessage(started: Started): string | null {
  if ("refused" in started) return REFUSALS[started.refused] ?? "O app recusou o pedido.";
  if ("error" in started) return `A instalação não deu certo (${started.error.kind}): ${started.error.reason}`;
  if ("none" in started) return "Não há versão mais nova.";
  return null;
}

export function AddWidget({ onInstalled }: { onInstalled: () => void }) {
  const [address, setAddress] = useState("");
  const [working, setWorking] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [request, setRequest] = useState<{ token: string; model: DialogModel } | null>(null);
  /** The control that opened the dialog, where focus returns when it closes (HS-20). */
  const opener = useRef<HTMLButtonElement>(null);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (working) return;
    setWorking(true);
    setMessage(null);
    installStart(address)
      .then((started) => {
        if ("request" in started) setRequest({ token: started.request, model: started.model });
        else setMessage(startedMessage(started));
      })
      .catch(() => setMessage("O app não respondeu."))
      .finally(() => setWorking(false));
  };

  return (
    <div className="flex flex-col items-end gap-1">
      <form onSubmit={submit} className="flex items-center gap-2" aria-label="Adicionar widget">
        <Input
          data-add-address
          value={address}
          onChange={(event) => setAddress(event.target.value)}
          placeholder="github.com/dono/repositorio"
          aria-label="Endereço do repositório do widget"
          maxLength={200}
          autoComplete="off"
          spellCheck={false}
          className="w-64"
        />
        <Button ref={opener} type="submit" data-add-widget disabled={working}>
          {working ? <LoaderCircle aria-hidden className="animate-spin" /> : <Plus aria-hidden />}
          Adicionar widget
        </Button>
      </form>
      <p role="status" data-add-status className="text-xs text-muted-foreground">
        {working ? "Buscando a versão mais recente e compilando…" : message}
      </p>
      {request && (
        <InstallDialog
          token={request.token}
          model={request.model}
          opener={opener}
          onClosed={(installed) => {
            setRequest(null);
            if (installed) {
              setAddress("");
              setMessage("Widget instalado.");
              onInstalled();
            }
          }}
        />
      )}
    </div>
  );
}
