# Schafkopf machine learning

This directory is a standalone Python project intended to move to its own
repository. It imports no parent-directory Python modules and contains no game
rules. Supply a matching `replay_dataset` executable built from the logic repo.
No nested Git repository is initialized here.

The Rust library owns rules, public/private observations, recording schemas,
replay validation, and baseline NPCs. This project owns features, dataset splits,
Parquet conversion, model training, and checkpoints. Generated data and models
are ignored by Git.

## Prepare the tools

In the logic repository:

```bash
nix-shell --run 'cargo build --release --example self_play --example replay_dataset'
```

For Python, either use this directory's Nix environment:

```bash
cd machine-learning
nix-shell
```

Or create a Python 3.11+ virtual environment and run `pip install -e .`.
The examples below run from `machine-learning/`. Set the executable path to the
matching engine build; after moving the project, this path can point anywhere.

```bash
export SCHAFKOPF_REPLAY_BIN="../target/release/examples/replay_dataset"
```

## Validate and prepare

Both historical full-v1 JSONL and new compact-v2 JSONL are accepted, including
multi-member gzip. Rust validates every declaration, legal play, result, and
stored full observation/event. Unknown rule versions fail; use the matching
engine instead of silently interpreting records under changed rules.

```bash
python -m schafkopf_ml.dataset validate ../games/npc-run-003.jsonl.gz \
  --replay-bin "$SCHAFKOPF_REPLAY_BIN" --max-games 1000

python -m schafkopf_ml.dataset prepare ../games/npc-run-003.jsonl.gz \
  --replay-bin "$SCHAFKOPF_REPLAY_BIN" --max-games 50000 \
  --output data/first-model
```

For sharded collections, pass all shard filenames (shell globs work), excluding
manifest files. When a collection manifest is available, add
`--manifest ../data/npc-v2.jsonl.gz.manifest.json` to verify completion and SHA-256
checksums before conversion. This makes an extra streaming pass over selected
input files and preserves collection provenance in `dataset.json`. Historical
archives without manifests still work, but their original build cannot be
identified automatically. The output must be a new directory. Omit `--max-games` to scan
all input. A limit applies to input games, including duplicate episodes.

The converter streams one game at a time, deduplicates episode content with a
temporary on-disk SQLite index, and writes bounded Zstandard-compressed Parquet
shards. SQLite is an internal deduplication index, not the training format.
Default shards contain at most 100,000 decision rows; `--shard-rows` overrides it.
`dataset.json` appears only after successful conversion. A failed conversion may
leave partial shards; do not train from them or reuse that output directory.

Each Parquet row contains stable deal/game IDs, turn, player, mode, 471 float
features, a 32-card legal mask, target card ID, and a terminal +1/-1 outcome for
the actor. The outcome is metadata, not a feature or the imitation training
objective. It is not a tariff payout. `declarations.jsonl.gz` holds exact bidding
inputs and choices with the same split assignments for later bidding training.

Splits are 80/10/10 by SHA-256 of a canonical deal grouping ID, not randomly by
row. All policies, repeated runs, seat permutations, leaders, and hand orders of
the same four hands stay together. Exact repeated episodes are skipped. Counts
and mode coverage are included in the conversion report. Small inputs may have
empty splits; the trainer requires all three.

## Train the first model

```bash
OPENBLAS_NUM_THREADS=1 python -m schafkopf_ml.train data/first-model \
  --output artifacts/first-model --epochs 5
```

This is a NumPy CPU baseline: 471 inputs, a 128-unit ReLU hidden layer, and 32
card logits. Cross entropy imitates the recorded policy; illegal logits are
masked before normalization. It uses mini-batch SGD and bounded Parquet row-group
loading. Shuffling happens across files, row groups, and rows within groups;
it is not a full in-memory global shuffle. The checkpoint with lowest validation
loss is tested once and saved as `model.npz`, with configuration and metrics in
`training.json`. Reports distinguish overall accuracy from decisions with more
than one legal action and include a lowest-legal-card-ID baseline.

Checkpoint arrays are `w1`, `b1`, `w2`, `b2`, plus `feature_version` and
`input_size`. Inference is `relu(features @ w1 + b1) @ w2 + b2`, followed by
legal-action masking and argmax. Load with `numpy.load(..., allow_pickle=False)`.
This is a supervised imitation baseline; playing strength requires separate game evaluation.

## Feature contract

See [FEATURES.md](FEATURES.md). Changes to card IDs, block layout, seat rotation,
normalization, or public-information semantics require a feature-version bump.
Original archives remain independent of the chosen feature encoding.

## Tests

Build the Rust examples first, then from this directory. After moving this
project, set `SCHAFKOPF_LOGIC_ROOT` to the logic checkout for the integration test:

```bash
nix-shell --run 'python -m unittest discover -s tests -v'
```

Tests exercise collection/sharding, legacy conversion, exact replay, content
hashes, cross-format deduplication, split isolation, feature input boundaries,
corruption rejection, checkpoint creation, and learning on a small masked task.
