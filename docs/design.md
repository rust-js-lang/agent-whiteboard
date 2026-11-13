# AgentWhiteboard design and implementation plan

Status: initial implementation available; longer-term architecture below. Updated: 2026-11-14.

The initial release implements a Rust/rust-js browser UI, Canvas drawing, native WebMCP, IndexedDB, undo/redo, replay, offline assets, and a Rust/SQLite shared server. Shared clients currently poll every 700 ms. Board resizing, grouping, granular roles, WebSocket transport, and import controls described below are planned, not shipped. See the README for current commands and limitations.

## Product and drawing experience

AgentWhiteboard is a shared drawing surface for humans and AI agents. A user
should be able to ask an agent to draw a duck, sketch a bird in flight, annotate
an idea, or collaborate on a scene. The agent draws the actual marks, inspects
the result, and revises it. There is no fixed catalog of subjects it can draw.

The foundation is freehand ink and arbitrary editable paths. Rectangles, arrows,
ellipses, and text are conveniences built on the same board, rather than the
limits of what the board can express. An agent does not need to simulate hundreds
of mouse gestures or invoke a hidden image generator to draw a duck.

Start with a clean white surface, a compact marker tray, an eraser, selection,
undo, and zoom. Give ink round caps, smooth curves, clear colors, and optional
pressure variation. Drawing should feel immediate under a mouse, touch, or pen.
Keep the chrome quiet; show connection and saving status without covering art.
A flat 2D surface serves this experience. Three.js and a simulated 3D room are
unnecessary for the initial application.

The board starts at 4096 by 2304 logical units, with pan, zoom, fit-to-board,
and explicit resize. These are initial product defaults. Logical units are
independent of screen pixels and device pixel ratio. Artists can enlarge the
surface without changing existing marks. Resizing changes the frame; it does
not scale or delete artwork. Exports crop to the requested frame or region.

### What makes it a real drawing surface

- Freehand strokes accept ordered points and optional pressure, with no template
  recognition or automatic conversion into shapes.
- Paths support lines, quadratic and cubic Bezier curves, closed contours, fills,
  and configurable stroke widths. Filled silhouettes and fine line art both work.
- Erasing removes parts of existing ink and reveals the actual content underneath.
  Painting white over a mark is not erasing.
- Selection can move, resize, rotate, duplicate, group, and reorder artwork.
  Authorship is preserved when another editor changes a mark.
- Text supports labels and annotations. Drawing works without text, fonts, or
  an image service. Imported raster images can be added after the ink workflow.

Deterministic brushes come first. Later marker texture must use stored brush
versions and seeds, so clients reproduce the same artwork. Avoid random jitter
that changes the drawing on each repaint.

## Local and shared modes

| Concern | Local mode | Shared mode |
|---|---|---|
| Command authority | Browser session | Native Rust server |
| Durable storage | IndexedDB | SQLite |
| Drawing and editing | Full board tools | Same tools, subject to room permissions |
| Collaboration | Same-browser tabs initially | Multiple humans and agents on multiple machines |
| Recovery | Local snapshot and accepted operations | Server snapshot and ordered event stream |
| Sharing | Export a board file or image | Authenticated room link and optional export |

Local mode needs no application backend. Bundle all assets, fonts, and required
code; cache them with a service worker so an installed application can reopen
without a network after its first successful installation. Do not fetch runtime
assets from a CDN. Opening an arbitrary HTML file through file:// is not the
standalone distribution contract; use a secure-origin web app or installed PWA.
Whether an AI model itself works offline depends on the agent client and model,
not on the whiteboard. A local model and compatible client can provide that part.

Shared mode serves the frontend and API from the same HTTPS origin. A server
inside a Tailscale network is a supported deployment. Use trusted HTTPS
termination, such as Tailscale Serve, so browser features can run in a secure
context. Network membership limits reachability; application sessions and room
permissions still control who can read and draw. Static GitHub Pages hosting
serves local mode; it cannot host the Rust collaboration server.

Each connected agent opens the room in its own compatible browser and invokes
that page's WebMCP tools. The server synchronizes board operations; it does not
host or orchestrate the agents' language models. A separate headless MCP bridge
may be added later if a client cannot operate through a browser.

A disconnected shared room becomes read-only. Preserve its last committed view
and mark it disconnected; never silently accept edits into a divergent local
copy and claim they reached the room. Offer an explicit local fork when someone
wants to continue drawing. Rejoining or importing that fork is an explicit action.
Automatic offline merging is outside the first collaboration milestone.

