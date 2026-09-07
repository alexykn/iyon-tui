// Documentation/evidence validation only. Run from any cwd with bun <this file>.
import { readFileSync, readdirSync, existsSync, statSync } from "node:fs";
import { dirname, resolve, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const atlas = "docs/architecture/atlas-4355c02/";
const provenance = JSON.parse(readFileSync(resolve(root, atlas, "evidence/report-provenance.json"), "utf8"));
const originals = new Set(provenance.map((p) => p.path));
const errors = [];
const warnings = [];
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8" }).trimEnd();
const files = [];
function walk(path) {
  for (const entry of readdirSync(resolve(root, path), { withFileTypes: true })) {
    const child = path + "/" + entry.name;
    if (entry.isDirectory()) walk(child);
    else files.push(child);
  }
}
walk("docs/architecture");
let totalReportBytes = 0;
let totalReportLines = 0;
let headingsChecked = 0;
let managedOriginalsMatched = 0;
for (const p of provenance) {
  const bytes = readFileSync(resolve(root, p.path));
  const text = bytes.toString("utf8");
  if (hash(bytes) !== p.sha256 || bytes.length !== p.bytes) errors.push({ path: p.path, kind: "hash-or-size" });
  if (!p.readByParent) errors.push({ path: p.path, kind: "unread" });
  let cursor = 1;
  for (const range of p.parentReadRanges ?? []) {
    const [start, end] = range.split("-").map(Number);
    if (start !== cursor || end < start) errors.push({ path: p.path, kind: "reading-range", range, expected: cursor });
    cursor = end + 1;
  }
  if (cursor !== p.lines + 1) errors.push({ path: p.path, kind: "reading-range-end", cursor, lines: p.lines });
  if (p.id !== "46") {
    for (let i = 0; i <= 10; i++) {
      if (!new RegExp("^## " + i + "[.) —:-]", "m").test(text)) errors.push({ path: p.path, kind: "required-heading", heading: i });
      headingsChecked++;
    }
  }
  if (existsSync(p.original) && !p.managedOriginalOverwrittenByResume) {
    if (hash(readFileSync(p.original)) !== p.sha256) errors.push({ path: p.path, kind: "managed-original-mismatch" });
    else managedOriginalsMatched++;
  } else if (p.managedOriginalOverwrittenByResume) {
    warnings.push({ path: p.path, kind: "managed-output-reused", detail: "canonical45 checked against preserved original hash; managed path now contains46" });
  } else {
    warnings.push({ path: p.path, kind: "managed-original-unavailable", detail: "canonical hash still verified" });
  }
  totalReportBytes += bytes.length;
  totalReportLines += p.lines;
}
if (provenance.length !== 46) errors.push({ kind: "report-count", actual: provenance.length });
const markdown = files.filter((p) => p.endsWith(".md"));
let linksChecked = 0;
let authoredLinksChecked = 0;
for (const path of markdown) {
  const text = readFileSync(resolve(root, path), "utf8");
  const archived = originals.has(path) || path.endsWith("/evidence/grouped-followup-task.md");
  const targetList = archived ? warnings : errors;
  if (!text.endsWith("\n") || text.includes("\u0000")) targetList.push({ path, kind: "text-framing" });
  const lines = text.split("\n");
  let inFence = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (/^\s*```/.test(line)) inFence = !inFence;
    // Two trailing spaces are intentional Markdown hard breaks.
    if (/[\t ]+$/.test(line) && !/\S {2}$/.test(line)) targetList.push({ path, line: i + 1, kind: "trailing-whitespace" });
    if (inFence) continue;
    for (const match of line.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
      const target = match[1].replace(/^<|>$/g, "").split(/\s+"/)[0];
      if (/^(?:https?:|mailto:|#)/.test(target)) continue;
      const pathname = target.split("#")[0];
      if (!pathname) continue;
      linksChecked++;
      if (!archived) authoredLinksChecked++;
      const abs = resolve(dirname(resolve(root, path)), decodeURIComponent(pathname));
      if (!existsSync(abs)) targetList.push({ path, line: i + 1, target, kind: "broken-relative-link" });
    }
  }
  if (inFence) targetList.push({ path, kind: "unclosed-code-fence" });
}
const baseline = "4355c02d6853549adf32a1e038b14665ce5c6bf8";
const head = git("rev-parse", "HEAD");
const branch = git("branch", "--show-current");
const trackedDiff = git("diff", "--name-only", "HEAD");
const manifest = readFileSync(resolve(root, atlas, "evidence/tracked-source-manifest.txt"), "utf8").trimEnd();
const baselineManifest = git("ls-tree", "-r", "--name-only", baseline);
const completeManifest = readFileSync(resolve(root, atlas, "evidence/baseline-tracked-manifest.txt"), "utf8").trimEnd();
if (head !== baseline) errors.push({ kind: "source-head-changed", head });
if (trackedDiff) errors.push({ kind: "tracked-diff", trackedDiff });
if (completeManifest !== baselineManifest) errors.push({ kind: "complete-baseline-manifest-mismatch" });
const collectedPaths = new Set(manifest.split("\n"));
const missingCollected = baselineManifest.split("\n").filter((p) => !collectedPaths.has(p));
const expectedOmissions = [".bun-version", ".cursor/rules/orchestration.mdc", ".gitignore", "LICENSE", "biome.jsonc", "cat-poem.txt"];
if (JSON.stringify(missingCollected) !== JSON.stringify(expectedOmissions)) errors.push({ kind: "unexpected-collected-manifest-omissions", paths: missingCollected });
if (missingCollected.length) warnings.push({ kind: "collected-manifest-omissions", paths: missingCollected, detail: "all six fully read; complete baseline manifest and coverage addendum preserved" });
const untracked = git("ls-files", "--others", "--exclude-standard").split("\n").filter(Boolean);
const unexpected = untracked.filter((p) => !p.startsWith("docs/architecture/"));
if (unexpected.length) errors.push({ kind: "untracked-outside-doc-scope", paths: unexpected });
try { git("diff", "--check"); } catch (e) { errors.push({ kind: "git-diff-check", detail: String(e) }); }
console.log(JSON.stringify({
  status: errors.length ? "failed" : "passed",
  baseline, head, branch,
  sourceManifestPaths: manifest.split("\n").length,
  completeBaselinePaths: baselineManifest.split("\n").length,
  trackedDiff: trackedDiff || null,
  reportCount: provenance.length, totalReportBytes, totalReportLines,
  requiredHeadingsChecked: headingsChecked,
  managedOriginalsMatched,
  markdownFiles: markdown.length, linksChecked, authoredLinksChecked,
  errors, warnings,
  limits: [
    "Link check covers inline relative path targets outside triple-backtick fences; URL health and heading fragments are not checked.",
    "Canonical scout reports are immutable evidence; their formatting/link defects are warnings, not silently rewritten.",
    "Parent reading flags/ranges verify recorded coverage, not automated substitute for actual full reading.",
    "No production test or benchmark execution; source-symbol checks remain parent evidence, not exhaustive compiler verification."
  ]
}, null, 2));
process.exitCode = errors.length ? 1 : 0;
