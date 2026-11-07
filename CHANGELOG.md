# Change notes

## Unreleased — working-state snapshot

### Gameplay and public API

- Complete eight-trick games with turn/ownership/following validation and structured results.
- Sauspiel called-ace rules, partnership visibility, and Davonlaufen.
- Scoring for point games, the documented house Bettel variant, Ramsch, and Durchmarsch.
- One-pass declarations, hand ratings, and an explicitly recorded force-contract override.
- Public trick opener, outcome-group summaries, partial-trick evaluation, and hand sorting.
- Serializable observations/actions, stable card IDs, and checked deal reconstruction.
- Deterministic heuristic NPCs with public-information tactics and a first-legal baseline.

### Data, agents, and evaluation

- Full and compact replay-validated recordings, including legacy and forced games.
- Parallel headless self-play, direct gzip output, bounded batches, sharding, run manifests, and checksums.
- Movable Python project for validation, Parquet preparation, grouped splits, and an imitation MLP.
- Versioned stateless HTTP agent contract and a checkpoint-serving implementation.
- Paired model arena with held-out-deal checks, reusable exact suites, and deal-clustered confidence intervals.
- Sampled decision assessments based on player information, separate from actual outcomes.

### Compatibility and limits

This snapshot extends the early deck/trick API substantially. `GameConfig` needs
a declarer, matches now finish after eight tricks, and errors, phases, events,
and observations have additional variants/fields. Exhaustive matches and struct
literals in older consumers may require updates. The legacy `player` module is
still available.

Rules are explicitly the documented house variants, including Herz-trump Bettel
and a single-pass declaration phase. There are no tariff payouts, doubling,
full tournament bidding, or match-series management. Heuristic ratings are not
win probabilities. Analysis estimates are not optimal-play proofs. The first
learned model is an imitation baseline, not a claim of expert playing strength.

Record, agent, feature, and evaluation versions are separate from the crate
version. Keep the matching rules implementation with long-lived compact records.
Datasets, checkpoints, local scratch `src/main.rs`, and Python tooling are not
included in the published Rust crate.

The Cargo manifest remains `0.0.0`; the existing release workflow sets the
published package version from the hosted release tag.
