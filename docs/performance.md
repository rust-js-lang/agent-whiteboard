# Renderer performance

Measured 2026-11-14 against the production build at bb0a099. The input is the
same 847-mark, 79-event saved drawing, including gradients, soft ink, texture,
curves and text. Each run uses a fresh Chromium 158.0.8127.18 context,
a 1300×1000 viewport and device scale 2 on the same Mac. Before and after run
sequentially, without simultaneous tests. The fixture SHA-256 and raw values are
in [performance-results.json](performance-results.json).

| Interaction | Before | After | Improvement |
|---|---:|---:|---:|
| Three pan updates | 6.412 s | 0.179 s | 35.9× |
| One stroke, six pointer moves, including save | 22.389 s | 0.385 s | 58.1× |
| Replay requested for 2 seconds | 8.558 s | 2.169 s | 3.9× |

These are local end-to-end measurements, not universal frame-rate guarantees.
The stroke includes automation, transaction validation, persistence and final
presentation. Its measured JavaScript time falls from 17656.2 ms to
8.0 ms. Saving and repainting a changed document still cost more
than showing another pointer sample. Larger boards and complex brushes need
further measurement; the design's 10,000-stroke/60 Hz target is not claimed here.

## What changed

The old renderer cleared its entire path cache at 500 entries. A larger board
therefore rebuilt paths continually: the pan sample constructed 2,520 Path2D
objects; the new sample constructs none. Prepared paths, clip paths, masks and
brush layers now belong to element IDs and their drawing contents. Replacement,
partial replay and masks invalidate the entry; removed entries are pruned.

Translucent marks formerly resized and composited a full viewport for every mark.
The renderer now keeps a workspace aligned to the viewport, clears only the
mark's conservative bounds, copies that region at full opacity to a small
surface, and applies the element opacity once when compositing it. Keeping the
raster origin and opacity operation preserves the original browser pixels.
Text uses the whole viewport because approximate selection bounds are not safe
font extents. Stroke bounds include affine scale and shear.

A separate raster cache holds finished art. New pointer samples copy that image
and paint the current mark. Document sequence, camera mapping, physical size,
fonts and the selected element invalidate the cache. Moving an existing mark
caches the preceding layers and paints the mark beneath its foreground, so layer
order is preserved. Live snapshots are borrowed rather than copied per sample.

All caches are disposable. Persisted geometry, command acceptance, history and
storage formats stay authoritative and unchanged.

## Fidelity and regressions

Before/before controls and before/after comparisons inspect 11,233,750
RGBA pixels across five images: the orange, the whole drawing, affine paths with
erasing/soft clipping/text, and two camera views. Every comparison has **zero
changed pixels**. The comparison tool rejects any changed pixel.

All 38 production Playwright tests pass, as do application checks and the 14
shared tests on native Rust and generated JavaScript. The five new browser
regressions cover a 700-mark cache/work budget, transformed compositing, wide text,
selection preview order, and exact native gradient pixels. The large-board test
fails against the old build because a pan rebuilds all 700 paths.

## CPU and memory limits of these measurements

Idle JavaScript remains negligible: 0.231 ms over the 2-second sample, with
no paints or animation frames. This identifies interaction work, not an idle
background loop. Browser task time also contains profiler/protocol overhead.

After an explicit garbage collection, the measured JavaScript heap is
11.76 MiB before and 12.35 MiB after. Retaining useful caches costs some
memory. These numbers exclude native/GPU canvas buffers and are not whole-machine
memory measurements. System CPU/memory pressure was also present; this profile
does not attribute every machine slowdown to the app.

## Reproduce

Export the editable journal through WebMCP export_board with format json, keeping
the returned data string as a local file. Serve each production build on a
separate local origin. The profiling script changes only an isolated browser
copy of that file, never the source board.

```sh
bun scripts/profile-whiteboard.ts /tmp/board.json /tmp/before http://127.0.0.1:4175/agent-whiteboard/
bun scripts/profile-whiteboard.ts /tmp/board.json /tmp/after http://127.0.0.1:4174/agent-whiteboard/
bun scripts/compare-renderer.ts /tmp/board.json /tmp/pixels http://127.0.0.1:4175/agent-whiteboard/ http://127.0.0.1:4174/agent-whiteboard/
```

Profiling writes metrics, a screenshot and Chrome CPU profiles per interaction.
The comparison writes reference/control/current PNGs and a JSON pixel report.
Run performance measurements separately from tests and builds.
