# Lidarr for lemonfiber

**Watches for music and fetches it.**
Lidarr is one of the parts the stack lemonfiber ships is built from.
Without it: Find and download music yourself.

## How lemonfiber carries it

The default bundle in
[lemonfiber-plugins](https://github.com/lemonfiber/lemonfiber-plugins) pins this
repository by a revision and the digests of its manifest and its tree, and
lemonfiber compiles that tree in.
lemonfiber assembles Lidarr into the stack it ships as a stack service, and
reaches it through the in-process adapter its `api` names.

This repository also holds the adapter that speaks lemonfiber's `library.curate`
contract in front of Lidarr: a Rust crate built on the core's adapter kit and on
[adapter-kit-servarr](https://github.com/lemonfiber/adapter-kit-servarr), into the
image `ghcr.io/lemonfiber/lidarr-adapter`, published from every commit on `main`
and from each tag the release train cuts.

## What is here

| Path | What it is |
| --- | --- |
| `plugin.toml` | The part's manifest |
| `recordings/lidarr/` | The responses its claims are proved against |
| `src/` | The adapter |
| `conformance/library.curate@1/` | The adapter's answer to each conformance case, in front of the pinned Lidarr |
| `Dockerfile` | The adapter's image |
| `targets.toml` | The lemonfiber its checks run against, and the core the adapter is built on |
| `proofs.json` | What those checks said, committed |

## Checks

The plugin verdict is one job of spec's
[`first-party-plugin`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-plugin.yml)
workflow: the manifest as the lemonfiber `targets.toml` names reads it, every
claim against its recording, and every pinned digest. The adapter is held to its
format, its lints, its tests, full line coverage and its dependency policy, its
image runs as lemonfiber runs it, and `recordings` takes every conformance case
again against the pinned Lidarr. `just ci` runs the gates that need nothing but
this tree.

## Licence

The code and data in this repository are under the licence in `LICENSE`.
Lidarr itself is under GPL-3.0-only and is not distributed here: this
repository names an image, it does not contain one.
