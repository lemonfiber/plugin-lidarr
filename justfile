# Task runner for a bundled part. `just` with no argument lists these.
default:
    @just --list

# Turn on the repository's own git hooks. Once per clone.
hooks:
    git config core.hooksPath .githooks
    @echo "hooks on: .githooks/commit-msg, .githooks/pre-push"

# The gates that need nothing but this tree. The plugin verdict runs in CI.
ci: hooks typos links

# Spell check.
typos:
    typos

# Link check.
links:
    lychee --no-progress .