## Architecture and repository layout

Use idiomatic Rust for the shared drawing model, validation, command execution,
and protocol. Compile the browser application with rust-js; run the server as
native Rust. React manages application UI, and Canvas 2D renders the board.
Use Bun for package management, build scripts, and browser tests.

Keep everything in this repository until real consumers justify extraction:

```text
agent-whiteboard/
  crates/
    board-core/       Geometry, elements, brushes, commands, events, undo rules
    protocol/         Versioned request, receipt, snapshot and stream types
    server/           Native room authority, HTTP, WebSocket, SQLite
    web/              rust-js UI, local authority, Canvas renderer, WebMCP
  web/                Entry point, styles, fonts, service worker and assets
  tests/              Native/JS parity, storage contracts, browser acceptance
  docs/               Design and decisions
  rust-js/            Submodule pinned to a published compiler commit
```

The initial crate split may be smaller. Preserve these responsibilities even if
protocol types start beside board-core. Browser APIs, storage, React, and network
code must not enter the drawing rules.

```text
Human pointer input                 AI agent through WebMCP
         |                                   |
         +------------ BoardSession ---------+
                            |
                 +----------+----------+
                 |                     |
           Local authority       Remote session client
                 |                     |
          Shared board rules      HTTPS commands
                 |                     |
             IndexedDB          Native room authority
                                       |
                                Shared board rules
                                       |
                                     SQLite
                                       |
                              WebSocket accepted events

Accepted state ---> Canvas renderer ---> Region captures and exports
Draft strokes and cursors ---> Separate transient overlay
```

BoardSession exposes snapshot/query, submit, and subscribe. A storage abstraction
alone is insufficient: remote authority must validate edits and order concurrent
commands. UI and WebMCP use this same session contract, rather than writing to
IndexedDB or sending unrelated drawing instructions directly to a canvas.

AgentCube provides patterns worth reviewing: its published rust-js submodule,
typed browser/library bindings, atomic receipts, and real WebMCP browser tests.
Copy only pieces that fit the new board contract, retain required licenses, and
keep them local initially. Do not copy cube-specific state or visual queues.
Compiler gaps should become upstream issues or fixes, not restrictions on what
the whiteboard can draw. Any temporary downstream patch must be explicit,
tested, tracked in this repository, and tied to an upstream issue.

## Canonical board model

The editable document is a scene of vector elements, not a screenshot or a list
of Canvas API calls. Persist geometry, style, order, transforms, and metadata.
Rendering caches are disposable and can always be rebuilt from the document.

A board contains its ID, schema version, frame, background, accepted sequence,
and ordered elements. Each element has a stable ID, version, creator, creation
sequence, affine transform, optional group ID, and a typed payload:

| Element | Canonical content |
|---|---|
| Ink | Sample points, optional pressure, brush specification and brush version |
| Path | Typed move/line/quadratic/cubic/close commands, fill and stroke style |
| Text | Plain text, position, font asset ID, size, alignment and layout bounds |

Convenient shapes lower to editable paths through shared geometry functions.
Do not persist a competing shape representation with separate rendering rules.
Groups share an ID for selection; transforming a group atomically updates its
members, without introducing a second transform hierarchy in version one.

Coordinates and transforms use finite f64 values with a documented world
coordinate system: x increases rightward and y downward. Reject NaN, infinity,
invalid pressure, noninvertible transforms, and out-of-range values. JSON decoding,
geometry normalization, and validation have one owner in board-core. Original
stroke samples remain available; derived smoothing and outlines carry their
algorithm version. Viewport changes never rewrite geometry.

Brushes define color, width in world units, opacity, and pressure response.
No pressure means constant width, not a missing or invisible stroke. Build a
filled outline for pressure ink through the shared geometry implementation;
Canvas drawing, hit testing, bounds, erasing, and SVG export consume that result.
Union a stroke's overlapping outline before applying opacity so a translucent
marker does not darken itself at every sample. Define path fill rules explicitly.
Use bundled, licensed fonts and test text layout against export; cross-platform
pixel identity is not assumed.

### Partial erasing

Represent erasure as editable masks attached to specific existing elements.
An erase operation includes its swept path, radius, target IDs and expected target
versions. The shared geometry code computes intersections and converts masks into
the targets' local coordinates. Masks follow their targets when artwork moves.
Masks affect only those targets; later ink remains intact. Selection deletion
removes whole elements and is a different operation.

