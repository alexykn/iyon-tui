import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { basename, dirname, join, relative, resolve, sep } from "node:path";
import { tmpdir } from "node:os";

const ROOT = resolve(import.meta.dir, "../..");
const SOURCE = join(ROOT, "packages/iyon-tui/src/index.ts");
const TESTING_SOURCE = join(ROOT, "packages/iyon-tui/src/testing.ts");
const PRIVATE_MODULES = new Set(["ir", "native", "retained_dag", "native_view_abi"]);
const PRIVATE_TYPE = /\b(?:Bridge[A-Z]\w*|Native(?:Tui|View|History|Text|Scroll|Projector|Output)(?:Contract|Abi[A-Z]\w*))\b/u;

const publicTypeNames = [
  "AnsiColor",
  "BorderEdges",
  "BorderGlyphs",
  "BorderSpec",
  "BorderStyle",
  "ColorSpec",
  "ComponentHandle",
  "ComponentAdapter",
  "ComponentCapabilities",
  "ComponentContext",
  "DiffHunk",
  "DiffLineKind",
  "DiffLineTermination",
  "GridBuilder",
  "GridCell",
  "GridRow",
  "GridRowBuilder",
  "GridSpec",
  "GridTrack",
  "HandleId",
  "History",
  "HistoryLayout",
  "HorizontalAlign",
  "Insets",
  "InsetsValue",
  "InteractionResult",
  "LayoutChild",
  "Output",
  "OutputEvent",
  "OverflowIndicator",
  "ProjectionSpan",
  "RgbColor",
  "Scene",
  "SceneProducer",
  "ScrollPane",
  "SemanticTag",
  "SemanticValue",
  "State",
  "StreamAnnotation",
  "StreamSegmentSnapshot",
  "StreamSnapshot",
  "StyleRef",
  "StyleSelector",
  "StyleSelectorValue",
  "StyleSpec",
  "StyleSpecValue",
  "StyleStateKey",
  "StyleStateValue",
  "TextAttribute",
  "TextContent",
  "TextFormat",
  "TextInput",
  "TextInputOptions",
  "TextOrigin",
  "TextPart",
  "TextRole",
  "TextSelector",
  "TextSelectorValue",
  "TextSpan",
  "TextSpanValue",
  "TextStream",
  "TextStreamOptions",
  "TextStreamPacing",
  "TextStreamPresentation",
  "TerminalMetadata",
  "Theme",
  "ThemeColor",
  "ThemeColorDefault",
  "ThemeColorIndexed",
  "ThemeColorNamed",
  "ThemeColorReference",
  "ThemeKey",
  "Tui",
  "TuiErrorCategory",
  "TuiEvent",
  "TuiOpenOptions",
  "TuiRuntime",
  "VerticalAlign",
  "View",
  "ViewChildren",
  "ViewComponent",
  "ViewSlot",
  "WrapMode",
] as const;

