# Downstream rust-js patches

AgentWhiteboard pins a published upstream commit through its rust-js Git submodule.
The following fixes stay in this repository until the upstream issues are resolved:

| Patch | Upstream issue | Purpose |
|---|---|---|
| 0001-preserve-unit-callback-results.patch | [#1685](https://github.com/rust-js-lang/rust-js/issues/1685) | Keep spawning unit callbacks synchronous so React sees undefined, not a Promise cleanup |
| 0003-typed-canvas-contexts.patch | [#1698](https://github.com/rust-js-lang/rust-js/issues/1698) | Give typed Canvas context requests precise nullable return types without unsafe DOM-class discrimination |
| 0002-find-submodule-worktrees.patch | [#1686](https://github.com/rust-js-lang/rust-js/issues/1686) | Find the actual submodule checkout in process-check tooling |

Setup, compilation, and local checks call scripts/patch-rust-js.ts. It verifies
that the checked-out commit matches the repository's gitlink, applies patches in
order, accepts an already-applied patch, and fails clearly on a conflict. It does
not reset the checkout or silently discard local edits. Git may show rust-js as
modified after setup: those tracked modifications are the expected applied patches.
No commit inside the submodule is needed.

The first two patches retain AgentCube’s upstream regression coverage. The Canvas patch adds a binding regression for typed and dynamic requests, nullable results, and foreign-realm context objects. That regression passes locally. AgentWhiteboard’s native/JavaScript domain tests and production-browser tests exercise the patched compiler as a consumer. This does not claim the entire compiler suite was run for this application.

When updating the submodule, review the upstream issues first. Remove a patch
only after the pinned upstream commit includes its fix and the relevant app and
regression checks pass. Rebase remaining patch files deliberately when upstream
changes their context. Do not replace the published pin with an unpublished
local commit.
