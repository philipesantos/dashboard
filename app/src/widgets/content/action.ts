// How a widget's controls run its actions: the frame provides it, the button and the form use it.

import { createContext } from "react";

/** Runs a widget's action from the control `control`; the frame answers it. */
export type Act = (action: string, values: Record<string, string>, control: string) => void;

type ActionState = {
  act: Act;
  /** The control whose action is running, if one is: it shows it is working. */
  running: string | null;
};

export const ActionContext = createContext<ActionState>({
  act: () => {},
  running: null,
});
