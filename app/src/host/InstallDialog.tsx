// The installation dialog (specs/010-plugin-installation/, US1, US2; research R6, R7): the host's
// model of a release, drawn as it is. Every sentence, name, destination and warning comes from the
// host; every string is drawn as text. The page sends back only the request's token, the choice
// digest the host last returned, and the optionals the person turned on, so it cannot approve
// anything other than what it shows.
//
// Part of the security core (constitution v2.7.0, Principle IX).

import { useState, type RefObject } from "react";
import { TriangleAlert } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Switch } from "@/components/ui/switch";

import {
  requestApprove,
  requestCancel,
  requestJudge,
  type Change,
  type ConnectorLine,
  type DialogModel,
  type Flow,
} from "./api";
import { HostDialog } from "./HostDialog";

/** The host's words for each way an approval can fail. */
function approvalMessage(kind: string, reason: string): string {
  if (kind === "approval-changed") return "O que seria instalado mudou desde que este diálogo abriu. Feche e tente de novo.";
  if (kind === "unknown-request") return "Este pedido já terminou. Feche e tente de novo.";
  return `A instalação não deu certo (${kind}): ${reason}`;
}

const CHANGE: Record<Change, string | null> = { added: "novo", changed: "mudou", unchanged: null };

function ChangeBadge({ change }: { change: Change | null }) {
  const text = change ? CHANGE[change] : null;
  return text ? <Badge variant="secondary">{text}</Badge> : null;
}

function warningText(flow: Flow): string {
  const reach = flow.destinations.join(", ");
  if (flow.source.kind === "account") {
    return `Dados lidos como a sua conta em ${flow.source.connector} podem chegar a ${reach}.`;
  }
  return `Dados deste computador (${flow.source.origins.join(", ")}) podem chegar a ${reach}.`;
}

export function InstallDialog({
  token,
  model,
  opener,
  onClosed,
}: {
  token: string;
  model: DialogModel;
  opener: RefObject<HTMLElement | null>;
  onClosed: (installed: boolean) => void;
}) {
  const [optionalOn, setOptionalOn] = useState<string[]>([]);
  const [warning, setWarning] = useState<Flow[]>(model.warning);
  const [choice, setChoice] = useState(model.choice);
  const [working, setWorking] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);

  const toggle = (name: string, on: boolean) => {
    const next = on ? [...optionalOn, name] : optionalOn.filter((each) => each !== name);
    setOptionalOn(next);
    setWorking(true);
    // The host judges every set: the warning shown is always the host's for this set
    requestJudge(token, next)
      .then((judged) => {
        if ("refused" in judged) setProblem(approvalMessage(judged.refused, ""));
        else {
          setWarning(judged.warning);
          setChoice(judged.choice);
        }
      })
      .catch(() => setProblem("O app não respondeu."))
      .finally(() => setWorking(false));
  };

  const cancel = () => {
    void requestCancel(token).catch(() => undefined);
    onClosed(false);
  };

  const approve = () => {
    setWorking(true);
    requestApprove(token, choice, optionalOn)
      .then((approved) => {
        if ("approved" in approved) onClosed(true);
        else if ("refused" in approved) setProblem(approvalMessage(approved.refused, ""));
        else setProblem(approvalMessage(approved.error.kind, approved.error.reason));
      })
      .catch(() => setProblem("O app não respondeu."))
      .finally(() => setWorking(false));
  };

  const { asking } = model;
  const update = model.kind !== "install";
  const title =
    model.kind === "install"
      ? `Instalar ${asking.name}?`
      : model.kind === "update"
        ? `Atualizar ${asking.name}?`
        : `Atualizar ${asking.name} para ${asking.version}?`;

  return (
    <HostDialog
      open
      title={title}
      safeLabel="Cancelar"
      approveLabel={model.kind === "install" ? "Instalar" : "Atualizar"}
      approveDisabled={working}
      opener={opener}
      onSafe={cancel}
      onApprove={approve}
      description={
        <p data-asking>
          <bdi>{asking.name}</bdi> (<bdi data-asking-id>{asking.id}</bdi>), versão <bdi>{asking.version}</bdi>
          {update && asking.installed_version && (
            <>
              {" "}
              (instalada: <bdi>{asking.installed_version}</bdi>)
            </>
          )}
          . Autor, como o plugin declara: <bdi data-asking-author>{asking.author}</bdi>.
        </p>
      }
    >
      <div data-dialog-body className="space-y-4 text-sm">
        {warning.length > 0 && (
          <div role="alert" data-warning className="flex gap-2 rounded-md border border-destructive p-3 text-destructive">
            <TriangleAlert aria-hidden className="mt-0.5 size-4 shrink-0" />
            <div className="space-y-1">
              <p className="font-medium">Atenção: esta combinação deixa dados saírem.</p>
              {warning.map((flow, index) => (
                <p key={index}>{warningText(flow)}</p>
              ))}
            </div>
          </div>
        )}
        {model.kind === "update-confirm" ? (
          <p>Esta versão não pede nada novo: as permissões e os conectores continuam os mesmos.</p>
        ) : (
          <>
            {model.permissions.length > 0 && (
              <section aria-label="Permissões" className="space-y-2">
                <h3 className="font-medium">Permissões</h3>
                <ul className="space-y-2">
                  {model.permissions.map((line) => (
                    <li key={line.name} data-permission={line.name} className="flex items-start justify-between gap-3">
                      <span className="min-w-0">
                        {line.sentence} <ChangeBadge change={line.change} />
                      </span>
                      <Optional
                        name={line.name}
                        required={line.required}
                        on={optionalOn.includes(line.name)}
                        disabled={working}
                        onChange={toggle}
                      />
                    </li>
                  ))}
                </ul>
              </section>
            )}
            {model.connectors.length > 0 && (
              <section aria-label="Conectores" className="space-y-3">
                <h3 className="font-medium">Conectores</h3>
                {model.connectors.map((connector) => (
                  <Connector
                    key={connector.id}
                    connector={connector}
                    on={optionalOn.includes(connector.id)}
                    disabled={working}
                    onChange={toggle}
                  />
                ))}
              </section>
            )}
          </>
        )}
        {problem && (
          <p role="alert" data-problem className="text-destructive">
            {problem}
          </p>
        )}
      </div>
    </HostDialog>
  );
}

