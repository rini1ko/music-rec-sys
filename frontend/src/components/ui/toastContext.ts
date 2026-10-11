import { createContext } from "react";

export type ToastKind = "success" | "error" | "info";

export interface ToastApi {
  show: (kind: ToastKind, text: string) => void;
}

export const ToastContext = createContext<ToastApi | null>(null);
