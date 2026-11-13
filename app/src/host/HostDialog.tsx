// The one place a host dialog is drawn (specs/010-plugin-installation/research.md, R8; criteria
// HS-10 to HS-23 of docs/design/host-surfaces.md):
//
// - over the chrome and every frame, in the host-only `dialog` roles, on the `scrim` (HS-10);
// - the rest of the window `inert` while it is open, and a click outside it deciding nothing
//   (HS-11, HS-22);
// - input ignored for INPUT_DELAY_MS after it opens, again when the window regains focus and
//   while input aimed at it keeps arriving; the approve button disabled meanwhile (HS-12);
// - Enter and Escape handled by its own key handler, not the browser's default action: Escape,
//   and Enter anywhere but on another control, take the safe choice (HS-14, HS-19);
// - the safe choice first in tab order and focused on open, Tab kept inside (HS-18);
// - focus returned to the control that opened it, or to the window when it is gone (HS-20);
// - its title and its description, who is asking and what is decided, as its accessible name
//   and description (HS-21);
// - its header and footer fixed, only its body scrolling (HS-15); a visible edge in forced colors
//   (HS-17, `index.css`); no animation under reduced motion (HS-23, `components/ui/dialog.tsx`).
//
// Part of the security core (constitution v2.7.0, Principle IX).

import { useCallback, useEffect, useRef, useState, type KeyboardEvent, type ReactNode, type RefObject, type SyntheticEvent } from "react";
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

/** How long input is ignored after the dialog appears, as Firefox does for its security dialogs
 * (`security.dialog_enable_delay`). */
export const INPUT_DELAY_MS = 1000;

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
  /** The control that opened the dialog, where focus returns when it closes. */
  opener?: RefObject<HTMLElement | null>;
  onSafe: () => void;
  onApprove?: () => void;
};

/** The application's root, made inert while a host dialog is open. */
function appRoot(): HTMLElement | null {
  return document.getElementById("root");
}

export function HostDialog({
  open,
  title,
  description,
  children,
  safeLabel,
  approveLabel,
  approveDisabled,
  opener,
  onSafe,
  onApprove,
}: HostDialogProps) {
  const [armed, setArmed] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  /** Starts the input delay over: input is ignored until it ends. */
  const disarm = useCallback(() => {
    setArmed(false);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setArmed(true), INPUT_DELAY_MS);
  }, []);

  useEffect(() => {
    if (!open) return;
    disarm();
    const root = appRoot();
    root?.setAttribute("inert", "");
    window.addEventListener("focus", disarm);
    return () => {
      window.removeEventListener("focus", disarm);
      window.clearTimeout(timer.current);
      root?.removeAttribute("inert");
    };
  }, [open, disarm]);

  /** Input aimed at the dialog during the delay does nothing, and starts the delay over. */
  const swallow = (event: SyntheticEvent) => {
    if (armed) return;
    event.preventDefault();
    event.stopPropagation();
    disarm();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Enter" && event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    if (!armed) {
      disarm();
      return;
    }
    if (event.key === "Escape") {
      onSafe();
      return;
    }
    // Enter on another control (the approve button, a switch) is that control's; anywhere else,
    // the safe choice. Pressing Enter without moving focus never approves.
    const target = event.target as HTMLElement;
    const control = target.closest("button, [role=switch], input");
    if (control && !control.hasAttribute("data-safe-choice")) {
      (control as HTMLElement).click();
      return;
    }
    onSafe();
  };

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
        data-armed={armed ? "" : undefined}
        className="grid max-h-[calc(100dvh-2rem)] grid-rows-[auto_minmax(0,1fr)_auto] overflow-hidden sm:max-w-lg"
        onKeyDownCapture={onKeyDown}
        onPointerDownCapture={swallow}
        onClickCapture={swallow}
        // Escape and a click outside are handled here, never by the primitive's defaults
        onEscapeKeyDown={(event) => event.preventDefault()}
        onPointerDownOutside={(event) => event.preventDefault()}
        onInteractOutside={(event) => event.preventDefault()}
        onOpenAutoFocus={(event) => {
          event.preventDefault();
          document.querySelector<HTMLElement>("[data-host-dialog] [data-safe-choice]")?.focus();
        }}
        onCloseAutoFocus={(event) => {
          event.preventDefault();
          appRoot()?.removeAttribute("inert");
          const back = opener?.current;
          if (back?.isConnected) back.focus();
          else document.body.focus();
        }}
      >
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription asChild>
            <div>{description}</div>
          </DialogDescription>
        </DialogHeader>
        <div data-dialog-scroll className="min-h-0 overflow-y-auto">
          {children}
        </div>
        <DialogFooter>
          <DialogPrimitive.Close asChild>
            <Button variant="outline" data-safe-choice="">
              {safeLabel}
            </Button>
          </DialogPrimitive.Close>
          {approveLabel && onApprove && (
            <Button disabled={approveDisabled || !armed} data-approve="" onClick={onApprove}>
              {approveLabel}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
