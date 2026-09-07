# Orchestration recovery

- Original run: `b363950e-0ff9-4622-a8f0-491385679af2`, terminal failed after all44 independent scouts completed.
- Error: `emit.reports[0].outputReference must be a JSON value; received undefined` at workflow-script.js:4:1.
- Cause: parent script assumed optional metadata existed at workflow JSON boundary. Child output strings contained valid managed artifact paths; outputReference was absent.
- Pending45 did not launch. No scout investigation failed.
- Before retry: cwd `/Users/alxknt/github/iyon-n/iyon-tui`, main HEAD `4355c02d6853549adf32a1e038b14665ce5c6bf8`; only `?? docs/architecture/`, no tracked-source diff. `git diff --check` clean.
- Captured untracked docs at `/tmp/iyon-atlas-b363950e-pre-recovery-docs.tgz` (local backup).
- Verified all44 Markdown artifacts exist; bulk copied into atlas; originals preserved.
- Same native scout/workflow protocol, only pending45 against existing artifacts; no external/foreground fallback or completed-scout reruns.
- Recovery uses documented JSON fields with explicit null for optional values. Script: `recovery-workflow.js`.

Recovery workflow: `0b1ee8ef-ede7-4b21-9db7-16f5c2b82e20`; retained scout45: `258b3ac0-056c-4480-bbe4-909bf34d8a4e`. Completed canonical45 is preserved with hash in report-provenance.json and fully read by parent. Subsequent grouped management resume is recorded separately in grouped-followup-execution.md.
