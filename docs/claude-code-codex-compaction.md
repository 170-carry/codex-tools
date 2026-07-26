# Claude Code compaction compatibility for Codex proxy

## Background

`codex-tools` exposes Anthropic-compatible `/v1/messages` and related endpoints so Claude Code can talk to the local proxy, while the actual inference and quota consumption happen on the Codex upstream. The proxy is therefore not a true Anthropic Messages API backend; it is a compatibility layer that translates Claude Code requests into Codex/OpenAI-style requests.

Claude Code can trigger Anthropic Messages context compaction when the conversation gets large. Native Anthropic compaction relies on Anthropic-only request fields and content blocks, especially `context_management` with `compact_20260112` and response/history content blocks such as `compaction`. Codex upstream does not understand these blocks.

The proxy must therefore implement a compatibility shim: consume Claude Code's Anthropic-only context-management signals, preserve their useful information, and translate them into plain text that Codex can use as continuation context.

## Current issue

The Anthropic-to-Codex converter currently only handles a small set of content blocks:

- `text`
- `image`
- `tool_use`
- `tool_result`

Other Anthropic content blocks can be silently ignored. That is risky after Claude Code compacts a conversation: if a `compaction`, `thinking`, `redacted_thinking`, or future Anthropic-only block is dropped, the Codex request loses continuation context and debugging is difficult because nothing visible records what was skipped.

## Design goals

1. Do not attempt to make Codex support Anthropic native compaction.
2. Do not forward Anthropic-only fields to Codex if Codex may reject them.
3. Never silently discard Anthropic content blocks in the Claude Code compatibility path.
4. Convert useful Anthropic-only blocks into ordinary text content for Codex.
5. Add explicit logs for ignored, translated, or unknown pieces so future failures can be diagnosed from proxy logs.
6. Prefer conservative behavior: preserving too much as text is better than losing context silently.

## Compatibility strategy

### Request field handling

When `/v1/messages` receives `context_management.edits` containing `compact_20260112`, the proxy should:

- Log that Claude Code requested compaction.
- Keep treating `context_management` as Anthropic-only; do not forward it to Codex.
- Use the signal as future input for local proxy-side compaction if needed.

For now, request-level compaction can be handled as a diagnostic signal plus content-block preservation. A later enhancement can actually compact earlier messages before conversion.

### Content block handling

The converter should explicitly handle Anthropic-only blocks:

- `compaction`: convert to a plain text continuation note. Prefer readable fields such as `summary`, `text`, or `content` when present; otherwise preserve the full JSON block.
- `thinking`: if it contains non-empty visible/summarized thinking text, convert that to a plain text note. If empty, log and skip.
- `redacted_thinking`: log and skip, because there is intentionally no readable continuation content.
- Unknown block types: log the block type and preserve the full JSON as a plain text note.

This keeps Codex-compatible input while avoiding hidden data loss.

### Logging expectations

Logs should identify:

- When Claude Code requested compact context management.
- Which Anthropic content block types were translated.
- Which Anthropic content block types were skipped and why.
- Which unknown content block types were preserved as JSON.

Logs should not include sensitive full block contents by default. The converted request may still contain preserved JSON as model input because that is necessary for continuation, but logs should stay metadata-oriented.

## Future enhancement: local proxy-side compaction

A later implementation can add true proxy-side local compaction for Codex:

1. Estimate Anthropic input size conservatively.
2. If above a threshold, keep the most recent N messages.
3. Summarize older messages into a plain text `<context_summary>` message.
4. Convert `summary + recent messages` to Codex input.

Recommended initial constants:

```rust
const ANTHROPIC_LOCAL_COMPACT_TRIGGER_TOKENS: u32 = 180_000;
const ANTHROPIC_LOCAL_COMPACT_KEEP_RECENT_MESSAGES: usize = 24;
```

Start with heuristic summarization to avoid extra Codex quota consumption. Model-generated summaries can be added behind a setting later.

## Validation

After implementation:

- Run Rust formatting.
- Run targeted Rust tests if available.
- If no targeted tests cover this proxy path, run `cargo test --manifest-path src-tauri/Cargo.toml` or at least `cargo check --manifest-path src-tauri/Cargo.toml`.
- Manually inspect logs with a Claude Code request that includes `context_management.compact_20260112` or synthetic `compaction` content.
