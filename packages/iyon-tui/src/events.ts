import type { KeyEvent, PasteEvent, ResizeEvent, TerminateEvent } from "./types.ts";

export const keyEvent = (key: string, modifiers?: readonly string[]): KeyEvent => ({ type: "key", key, modifiers });
export const pasteEvent = (text: string): PasteEvent => ({ type: "paste", text });
export const resizeEvent = (width: number, height: number): ResizeEvent => ({ type: "resize", width, height });
export const terminateEvent = (reason?: string): TerminateEvent => ({ type: "terminate", reason });
