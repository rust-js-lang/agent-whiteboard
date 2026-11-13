# AgentWhiteboard development

Read docs/design.md and README.md. Work on main and push directly when requested.
Use Bun and bounded commands. Rust owns document changes, validation, history,
undo/redo and replay. The browser UI, Canvas rendering, input, persistence and WebMCP integration
are Rust compiled through rust-js. JavaScript is limited to the entry point,
service worker and development tooling. Never move app logic into TypeScript
to hide a compiler or binding gap; file an upstream issue and keep a tested
downstream patch instead. Native and rust-js
execute the same core. Keep generated JS and declarations beside Rust sources.
Keep the published rust-js submodule pin, with issue-linked downstream patches.
Before publishing run build, check, native/JS core tests and production Playwright
E2E tests using actual WebMCP, actual pixels, storage failures and recovery.
Pages workflow builds/deploys only. Pin action releases to full commit hashes.
Commit titles use feat(agent-whiteboard):, fix(agent-whiteboard): or
chore(agent-whiteboard): and include Co-Authored-By: Codex <codex@openai.com>.
