import { appendFile, readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";

const PACKAGE_ROOT = fileURLToPath(new URL("../", import.meta.url));
const CASE_RUNNER = fileURLToPath(new URL("./perf12_t15_realistic_case.ts", import.meta.url));

function run(command: string[], env?: Record<string, string>): { exitCode: number; stdout: string; stderr: string } {
  const result = Bun.spawnSync({
    cmd: command,
    cwd: PACKAGE_ROOT,
    env: { ...process.env, ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  return {
    exitCode: result.exitCode,
    stdout: result.stdout === undefined ? "" : new TextDecoder().decode(result.stdout),
    stderr: result.stderr === undefined ? "" : new TextDecoder().decode(result.stderr),
  };
}

function checked(command: string[], env?: Record<string, string>): string {
  const result = run(command, env);
  if (result.exitCode !== 0) throw new Error(`command failed (${result.exitCode}): ${command.join(" ")}\n${result.stderr}`);
  return result.stdout.trim();
}

function stage(direct: boolean): void {
  const env: Record<string, string> = direct ? { ION_NATIVE_FEATURES: "direct-ffi" } : {};
  const result = run(["bun", "run", "scripts/stage-native.ts"], env);
  if (result.stdout.length > 0) process.stdout.write(result.stdout);
  if (result.stderr.length > 0) process.stderr.write(result.stderr);
  if (result.exitCode !== 0) throw new Error(`native staging failed for ${direct ? "direct" : "N-API"}`);
}

async function addonSha(): Promise<string> {
  return createHash("sha256")
    .update(await readFile(new URL("../native/iyon-tui-native.node", import.meta.url)))
    .digest("hex");
}

async function main(): Promise<void> {
  const status = checked(["git", "status", "--porcelain"]);
  if (status !== "") throw new Error("authoritative realistic trace requires a clean committed checkout");
  const sourceSha = checked(["git", "rev-parse", "HEAD"]);
  const rustc = checked(["rustc", "--version"]);
  const target = checked(["rustc", "-vV"]).split(/\r?\n/).find((line) => line.startsWith("host:"))?.slice("host:".length).trim() ?? "unknown";
  const output = fileURLToPath(new URL(`./PERF-12-T15-realistic-${sourceSha.slice(0, 12)}.jsonl`, import.meta.url));
  await writeFile(output, "");

  for (const direct of [false, true]) {
    stage(direct);
    const transport = direct ? "feature_gated_direct_ffi" : "generated_safe_napi";
    const result = run(["bun", "run", CASE_RUNNER], {
      T15_PROFILE: "authoritative",
      T15_TRANSPORT: transport,
      T15_GIT_SHA: sourceSha,
      T15_RUSTC_VERSION: rustc,
      T15_TARGET: target,
      T15_NATIVE_SHA256: await addonSha(),
    });
    if (result.exitCode !== 0) throw new Error(`${transport} realistic trace failed:\n${result.stderr}`);
    const lines = result.stdout.trim().split(/\r?\n/).filter((line) => line.length > 0);
    if (lines.length !== 1) throw new Error(`${transport} realistic trace emitted ${lines.length} records`);
    await appendFile(output, `${lines[0]}\n`);
  }
  console.log(`wrote ${output}`);
}

await main();