function runTsc(args: readonly string[]): boolean {
  const result = Bun.spawnSync(["bunx", "tsc", ...args], {
    cwd: ROOT,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (result.exitCode === 0) return true;
  const output = `${new TextDecoder().decode(result.stdout)}${new TextDecoder().decode(result.stderr)}`.trim();
  console.error(output);
  return false;
}

function resolveDeclaration(from: string, specifier: string): string | undefined {
  const base = resolve(dirname(from), specifier);
  const candidates = [
    base,
    base.endsWith(".ts") ? `${base.slice(0, -3)}.d.ts` : base,
    `${base}.d.ts`,
    join(base, "index.d.ts"),
  ];
  return candidates.find((candidate) => existsSync(candidate));
}

function localSpecifiers(source: string): string[] {
  const result: string[] = [];
  const pattern = /(?:\bfrom\s+|\bimport\s*\(\s*)(["'])([^"']+)\1/gu;
  for (const match of source.matchAll(pattern)) {
    const specifier = match[2]!;
    if (specifier.startsWith(".")) result.push(specifier);
  }
  return result;
}

function forbiddenModule(path: string): boolean {
  const normalized = path.split(sep).join("/");
  const file = basename(normalized).replace(/\.d\.ts$/u, "");
  return PRIVATE_MODULES.has(file) || normalized.includes("/generated/");
}

const output = mkdtempSync(join(tmpdir(), "iyon-tui-declaration-"));
let failed = false;
try {
  failed = !runTsc([
    "--ignoreConfig",
    "--declaration",
    "--emitDeclarationOnly",
    "--noEmit",
    "false",
    "--outDir",
    output,
    "--target",
    "ESNext",
    "--module",
    "ESNext",
    "--moduleResolution",
    "Bundler",
    "--strict",
    "--resolveJsonModule",
    "--allowImportingTsExtensions",
    "--verbatimModuleSyntax",
    "--types",
    "bun-types",
    SOURCE,
    TESTING_SOURCE,
  ]);
  const rootDeclaration = join(output, "index.d.ts");
  const testingDeclaration = join(output, "testing.d.ts");
  for (const [name, declaration] of [["index.d.ts", rootDeclaration], ["testing.d.ts", testingDeclaration]] as const) {
    if (!existsSync(declaration)) {
      console.error(`H1A/H1H declaration closure: ${name} was not emitted`);
      failed = true;
    }
  }

  const reachable = new Set<string>();
  const pending = [rootDeclaration, testingDeclaration];
  while (pending.length > 0) {
    const declaration = pending.pop()!;
    if (reachable.has(declaration) || !existsSync(declaration)) continue;
    reachable.add(declaration);
    const sourceText = readFileSync(declaration, "utf8");
    if (PRIVATE_TYPE.test(sourceText)) {
      console.error(`H1A declaration closure: private implementation type in ${relative(output, declaration)}`);
      failed = true;
    }
    for (const specifier of localSpecifiers(sourceText)) {
      const target = resolveDeclaration(declaration, specifier);
      if (target === undefined) {
        console.error(`H1A declaration closure: unresolved ${specifier} from ${relative(output, declaration)}`);
        failed = true;
        continue;
      }
      if (forbiddenModule(relative(output, target))) {
        console.error(`H1A declaration closure: private declaration edge ${relative(output, declaration)} -> ${relative(output, target)}`);
        failed = true;
        continue;
      }
      pending.push(target);
    }
  }

  const probe = join(output, "public-surface-probe.ts");
  const probeReferences = publicTypeNames.map((name) =>
    ["Output", "State"].includes(name)
      ? `${name}<unknown>`
      : name,
  );
  writeFileSync(
    probe,
    `import type {\n${publicTypeNames.map((name) => `  ${name},`).join("\n")}\n} from "./index.d.ts";\n\nexport type PublicSurfaceProbe = [${probeReferences.join(", ")}];\n`,
  );
  if (!runTsc([
    "--ignoreConfig",
    "--noEmit",
    "--target",
    "ESNext",
    "--module",
    "ESNext",
    "--moduleResolution",
    "Bundler",
    "--strict",
    "--allowImportingTsExtensions",
    "--verbatimModuleSyntax",
    "--skipLibCheck",
    probe,
  ])) {
    console.error("H1A declaration closure: public type nameability probe failed");
    failed = true;
  }

  const testingProbe = join(output, "testing-surface-probe.ts");
  writeFileSync(
    testingProbe,
    `import type { AppHarness, createAppHarness } from "./testing.d.ts";\n\nexport type PublicTestingFactory = typeof createAppHarness;\nexport type PublicTestingSurface = AppHarness;\n`,
  );
  if (!runTsc([
    "--ignoreConfig",
    "--noEmit",
    "--target",
    "ESNext",
    "--module",
    "ESNext",
    "--moduleResolution",
    "Bundler",
    "--strict",
    "--allowImportingTsExtensions",
    "--verbatimModuleSyntax",
    "--skipLibCheck",
    testingProbe,
  ])) {
    console.error("H1H declaration closure: testing subpath nameability probe failed");
    failed = true;
  }

  if (!failed) console.log(`PASS h1a-declaration-closure — ${reachable.size} declaration files reachable; root and testing public types are nameable`);
} finally {
  rmSync(output, { recursive: true, force: true });
}

if (failed) process.exit(1);