The renderer applies masks inside each element's isolated compositing surface,
then composites that element in board order. Erasing the top mark therefore
reveals lower marks, including their original colors. SVG export uses equivalent
masks. Store erase provenance so undo can remove an author's erasure without
restoring a stale copy of the entire board. If bounded compositing or vector
masking is not yet correct, ship whole-element deletion first and label partial
erasing as unfinished; never substitute white paint.

## Input and rendering

Use Pointer Events for mouse, touch, and stylus. Capture the primary drawing
pointer, consume available coalesced samples, and convert screen positions into
world coordinates through the inverse viewport transform. Pointer cancellation
ends the preview safely. Touch navigation and drawing must have explicit modes
so a two-finger gesture cannot accidentally commit ink.

Render a draft immediately on a transient overlay, then submit a finished stroke
as one undoable command. The final accepted stroke replaces the preview only
after commit. A failed write retains a clearly marked unsaved draft for retry
or discard. Presence and unfinished remote strokes are ephemeral, rate limited,
and expire after disconnection; they are never mistaken for saved artwork.

Keep one renderer alive while board state, selection, viewport, and tools change.
Use requestAnimationFrame to coalesce repaints, spatial bounds to cull offscreen
objects, and bounded caches for expensive paths and masks. Render the selection
and participant cursors separately. Drawing-tool state and camera state are local
preferences; they do not change the shared document.

Canvas 2D is the initial renderer. Editable vector geometry supplies resolution
independence and SVG export. Consider worker rendering or another GPU renderer
only after measuring concrete bottlenecks. Decorative draw-on playback can be
added later; direct tool batches should paint promptly and should not serialize
unrelated agents behind long animations.

## Commands and persistence

A mutation contains protocol version, board ID, command ID and typed operations.
The session supplies actor identity; a tool cannot impersonate another actor by
passing a name. Generate element and command IDs before submission and retain
them for retries. Accept a batch atomically or reject it without partial changes.

The authority validates against current state and prepares an event, then commits
state changes, the next sequence, an accepted event, and the retry receipt in one
transaction. Publish only after commit. Bind deduplication to board, authenticated
actor, command ID and normalized request content. Same ID and same content returns
the original receipt; same ID and different content is a collision error.

The receipt contains the accepted sequence and resulting element versions.
Separate current presentation metadata from that immutable receipt. A retry may
refer to an older sequence; it must not rewind the board or replay the edit.
Cancellation before acceptance aborts without changes. After commit, a lost or
cancelled response is recovered by retrying the identical command ID.

IndexedDB provides local snapshots, events, receipts and schema migrations.
Serialize local acceptance with read/write transactions, including across browser
tabs. BroadcastChannel is a refresh hint; durable state remains authoritative.
SQLite provides the corresponding room transaction on the server. Event replay
from a versioned snapshot must reconstruct the same document on both targets.
Reject unsupported future schemas without overwriting them. If browser storage
is unavailable, show temporary mode and offer export; do not silently switch a
previously durable session to memory after a failed write.

Start with JSON board import/export including schema version and embedded asset
references. PNG export is for viewing; SVG preserves vector artwork; a board file
preserves editability and supported undo metadata. Validate imported contents
before replacing or adding anything. Room URLs identify a room and require its
normal authorization; do not put credentials or the full drawing in the URL.

## Collaboration and undo

A server assigns one monotonically increasing sequence to each accepted batch
within a room. Clients consume that order, buffer gaps, and request missing events.
Independent creates need no global expected sequence, so two agents can draw in
different areas concurrently. Their final overlap order follows server acceptance
order, not their machine clocks. Multi-element drawings can be one atomic batch.

Edits, deletions, reorders, and erasures carry expected versions for their targets.
If two actors edit the same version, one succeeds and the other receives a
structured conflict with current versions. Do not silently overwrite changes.
Reorders name a stable neighboring element ID and validate that anchor as well
as the moved elements; never address changing array indices as object identity.
Room-wide clear and frame changes also require an expected board sequence.
A committed clear is undoable subject to subsequent-edit conflict rules.

Undo and redo are new accepted commands; never rewind the room sequence. Undo
addresses the requesting actor's most recent eligible command, not another
participant's last edit. Apply an inverse only when its target preconditions
still hold. If someone has changed the object since, return a conflict and keep
the board unchanged. Undoing an erase removes its masks, not other actors' masks.
History actions run alone, rather than inside a mixed batch. Return exactly
which action was undone or redone. A CRDT is not required for this initial
server-authoritative model; revisit it if automatic offline merging becomes a
product requirement.

