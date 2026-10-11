# Lidarr for lemonfiber

**Watches for music and fetches it.**
Lidarr is one of the parts the stack lemonfiber ships is built from.
Without it: Find and download music yourself.

## How lemonfiber carries it

The default bundle in
[lemonfiber-plugins](https://github.com/lemonfiber/lemonfiber-plugins) pins this
repository by a revision and the digests of its manifest and its tree, and
lemonfiber compiles that tree in.
Until its adapter ships, lemonfiber assembles Lidarr into the stack it ships as a
stack service, and reaches it through the in-process adapter its `api` names.
Its image is Lidarr's own, so the release train never tags this repository, and
its pin moves by a reviewed pull request on the catalogue.

## What is here

| Path | What it is |
| --- | --- |
| `plugin.toml` | The part's manifest |
| `recordings/lidarr/` | The responses its claims are proved against |
| `targets.toml` | The lemonfiber its checks run against |
| `proofs.json` | What those checks said, committed |

## Checks

The plugin verdict is one job of spec's
[`first-party-plugin`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-plugin.yml)
workflow: the manifest as the lemonfiber `targets.toml` names reads it, every
claim against its recording, and every pinned digest. This repository holds no
Python of its own. `just ci` runs the gates that need nothing but this tree.

## Licence

The data in this repository is under the licence in `LICENSE`.
Lidarr itself is under GPL-3.0-only and is not distributed here: this
repository names an image, it does not contain one.
