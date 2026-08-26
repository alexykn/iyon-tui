import type { Pointer } from "bun:ffi";
import { native as baseNative, type NativeTuiAddon } from "../../src/native.ts";

export interface NativeViewAbiBootstrap {
  readonly runtime_ptr: number;
  readonly abi_name: string;
  readonly abi_version: number;
  readonly semantic_version: number;
  readonly schema_blake3: string;
  readonly generator_blake3: string;
  readonly generation: number;
  readonly fast_view_abi: boolean;
  readonly function_count: number;
  readonly functions: Readonly<Record<string, number>>;
}

export interface DirectNativeTuiHost {
  tuiViewAbiHostPointer(): Pointer;
  render(view: object): void;
  screenRows(): string[];
  dispose(): void;
}

export type DirectNativeTuiAddon = Omit<NativeTuiAddon, "NativeTuiHost"> & {
  tuiViewAbiBootstrap?: (pruneExpired?: boolean) => NativeViewAbiBootstrap;
  NativeTuiHost?: new (width?: number, height?: number, headless?: boolean) => DirectNativeTuiHost;
};

export const native = baseNative as DirectNativeTuiAddon;
