// The one place a host dialog is drawn (specs/010-plugin-installation/research.md, R8): over the
// chrome and every frame, in the host-only `dialog` roles, its safe choice first and focused, and
// Escape taking it. The form of criteria HS-10 to HS-23 (inertness, the input delay, focus
// return, the accessible name) is completed with task T044.
//
// Part of the security core (constitution v2.7.0, Principle IX).

import { type ReactNode } from "react";
import { Dialog as DialogPrimitive } from "radix-ui";

import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

export type HostDialogProps = {
  open: boolean;
  title: string;
  /** Who is asking and what is decided: the dialog's accessible description. */
  description: ReactNode;
  children?: ReactNode;
  /** The safe choice's label: cancel or deny. */
  safeLabel: string;
  /** The choice that grants something, or none for a dialog that only informs. */
  approveLabel?: string;
  approveDisabled?: boolean;
  onSafe: () => void;
  onApprove?: () => void;
};

export function HostDialog({
  open,
  title,
  description,
  children,
  safeLabel,
  approveLabel,
  approveDisabled,
  onSafe,
  onApprove,
}: HostDialogProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onSafe();
      }}
    >
      <DialogContent
        showCloseButton={false}
        data-host-dialog=""
        onEscapeKeyDown={(event) => {
          event.preventDefault();
          onSafe();
        }}
        // A click outside is not a decision: the rest of the window does nothing while it is open
        onPointerDownOutside={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
      >
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription asChild>
            <div>{description}</div>
          </DialogDescription>
        </DialogHeader>
        {children}
        <DialogFooter>
          <DialogPrimitive.Close asChild>
            <Button variant="outline" autoFocus data-safe-choice="">
              {safeLabel}
            </Button>
          </DialogPrimitive.Close>
          {approveLabel && onApprove && (
            <Button disabled={approveDisabled} data-approve="" onClick={onApprove}>
              {approveLabel}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