function Optional({
  name,
  required,
  on,
  disabled,
  onChange,
}: {
  name: string;
  required: boolean;
  on: boolean;
  disabled: boolean;
  onChange: (name: string, on: boolean) => void;
}) {
  if (required) return <Badge variant="outline">obrigatória</Badge>;
  return (
    <label className="flex shrink-0 items-center gap-2 text-xs text-muted-foreground">
      opcional
      <Switch
        data-optional={name}
        checked={on}
        disabled={disabled}
        onCheckedChange={(checked) => onChange(name, checked)}
        aria-label={`Ligar ${name}`}
      />
    </label>
  );
}

function Connector({
  connector,
  on,
  disabled,
  onChange,
}: {
  connector: ConnectorLine;
  on: boolean;
  disabled: boolean;
  onChange: (name: string, on: boolean) => void;
}) {
  return (
    <div data-connector={connector.id} className="space-y-1 rounded-md border p-3">
      <div className="flex items-start justify-between gap-3">
        <p className="min-w-0">
          {connector.sentence} <ChangeBadge change={connector.change} />
        </p>
        <Optional name={connector.id} required={connector.required} on={on} disabled={disabled} onChange={onChange} />
      </div>
      <p className="text-xs text-muted-foreground">
        <bdi>{connector.id}</bdi> {connector.version}, do catálogo oficial. Destinos:{" "}
        <bdi data-destinations>{connector.destinations.join(", ")}</bdi>
      </p>
      {connector.account && <p className="text-xs">{connector.account.sentence}</p>}
      <ul className="space-y-1 text-xs text-muted-foreground">
        {connector.operations.map((operation) => (
          <li key={operation.name} data-operation={operation.name}>
            <bdi>{operation.name}</bdi>: {operation.kind === "read" ? "leitura" : "alteração"} (
            {operation.review === "reviewed" ? "revisado no catálogo" : "declarado pelo dono do serviço"}) {operation.method}{" "}
            <bdi>{operation.url}</bdi>; envia: {operation.sends.length > 0 ? operation.sends.join(", ") : "nada"}
            {operation.fixed.length > 0 && <>; fixo: {operation.fixed.join(", ")}</>}
          </li>
        ))}
      </ul>
    </div>
  );
}
