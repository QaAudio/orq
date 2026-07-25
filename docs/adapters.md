# Adapter authoring (launch profiles + integration packs)

Porq stays **agent-system agnostic**. Core knows tasks, events, leases,
triggers, and structured **launch profiles** / **result envelopes** — not
Cursor flags, Claude Code argv, or Codex quirks.

Source of truth for the profile schema:
`crates/orq-core/src/launch_profile.rs` (`LAUNCH_PROFILE_SCHEMA_VERSION = 1`).

## Launch profiles (generic)

A profile is typed argv + transports, never a joined shell string:

- `executable` + `argv` templates (`{cmd}`, `{prompt_file}`, `{event.*}`, …)
- `prompt_transport`: `stdin` | `file` | `argument`
- `result_transport`: `stdout_json` | `result_file` | `exit_code`
- `capabilities` (e.g. `repo.read`, `diff.review`) for affinity / policy
- optional `adapter_id` / `adapter_version` — filled by an integration pack

Untrusted event text must stay in argv placeholders / allowlisted env keys
(`ORQ_EVENT_*` / `ORQ_CORR_*`), never interpolated into `cmd /C` strings.

## Integration packs

`porq integrate <target>` should resolve through a **pack registry/trait**:

- Cursor is the first configured consumer pack (existing behavior).
- New providers (Claude Code, Codex, custom processes) add a pack — they do
  **not** edit task/trigger/router core.
- Target-specific install paths and CLI flags live in the pack, not in
  generated AGENTS/skill prose that assumes one vendor.

## Fake pack / conformance

Any new runner, spawn, or integrate behavior must pass a **fake fixture pack**
conformance test before a real provider binding lands:

1. Register a no-op / echo adapter that implements the same profile + result
   envelope contract.
2. Prove trigger spawn / supervised `run` / result parsing without Cursor
   installed.
3. Only then bind a real provider profile (e.g. `tdrs.exec-review` → Cursor
   Agent argv) in the consumer repo.

If the feature cannot work with the fake pack, it is not generic enough for
porq core — keep it in the consumer integration layer.
