# Releasing the working state

Only a **published hosted release** triggers `.github/workflows/release.yml`.
Pushing `main` or a tag alone is not its publication trigger. The existing
workflow sets the Cargo version from the release tag, then publishes to the
Forgejo registry and crates.io. The source manifest intentionally stays `0.0.0`.

Before creating a release such as `0.2.0`:

1. Review `CHANGELOG.md` and the documented house rules/API compatibility notes.
2. Run `cargo test --all-targets` and `cargo test --doc`.
3. Build the Rust examples used by Python:
   `cargo build --release --examples`.
4. From `machine-learning/`, enter its Nix shell and run
   `python -W error::ResourceWarning -m unittest discover -s tests -v`.
5. Run `cargo package --list` and `cargo package` from a clean checkout.
   The Rust package must contain no datasets, weights, Python environment, or
   scratch main. `main.rs` is a local test file and intentionally ignored.
6. Check that the repository secrets `CONTAINER_REGISTRY_TOKEN` and
   `CARGO_REGISTRY_TOKEN` are configured and have permission for their registries.
   Local package checks cannot verify these hosted-runner permissions.
7. Publish a hosted release pointing at the reviewed commit, with the desired
   version as its tag and the relevant change notes. Observe the workflow runs
   and both registry publication results.

The two registries publish sequentially; a workflow failure can leave one
published and the other pending. Check which upload succeeded before retrying:
registry releases are immutable and should not be blindly republished.

This is a pre-1.0 working-state release of the supported house-rule engine, not
an assertion that all regional Schafkopf variants or tournament/payment rules
are implemented. The Python project's own version and model checkpoints remain
separate from the Rust package version. Publishing this crate does not publish
trained weights or the companion frontend application.