On reconnect, send the last applied sequence. Recover with later events, or a
consistent snapshot plus its subsequent events. Version one retains accepted
history, command identities and receipts for the board's lifetime; snapshots are
checkpoints, not permission to discard retry protection. At the configured room
quota, reject new writes with an actionable export/archive message rather than
silently losing history. A future compaction design must specify undo retention
and how expired command identities are rejected before deleting any receipts.

Use room roles: owner, editor, viewer. All checks happen at the authority.
Presence is advisory and does not lock the board. A batch can fail because of
a conflict; another artist's cursor being nearby is not a reason to reject it.

## WebMCP contract and visual feedback

WebMCP exposes page tools backed by BoardSession. It supplies structured access
to browser-side application logic; the shared-mode server remains the authority.
The browser API is experimental, so isolate registration and result formatting
in one adapter, feature-detect it, and pin a tested browser/tooling combination.
Without it, human drawing still works and the UI clearly reports that agent
tools are unavailable. Do not present a private window object as native WebMCP.
See the [WebMCP draft](https://webmachinelearning.github.io/webmcp/) and
[Chrome overview](https://developer.chrome.com/docs/ai/webmcp).

Use a small, expressive tool surface:

| Tool | Purpose |
|---|---|
| get_board | Board ID, frame, current sequence, permissions, limits and viewport |
| get_elements | Paged element geometry and versions, optionally filtered by region or IDs |
| get_view | A rendered region plus sequence, world bounds, pixel size and coordinate mapping |
| apply_operations | One atomic batch of create, edit, delete, mask, reorder or history operations |
| set_viewport | Pan, zoom or fit the caller's view without changing the document |
| export_board | Export a bounded image, SVG or editable board file without uploading it |

Schemas describe exact units, coordinate origin, path commands, styles, limits,
preconditions and examples. Use typed path data rather than executable drawing
scripts, arbitrary HTML, or arbitrary SVG markup. Rich text, scripts, external
asset URLs and network-fetch tools are not part of the initial contract.

Paged get_elements queries pin one snapshot sequence in their continuation cursor.
If that snapshot expires, return an explicit restart response instead of mixing
elements from several document versions across pages.

An agent needs both geometry and sight. get_view renders from an immutable board
snapshot, returns that snapshot's sequence, and includes the pixel-to-world mapping.
Return PNG image content through the tested client-supported result format;
verify that the intended agent client actually receives and can inspect it.
A JSON image descriptor alone is not proof of visual feedback. Keep get_elements
available for exact edits, but do not require the agent to reconstruct the entire
picture mentally from thousands of coordinates.

After a successful tool mutation, await presentation of at least its accepted
sequence in the caller's visible page before reporting presented=true. Other
clients need not have painted yet. Background pages may return the durable receipt
with presented=false; get_view can still render the accepted state offscreen.
No receipt may claim that the server committed something when only a preview
exists. Read/capture operations report their sequence so agents can detect edits
that happened while they were planning.

### Drawing a duck

A duck is an illustration assembled from unrestricted geometry, not a special
application command. A normal agent workflow is:

```text
Agent             Browser tools         Local or server authority
  | get_board          |                          |
  |------------------->| read snapshot             |
  |<-------------------| frame, sequence, limits   |
  | apply_operations   |                          |
  |------------------->| validate and commit batch |
  |                    |------------------------->|
  |                    |<-------------------------|
  |                    | paint accepted paths      |
  |<-------------------| receipt and presentation   |
  | get_view           |                          |
  |------------------->| capture committed region   |
  |<-------------------| image and world mapping    |
  | inspect and revise |                          |
```

The first batch could create a yellow curved body, a curved neck and head,
an orange closed path for the bill, a small dark eye, and a wing made from cubic
curves. The agent can then see that the bill is too long, query that path's
version, and shorten it with an edit. A bird in flight uses different paths
through exactly the same tools. The application must not silently replace this
work with a duck stamp or a generated raster image.

Document the simplest draw-path request in the implementation's tool examples,
then add a full duck fixture for regression and a natural-language agent demo.
The fixture proves drawing capability; only the demo proves an agent can use it.

## Limits and trust boundaries

Initial tunable limits are 1 MiB of decoded command JSON, 256 operations and
20,000 total path commands or stroke samples per batch, and absolute coordinates
no greater than 1,000,000 logical units. Captures are capped at 2048 pixels per
side and 4 MiB of encoded image data, with downscale or explicit limit errors.
Expose actual configured limits in get_board; benchmark before expanding them.
Bound room storage, expensive mask work, imports and participant presence queues.
Validate finite numbers and authorization before expensive geometry processing.

Board text and metadata are user content, never instructions granting an agent
new permissions. Treat them as untrusted in tool results. Use HTTPS/WSS for shared
rooms, same-origin requests, authenticated sessions, origin checks on mutations
and WebSocket upgrades, and role checks inside transactions. Tailscale access is
not a replacement for these application checks. Rendering and export must not
execute imported markup or fetch private URLs.

## Verification and quality bar

Prove drawing behavior with native Rust tests, the same tests compiled through
rust-js, and parity fixtures for paths, smoothing, masks, bounds, hit testing,
transforms, serialization and command history. Inspect generated JavaScript;
do not reshape natural Rust just to accommodate a lowering gap.

Use production-bundle Playwright tests through real page.webmcp discovery and
tool calls, as supported by the [Playwright WebMCP API](https://playwright.dev/docs/api/class-webmcp).
Mocks can test application adapters but cannot replace native browser acceptance.
Run checks locally. GitHub Actions should build and deploy the static application,
with current action releases pinned to full commit hashes; keep server deployment
separate. Do not add a separate check workflow unless explicitly requested.

Required regression scenarios include:

- Agent-created curved artwork produces actual colored pixels; text status or an
  element count alone cannot pass. Frozen renderer and ignored-path mutations fail.
- Mouse, touch and pen input keep smooth ink under zoom and device pixel ratio
  changes; cancel, release outside, and renderer remount do not lose completed ink.
- A partial eraser reveals a lower colored stroke, leaves later strokes intact,
  follows transformed targets and survives undo, reload and SVG export.
- IndexedDB and SQLite satisfy the same atomicity, duplicate retry, collision,
  transaction failure, recovery and future-schema preservation contracts.
- Several isolated browser contexts create concurrent strokes without losses;
  same-object conflicts, actor-specific undo, disconnect and reconnect are explicit.
- Agent captures contain the committed drawing and correct coordinate mapping.
  A real agent draws and revises a duck using only registered tools and visual feedback.
- Installed local mode opens with the network disabled, uses bundled assets, and
  retains finished drawing after reload. Missing WebMCP does not break human input.

Provisional performance targets on documented reference hardware are a responsive
60 Hz ink preview with 10,000 visible simple strokes, local pointer-to-preview
latency below 32 ms at the 95th percentile, and durable edit visibility below
250 ms at the 95th percentile for five active agents on a healthy local network.
Record hardware, brush complexity, payload sizes and storage costs. These are
engineering targets, not measured claims or reasons to silently drop geometry.

## Implementation milestones

| Milestone | Deliverable | Acceptance gate |
|---|---|---|
| 1 Drawing foundation | Pinned rust-js submodule, Canvas renderer, shared paths/ink, local session, marker, pan/zoom, native WebMCP create/read/capture, atomic local persistence | A real agent draws a recognizable duck from strokes/curves, sees its image, revises it, and reload preserves it; production-browser pixel tests pass |
| 2 Whiteboard editing | Selection, transforms, groups, partial eraser, history, text and JSON/PNG/SVG export | Erasing, undo and exports preserve layered artwork and exact editable geometry |
| 3 Standalone offline app | Bundled assets, service worker, versioned cache updates and recovery UX | Installed application works with network disabled, including reload and export; local agent connectivity assumptions are documented |
| 4 Shared rooms | Native Rust server, SQLite authority, HTTPS, roles, ordered events, presence and recovery | Five independent agent/browser sessions draw together, survive retries and reconnect, and cannot overwrite conflicting edits silently |
| 5 Scale and polish | Profiled rendering caches, accessible controls, connection/saving UX, optional textured brushes and larger-board testing | Published performance measurements and regression fixtures support the chosen limits |

Milestone 1 includes the serverless local runtime, but not the stronger promise
of reopening without network before milestone 3's asset-cache acceptance passes.
Do not label local mode fully offline before that gate. Finish each milestone
vertically: shared rules, storage, UI, native WebMCP and visible acceptance.

## Decisions and later options

The initial decisions are vector-first drawing, Canvas 2D, a shared Rust command
model, IndexedDB locally, SQLite and ordered server operations online, and native
WebMCP at the page boundary. Keep all code in one repository and extract only
when there is a proven reusable consumer.

Later options are raster imports, pressure/textured brush expansion, automatic
offline merging with a CRDT, a headless MCP bridge, and larger deployments.
Choose these from observed needs. None is required to let an agent draw arbitrary
freeform artwork or to let several agents share the first online whiteboard.
