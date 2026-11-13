// The installation's five commands, typed
// (specs/010-plugin-installation/contracts/window-commands.md). The dialog's model is built by
// the host: every sentence, name and warning the page draws comes from it, never from a plugin
// directly, and the page sends back only the request's token, the choice digest the host last
// returned, and the optionals the person turned on.
//
// Part of the security core (constitution v2.7.0, Principle IX).

import { invoke } from "@tauri-apps/api/core";

export type Change = "added" | "changed" | "unchanged";

export type PermissionLine = {
  /** The manifest's name, sent back as a choice. */
  name: string;
  sentence: string;
  required: boolean;
  change: Change | null;
};

export type OperationLine = {
  name: string;
  kind: "read" | "change";
  review: "reviewed" | "declared";
  method: string;
  url: string;
  sends: string[];
  fixed: string[];
};

export type AccountLine = { sentence: string; access: string[]; connected: boolean };

export type ConnectorLine = {
  id: string;
  version: string;
  origin: string;
  required: boolean;
  change: Change | null;
  sentence: string;
  destinations: string[];
  account: AccountLine | null;
  operations: OperationLine[];
};

export type Flow = {
  source: { kind: "account"; connector: string } | { kind: "this-machine"; origins: string[] };
  destinations: string[];
};

export type DialogModel = {
  kind: "install" | "update" | "update-confirm";
  asking: {
    name: string;
    id: string;
    author: string;
    version: string;
    installed_version: string | null;
  };
  permissions: PermissionLine[];
  connectors: ConnectorLine[];
  warning: Flow[];
  digest: string;
  choice: string;
};

export type InstallError = { kind: string; reason: string };

export type Started =
  | { request: string; model: DialogModel }
  | { none: true }
  | { refused: string }
  | { error: InstallError };

export type Judged = { warning: Flow[]; choice: string } | { refused: string };

export type Approved =
  | { approved: { handle: string } }
  | { refused: string }
  | { error: InstallError };

export function installStart(address: string): Promise<Started> {
  return invoke<Started>("install_start", { address });
}

export function updateStart(handle: string): Promise<Started> {
  return invoke<Started>("update_start", { handle });
}

export function requestJudge(request: string, optionalOn: string[]): Promise<Judged> {
  return invoke<Judged>("request_judge", { request, optionalOn });
}

export function requestApprove(
  request: string,
  choice: string,
  optionalOn: string[],
): Promise<Approved> {
  return invoke<Approved>("request_approve", { request, choice, optionalOn });
}

export function requestCancel(request: string): Promise<void> {
  return invoke<void>("request_cancel", { request });
}
