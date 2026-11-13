# Verification

Updated 2026-11-14.

- `bun run check`: source boundary, TypeScript tooling, Rust formatting, native
  clippy, frontend rustc check, and frontend clippy.
- `bun run test:core`: 10 shared engine tests on native Rust and generated
  JavaScript, including atomicity, version conflicts, idempotence, per-author
  undo/redo, repeated undo/redo ordering, validation, and erasing.
- `bun run build && bun run test`: 25 Playwright tests against production assets.
  Tests exercise real native WebMCP and image pixels, human mouse/touch input,
  shapes/text, selection, clear, exports, partial erasing and transformed masks,
  replay progression/cancellation/reduced motion, concurrent tabs, storage aborts,
  denied/newer storage, offline reload, authenticated collaboration, conflicts,
  and SQLite/session persistence across server restarts.

The Canvas binding regression is in the tracked compiler patch and runs with
`bun test test/webapi-bindings.test.ts -t 'canvas context requests'` inside
`rust-js`. Browser traces are retained on failures. These checks cover the tested
behaviors; they are not a guarantee that no other bugs exist.
