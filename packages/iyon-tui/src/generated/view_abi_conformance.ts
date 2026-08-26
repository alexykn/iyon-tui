// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = ba12318e83f692e82b105219fcfcc8a53da1db568f007d381e091b259c67bc5c
// generator_blake3 = 0b79288012c798d4cc0a43433ed9113b1e50283de95f86b6059047a63a057665
import type { NativeTuiHostContract } from "../native.ts";
import type { NativeViewAbiHandle } from "./view_abi";

export type NativeAbiConformanceSession = NativeViewAbiHandle;

export function u8_8(session: NativeAbiConformanceSession, a0: number, a1: number, a2: number, a3: number, a4: number, a5: number, a6: number, a7: number): number {
  return session.u8_8(a0, a1, a2, a3, a4, a5, a6, a7);
}

export function u16_8(session: NativeAbiConformanceSession, a0: number, a1: number, a2: number, a3: number, a4: number, a5: number, a6: number, a7: number): number {
  return session.u16_8(a0, a1, a2, a3, a4, a5, a6, a7);
}

export function u32_8(session: NativeAbiConformanceSession, a0: number, a1: number, a2: number, a3: number, a4: number, a5: number, a6: number, a7: number): number {
  return session.u32_8(a0, a1, a2, a3, a4, a5, a6, a7);
}

export function u32_16(session: NativeAbiConformanceSession, a0: number, a1: number, a2: number, a3: number, a4: number, a5: number, a6: number, a7: number, a8: number, a9: number, a10: number, a11: number, a12: number, a13: number, a14: number, a15: number): number {
  return session.u32_16(a0, a1, a2, a3, a4, a5, a6, a7, a8, a9, a10, a11, a12, a13, a14, a15);
}

export function i32_4(session: NativeAbiConformanceSession, a0: number, a1: number, a2: number, a3: number): number {
  return session.i32_4(a0, a1, a2, a3);
}

export function f32_4(session: NativeAbiConformanceSession, a0: number, a1: number, a2: number, a3: number): number {
  return session.f32_4(a0, a1, a2, a3);
}

export function f64_4(session: NativeAbiConformanceSession, a0: number, a1: number, a2: number, a3: number): number {
  return session.f64_4(a0, a1, a2, a3);
}

export function pointer(session: NativeAbiConformanceSession, a0: boolean): number {
  return session.pointer(a0);
}

export function buffer(session: NativeAbiConformanceSession, a0: Uint8Array): number {
  return session.buffer(a0);
}

export function cstring(session: NativeAbiConformanceSession, a0: string): number {
  return session.cstring(a0);
}
