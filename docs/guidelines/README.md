# Guidelines — claude-pretool-sidecar

This directory contains scoped guideline documents for the project. Below is a summary of the core principles; see individual files for details.

## Core Design Principles

1. **KISS** — Keep It Simple, Stupid. The core binary does one thing: aggregate votes from providers.
2. **UNIX Philosophy** — Small, composable tools. The sidecar is one tool; logging is another; policy checking is another.
3. **Composability** — Users compose their pipeline from independent pieces via config.
4. **Configurability** — Users are scripters/programmers. Let them hook up anything via stdio.
5. **Tests as Documentation** — Inspired by Knuth's literate programming. Tests explain behavior; reading them teaches how the tool works.

## Guideline Files

* `testing.md` — Testing philosophy and practices
* `rust-conventions.md` — Rust coding conventions for this project
* `composability.md` — How tools compose together

*(Files added as guidelines are established)*

## Quick Reference to Design Docs

Detailed design decisions live in `docs/design/`:

* `voting-quorum.md` — Vote aggregation algorithm and quorum rules
* `stdio-protocol.md` — JSON-over-stdio provider communication protocol
* `configuration.md` — Config file format, locations, and schema
* `architecture.md` — System architecture and Claude Code integration *(pending research)*
