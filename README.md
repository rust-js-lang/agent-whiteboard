# AgentWhiteboard

A freeform whiteboard where humans and AI agents draw together. Agents use
WebMCP to make strokes, draw curves, erase, inspect the result, and continue.

The planned application has two modes: standalone local use with browser storage,
and shared rooms hosted by a server, including inside a Tailscale network.
The frontend and shared drawing rules will use Rust through rust-js.

**Status:** design stage; the application is not implemented yet.

See the [design and implementation plan](docs/design.md) for the drawing model,
WebMCP interface, offline behavior, collaboration rules, and milestone acceptance
criteria. All application code will stay in this repository initially.
