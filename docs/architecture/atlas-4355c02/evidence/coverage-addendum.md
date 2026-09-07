# Final manifest reconciliation

Final `git ls-tree -r --name-only 4355c02` contains **1088 tracked paths**, not1082. The original [collected manifest](tracked-source-manifest.txt) contains1082 of those paths and is preserved unchanged as an execution input. The [complete baseline manifest](baseline-tracked-manifest.txt) records all1088. Set comparison found no extra collected paths and exactly six omitted paths.

The parent fully read all six at final validation:
| Path | Classification / architectural significance |
|---|---|
| .bun-version | Pins Bun1.4.0, corroborating packageManager |
| .cursor/rules/orchestration.mdc | Cursor-specific orchestration policy, not runtime production; this Pi run obeys its governing native protocol, not a tool-read instruction to change mode |
| .gitignore | Target/node/build/editor exclusions and ignored local qualification drafts; ignored artifacts are not tracked baseline evidence |
| LICENSE | MIT licensing, no execution behavior |
| biome.jsonc | Formatter/linter policy, generated exclusions, complexity threshold15 warning, selected existing warning downgrades including unsafe declaration merging |
| cat-poem.txt | Unreferenced incidental prose; no runtime implementation |

This closes a **manifest completeness discrepancy**, not a newly discovered production subsystem. Earlier parent notes/report45 corrections saying “canonical collected manifest1082” remain correct about that artifact but must not imply that1082 is the entire Git baseline. Approximate report LOC and appendix path totals remain non-census estimates.

Validation also normalized provenance's older `readRanges`/Unicode-dash spellings to `parentReadRanges`/ASCII from the existing full-reading ledger. No reading was inferred from line counts and no canonical report bytes changed.
