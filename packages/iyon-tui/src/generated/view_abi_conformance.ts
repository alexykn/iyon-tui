// DO NOT EDIT. Generated from tools/tui-abi/view_abi.toml.
// schema_blake3 = 5e7332e72b071e87f451f9710dd21d6d9f707277281abe50f2583dc3509c1745
// generator_blake3 = cbaa60a2d973bcd75cb8ee49be131d5883f98a31fd8e88aa5344e4280efebfae
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
