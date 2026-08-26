export {};

const transport = process.env.T15_TRANSPORT ?? "generated_safe_napi";

if (transport === "feature_gated_direct_ffi") {
  await import("./perf12_t15_direct_case.ts");
} else if (transport === "generated_safe_napi") {
  await import("./perf12_t15_authoritative_case.ts");
} else {
  throw new Error(`unsupported T15 transport: ${transport}`);
}
