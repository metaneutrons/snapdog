# Kiro Rules

## CRITICAL: Never Do Without Explicit User Consent

- **Never `git push`** — always ask first
- **Never create PRs** — always ask first
- **Never merge PRs** — always ask first
- **Never delete branches/tags/releases** — always ask first
- **Never run `cargo publish`** — always ask first

## Commit Rules

- Commit locally without asking — this is safe and expected
- Always use conventional commits
- Amend only the most recent commit, and only when it's the same topic
- Split unrelated changes into separate commits

## Principles

**IMPORTANT: No hacks, no shortcuts.** When tempted to say "the simplest way" — stop, step back, and think about the proper enterprise-grade solution. Even if it requires a heavy refactor, do it right. Quick fixes accumulate into technical debt. The correct pattern, implemented once, is always cheaper than a hack that needs to be replaced later.

### SSOT — Single Source of Truth
Every piece of knowledge (config, constants, types, logic) must live in exactly one place. If something needs to be referenced elsewhere, import it — don't duplicate it. When a value changes, it should only need to change in one location.

### DRY — Don't Repeat Yourself
Duplicated code is a bug waiting to happen. Extract shared logic into functions, traits, or modules. If you find yourself copying code, that's a signal to refactor. Two is a coincidence, three is a pattern — extract it.

### Convention over Configuration
Prefer sensible defaults and predictable project structure over explicit configuration. Follow Rust ecosystem conventions (module layout, error handling patterns, naming). When the community has a standard way, use it.

### Fail Fast, Recover Gracefully
Validate inputs early. Return errors immediately rather than propagating invalid state. Use `Result` and `?` — don't `unwrap()` in library code. Provide clear, actionable error messages that help the user fix the problem.

## Code Changes

- Use `str_replace` for edits, not Python scripts
- Run `cargo build` and `cargo test` after changes before reporting success
- Run `cargo fmt --all` before committing

## Communication

- Don't run interactive commands (cargo login, git rebase -i) — they freeze the terminal
- When a command fails, show the error and propose a fix — don't retry silently
- When context is getting long, summarize progress and suggest saving
