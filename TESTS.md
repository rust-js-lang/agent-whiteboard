# Verification

Updated 2026-11-14.

- `bun run check`: source boundary, TypeScript tooling, Rust formatting, native
  clippy, frontend rustc check, and frontend clippy.
- `bun run test:core`: 14 shared engine tests on native Rust and generated
  JavaScript, including atomicity, version conflicts, idempotence, per-author
  undo/redo, repeated undo/redo ordering, validation, and erasing.
- `bun run build && bun run test`: 38 Playwright tests against production assets.
  Tests exercise real native WebMCP and image pixels, human mouse/touch input,
  shapes/text, selection, clear, exports, partial erasing and transformed masks,
  replay progression/cancellation/reduced motion, concurrent tabs, storage aborts,
  denied/newer storage, offline reload, authenticated collaboration, conflicts,
  and SQLite/session persistence across server restarts. Painting regressions check
  interpolated gradient pixels, stop transparency, soft falloff, seeded texture,
  transformed clips, SVG pixel parity, human controls, server persistence, and
  atomic invalid-input rejection.

The Canvas binding regression is in the tracked compiler patch and runs with
`bun test test/webapi-bindings.test.ts -t 'canvas context requests'` inside
`rust-js`. Browser traces are retained on failures. These checks cover the tested
behaviors; they are not a guarantee that no other bugs exist.

`bun scripts/check-react-compiler.ts` verifies actual automatic memoization of
App, Icon, and useTools with no diagnostics. The four React compatibility
regressions in the downstream patch include native JSON comparisons and object
copy/identity checks. Deliberately ignoring the texture seed and disabling clipping
were both caught by the domain/browser regressions before restoring the implementation.

Renderer regressions also cover 700-mark path retention and compositing allocation budgets, reuse of finished art during drawing, affine stroke bounds, wide text, preview layer order, and exact native gradient pixels. See [the before/after profile](docs/performance.md) for the saved-drawing comparison and reproducible CPU/pixel tooling.
