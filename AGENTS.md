# AGENTS.md — plugin-lidarr

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

## What this repo is

Lidarr as a first-party lemonfiber plugin: `plugin.toml` and the adapter
that speaks the `library.curate@1` contract in front of Lidarr. The adapter is the
Rust crate at the root, built on `lemonfiber-adapter-kit` from the core and on
`adapter-kit-servarr`, the Servarr client the Servarr adapters share, each at the
commit `Cargo.toml` pins, into the image the `Dockerfile` builds. `release.yml`
publishes that image from every commit on `main` and from the train's tag. The
[bundle contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/bundle.md)
says how the catalogue pins this repository and how lemonfiber carries it.

`conformance/` holds one recording per case of the contract the adapter speaks.
`lemonfiber plugin record .`, at the commit `targets.toml` names, writes them
against a throwaway Lidarr at the digest `plugin.toml` pins and the adapter
it pins; CI takes every recording again with the adapter built from this tree.
Re-record whenever the adapter's answers or either digest change.
`tests/conformance.rs` reads every recording with the core's own client.

## The rules you cannot break

- **The plugin id is the service's id** (`ARCH-R266`). The container, the
  configuration directory, the settings and the journal keep their names.
- **Its proofs' recordings sit under `recordings/lidarr/`**
  (`ARCH-R271`).
- **Every service declares `reaches` and `asks_for`** (`ARCH-R270`).
- **The reader is spec's `first-party-plugin` workflow's,** and lemonfiber
  records the recordings itself. There is no Python here.
- **No `unwrap`, `expect`, `panic` or indexing** outside tests. The lints in
  `Cargo.toml` are the core's.
- **What the Servarr adapters share lives in `adapter-kit-servarr`**
  (`REPO-R99`). Only what Lidarr alone answers is written here.
- **The adapter never writes its credential.** lemonfiber mints the key Lidarr
  answers to, hands it to Lidarr as `LIDARR__AUTH__APIKEY`, and writes it into the adapter's
  `upstream.json` as `api_key`, as the adapter's `upstream` maps it
  (`ARCH-R315`, `ARCH-R317`); the adapter reads it for every call and never logs
  it.
- **The core's crates move with the train.** The `rev` in `Cargo.toml` is the
  core commit `targets.toml` names, the same for every `lemonfiber-*` crate and
  the one `adapter-kit-servarr` names at its own `rev` (`REPO-R100`).
- **Moving Lidarr's digest** means recording every proof's response and
  conformance case again against the new image in the same change, moving
  `RELEASE` and `DIGEST` in `src/adapter.rs` with it, and committing the
  `proofs.json` the checks write.

## Checks

```
just ci
```

The gates that need nothing but this tree. The plugin verdict and the
conformance recordings run in CI; `just recordings` takes the recordings again
locally, with Docker.
