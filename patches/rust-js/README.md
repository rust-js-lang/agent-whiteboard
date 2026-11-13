# Downstream rust-js patches

AgentWhiteboard pins a published upstream commit through its rust-js Git submodule.
The following fixes stay in this repository until the upstream issues are resolved:

| Patch | Upstream issue | Purpose |
|---|---|---|
| 0004-react-compiler-compatible-output.patch | [#1702](https://github.com/rust-js-lang/rust-js/issues/1702), [#1703](https://github.com/rust-js-lang/rust-js/issues/1703), [#1705](https://github.com/rust-js-lang/rust-js/issues/1705), [#1706](https://github.com/rust-js-lang/rust-js/issues/1706) | Preserve lexical callback forms and parameter shadows; centralize mutable-place handles so React Compiler can optimize generated components and hooks; retain check failure logs immediately |
| 0001-preserve-unit-callback-results.patch | [#1685](https://github.com/rust-js-lang/rust-js/issues/1685) | Keep spawning unit callbacks synchronous so React sees undefined, not a Promise cleanup |
| 0003-typed-canvas-contexts.patch | [#1698](https://github.com/rust-js-lang/rust-js/issues/1698) | Give typed Canvas context requests precise nullable return types without unsafe DOM-class discrimination |
| 0002-find-submodule-worktrees.patch | [#1686](https://github.com/rust-js-lang/rust-js/issues/1686) | Find the actual submodule checkout in process-check tooling |

Setup, compilation, and local checks call scripts/patch-rust-js.ts. It verifies
that the checked-out commit matches the repository's gitlink, applies patches in
order, verifies an already-applied stack in a disposable copy, and fails clearly on a conflict. It does
not reset the checkout or silently discard local edits. Git may show rust-js as
modified after setup: those tracked modifications are the expected applied patches.
No commit inside the submodule is needed.

The first two patches retain AgentCube’s upstream regression coverage. The Canvas patch adds a binding regression for typed and dynamic requests, nullable results, and foreign-realm context objects. That regression passes locally. AgentWhiteboard’s native/JavaScript domain tests and production-browser tests exercise the patched compiler as a consumer. This does not claim the entire compiler suite was run for this application.

When updating the submodule, review the upstream issues first. Remove a patch
only after the pinned upstream commit includes its fix and the relevant app and
regression checks pass. Rebase remaining patch files deliberately when upstream
changes their context. Do not replace the published pin with an unpublished
local commit.

React integration verification: four targeted compiler regressions pass, including native JSON mutation comparison. The app check verifies actual memoization of App, Icon and useTools with no diagnostics. The production-browser suite passes with automatic optimization enabled. Compiler cell, serde-value, function and standard-library suites also pass.

Additional local compiler proof: all 14 native-comparison corpus parts pass; the affected output/architecture/tooling group passes 101 tests; all three new mutations and the existing mutable-field mutation are caught. Snapshots, pilot output, and the readability baseline were regenerated after narrowing the parameter-copy change. The full compiler check reached its 600-second limit (exit 124); this is not a clean full-suite result. Its completed snapshot, pilot and readability failures were corrected and rechecked separately. The batched guide generator separately fails to find serde while its isolated page passes, tracked as [#1711](https://github.com/rust-js-lang/rust-js/issues/1711).
