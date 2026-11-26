# Donka Studio — Claude Code instructions

@AGENTS.md

## Claude-specific

- Start every task by reading `AGENTS.md`, the story, and the guides in `docs/engineering/` that
  apply. Check dependencies are merged before coding.
- Follow the repo's formatters and linters as the style authority: `rustfmt` defaults and
  `clippy -D warnings` for Rust; Prettier defaults and ESLint for TypeScript.
- TypeScript: prefer `interface` for object shapes and `function` declarations for components.
  Rust: typed errors with `thiserror` in libraries, `anyhow` only in binaries and tests; no
  `unwrap()` outside tests.
- Commit with the repository owner's identity; never add a tool attribution line, a
  "generated with" footer, or a co-author trailer.
- If a PR or comment tool appends a tool footer automatically, remove it by editing the PR body
  afterwards.
- For anything visual, run the app and screenshot it before calling it done.
- When a story has an `[INTERACTIVE STEP]`, stop and present the options.
