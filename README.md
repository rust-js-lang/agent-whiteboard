# AgentWhiteboard

A whiteboard for people and AI agents, built in Rust with rust-js.

[Open the whiteboard](https://rust-js-lang.github.io/agent-whiteboard/).

Draw freehand with pressure, make curves and shapes, add text, select and move
marks, erase parts of ink, and undo or redo. Replay reconstructs the drawing
from a blank board without changing saved history. Export PNG, SVG, or the JSON
journal. WebMCP agents use the same drawing engine and can inspect PNG captures.

## Run locally

Requirements: Bun 1.4.4, Rust/rustup, and the compiler toolchain specified in the
rust-js submodule. Clone with `--recurse-submodules`, then:

```sh
bun run setup
bun run build
bun run dev
```

The UI, input handling, Canvas rendering, persistence, and WebMCP tools are Rust.
JavaScript is limited to the React entry, generated output, build tooling, and
service worker. Generated files beside Rust sources are checked in; edit Rust.
The published rust-js submodule receives tracked [downstream patches](patches/rust-js/README.md).

## Local and shared storage

The default board persists in IndexedDB. After the first successful load, the
service worker caches application assets for offline use. If storage access is
denied, the UI explicitly offers a temporary session. Corrupt or newer records
are preserved and editing is blocked rather than overwriting them.

For a shared room, build first, then run the Rust server:

```sh
WHITEBOARD_TOKEN='replace-with-a-long-shared-secret' bun run server
```

The server serves `dist` and stores accepted commands in SQLite. Configure
`WHITEBOARD_DB` (default `whiteboard.db`) and `WHITEBOARD_BIND` (default
`127.0.0.1:4080`). Put it behind HTTPS, including Tailscale HTTPS, and open
`/#room=studio`. For local HTTP development only, set
`WHITEBOARD_SECURE_COOKIE=false`. Sign in using the configured shared secret.
Every signed-in participant is an editor; sessions have separate actor identities.
GitHub Pages runs the local mode; shared rooms require the native server origin.

Server clients poll every 700 ms. Commands commit atomically, duplicate command
IDs are idempotent, and stale edits are rejected. Undo affects the current actor's
history. Failed server writes are not acknowledged or silently stored elsewhere.
WebSocket streaming, offline server-write queues, granular roles, board resizing,
grouping, and import controls remain future work. See [the design](docs/design.md)
for the longer-term plan.

## Agent tools

Enable WebMCP in a compatible browser. The seven native tools are `get_board`,
`get_elements`, `apply_operations`, `get_view`, `set_viewport`, `export_board`,
and `replay_drawing`. Read board and element versions before editing; submit
strokes, Bezier paths, text, or batches of operations. `get_view` returns actual
image content so an agent can inspect and revise its drawing.

## Verification and deployment

```sh
bun run check
bun run test:core
bun run build
bun run test
```

Playwright exercises the built app through real WebMCP, mouse, touch, IndexedDB,
offline loading, and separate authenticated server clients. [TESTS.md](TESTS.md)
records coverage. GitHub Actions only builds and deploys Pages; tests run locally.
Actions are pinned to full commit hashes.
