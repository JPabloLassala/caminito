# 1. Build Camino in Rust on Smithay

Date: 2026-09-28
Status: Accepted

## Context

Camino is the Wayland counterpart of Marco, and MATE is written entirely in C. The usual C foundation for a compositor is wlroots, which Sway, labwc and Wayfire use. We want to write Camino in Rust. wlroots has no maintained Rust bindings: `wlroots-rs` was abandoned in 2019, and later attempts are partial.

## Decision

Camino is written in Rust on top of Smithay, the compositor library used by COSMIC and niri.

## Consequences

- There are no FFI bindings to maintain, and we get idiomatic Rust throughout.
- Smithay provides building blocks rather than finished pieces. Things wlroots hands you, such as a scene graph and output management helpers, may have to be written by us.
- It's harder to upstream to MATE, whose maintainers work in C. That's acceptable for now, because Camino is a personal project kept upstream-compatible only where it's cheap (licence GPL-2.0-or-later).
- Rejected: C + wlroots (not Rust), Rust over self-maintained wlroots FFI (maintenance burden), forking labwc or Mutter (C, and a foreign architecture).
