# Player and model evaluation

The arena measures results over matched games. The decision analyzer estimates
avoidable loss from the information available to a player. These are different
questions: losing is not itself a bad-decision label, and an unlikely win is not
proof of good play. Imitation accuracy in `training.json` measures neither.

## Build

From the logic repo:

```bash
nix-shell --run 'cargo build --release --example evaluation_bridge'
```

Then enter `machine-learning/` and `nix-shell`. All Python commands below run from
that directory. The project remains movable: supply the matching bridge binary
through `--bridge-bin`; Python does not duplicate the game rules.

## Paired model arena

```bash
python -m schafkopf_ml.evaluation.arena \
  --model artifacts/first-model/model.npz \
  --bridge-bin ../target/release/examples/evaluation_bridge \
  --training-dataset data/first-model \
  --deals 10000 --seed 4000000000 \
  --opponents heuristic first_legal mixed \
  --output artifacts/arena-checkpoint-001.json
```

Use a new output filename for each run. The candidate checkpoint is read once,
identified by SHA-256, and never modified. `--candidate heuristic` or
`--candidate first_legal` evaluates a built-in Rust policy instead of a model.
`--baseline` defaults to `heuristic`. Opponent lineups are all-heuristic,
all-first-legal, or a fixed mix of those policies across the other three seats.

Every deal is evaluated with the candidate in all four seats. Opening seats
rotate with the deal index. For each seat and lineup, compare two complete
games differing only in that seat's card policy. The baseline and candidate see
the same hands, opener, contract, teammates, and opponent policies. A changed
move may naturally change later opponent decisions; it is not valid to freeze
those decisions after the histories diverge.

`--deals` counts unique underlying deals, not simulated games. With three
opponent lineups, 1000 deals means **12,000 paired comparisons / 24,000 games**.
Subsequent candidate actions receive only seat observations and opaque protocol
IDs. Shuffle seeds, sequential deal IDs, and exact hands are not exposed through
agent request metadata.

### Card play versus bidding

The default `--scope card-play` freezes all four declarations using the heuristic
before comparing either card policy. This isolates card play.

`--scope full-player` lets the candidate declare/pass and play. The baseline seat
uses its own declarations, and other seats retain their named policies. Contracts
may differ. Reports stratify by the baseline mode/role, explicitly noted in their
metadata. The current NPZ model has no bidding policy, so full-player mode rejects
it rather than silently using baseline bids and calling that a bidding evaluation.
A future HTTP candidate advertising both `act` and `declare` can use this mode.

### Score and uncertainty

Primary metric: **candidate minus baseline `balanced_win_v1` utility**. Winners
share +1, losers share -1, making a soloist's individual outcome different from
one defender's outcome. Games with no winners/all winners are neutral ties for
this artificial metric. It is zero-sum, versioned, and is **not** a Schafkopf
monetary tariff. Win rates remain separate, along with role/mode/opponent splits.
A different payout objective needs a new utility version and separate results.

An unavoidable loss where both policies lose is not counted as a candidate
regression. A loss can still differ in artificial utility in Ramsch if the final
winner/loser grouping changes. Decision quality must be analyzed separately;
secondary card points and tricks remain available in engine pair outcomes.

All seat variants and opponent lineups of the same canonical deal form **one
statistical cluster**. The reported confidence interval is an asymptotic normal
95% ratio-estimator interval with deal-clustered variance. Role strata can have
unequal numbers of comparisons per cluster; the estimator accounts for that.
Intervals are null below two independent clusters and unreliable with very few
clusters. These are uncertainty estimates, not exact small-sample guarantees.

Canonical grouping treats seat permutations and hand orders of the same four
hands as the same deal. A temporary on-disk SQLite index rejects repeated deals.
`--training-dataset` additionally excludes every deal from the prepared dataset,
including its training, validation, and test splits. If absent, the report
explicitly marks training overlap as unchecked. Separate seed ranges alone are
not proof of no overlap.

### Exact benchmark reuse

Each report has a companion `.deals.jsonl.gz` file holding the actual ordered
cards and opening-seat indices. Preserve it with the report. Reuse those exact
hands for another checkpoint:

```bash
python -m schafkopf_ml.evaluation.arena \
  --model artifacts/next-model/model.npz \
  --bridge-bin ../target/release/examples/evaluation_bridge \
  --suite artifacts/arena-checkpoint-001.deals.jsonl.gz --deals 10000 \
  --training-dataset data/next-model \
  --opponents heuristic first_legal mixed \
  --output artifacts/arena-checkpoint-002.json
```

The exact-deal content hash excludes seed labels; replaying a suite does not use
the seed to shuffle. `benchmark_id` includes the actual cohort hash, scope,
baseline/opponents, utility, and bridge binary fingerprint, and excludes the
candidate checkpoint and batching size. Preserve the matching engine build and
configuration. A changed rules/engine build deliberately gets a different ID.
No training or checkpoint-selection run should consume the final reserved test
suite; use a separate evaluation cohort for iterative model selection.

### Throughput and failures

The Rust bridge persists across batches, advances local players in parallel, and
never collects per-turn dataset recordings. Local NPZ inference is vectorized
across active games with the same encoder/forward pass used by the HTTP service.
`--batch-deals` bounds active state (default 64); `--threads` controls Rust worker
threads (default at most eight). Only aggregate statistics and the compact exact
deal suite grow on disk. The training/benchmark membership indices use bounded
RAM and temporary on-disk SQLite storage.

`--agent-url http://127.0.0.1:8765` can evaluate a protocol-compatible server,
with `--timeout 2`. This path is sequential HTTP for interoperability, **not**
the recommended path for millions of decisions. Use local batched inference for
large NPZ evaluations. Single-legal-card turns and forced passes bypass policy
inference; this benchmark does not test protocol compliance on those forced
turns.

There is no heuristic fallback. Illegal actions, bad protocol/checkpoint
identities, and timeouts stop the run and leave `complete: false`, failure details,
and counters. A partially completed deal is excluded from the aggregate. Do not
publish incomplete reports as model scores. An interrupted run may have an
incomplete JSON report or deal suite and must not be treated as completed.
Inference times describe batches and amortized seconds per requested decision,
not individual-request tail latency. Exact timing varies with load and hardware.

## Sampled decision analysis

Analyze a card choice from a completed recording:

```bash
python -m schafkopf_ml.evaluation.analyze \
  --input ../games/npc-run-004-00000.jsonl.gz \
  --game-index 0 --turn 12 --samples 256 --seed 42 \
  --bridge-bin ../target/release/examples/evaluation_bridge \
  --output artifacts/decision-001.json
```

Supports full/compact JSONL or gzip, completed frontend game `.json` files, and
completed plain `RoundRecord` JSON. Turn indices are chronological card plays
0..31. Records are replay-validated before extracting the seat observation. The
actual opponents' hands and eventual result are not passed to the analyzer.
Use `--action-card N` to assess another legal card instead of the recorded choice.

For frontend/backend integration without an archive, save a JSON object with
`observation` and `action`, then use `--observation request.json` instead of
`--input`. The output is the shared Rust `DecisionAssessment` schema, with player,
mode, turn, observation fingerprint, action estimates, uncertainty, and evaluator
identity. The CLI provides offline reports; no automatic GUI integration or
`/analyze` HTTP endpoint is installed by this evaluation project.

The analyzer:

1. Fixes the actor's known initial cards and all public played cards.
2. Uses dynamic-programming completion counts to sample uniformly among hidden
   card assignments satisfying hand sizes and public voids. Greedy assignment
   would bias the samples.
3. Rejects samples that cannot replay the actual public transcript, declaration
   eligibility, called-ace restrictions, or observed partnership knowledge.
4. Tries every legal action on the same accepted hidden states, then continues
   with frozen reference policies. Each reference sees only its own observation.
5. Selects an estimated best action on the first half of the samples, and measures
   its difference from the chosen action on the independent second half. This
   avoids selecting and evaluating a noisy maximum on the same samples.

Reports contain estimated win probabilities with Wilson intervals, benchmark
utilities, estimated decision loss, gain over the reference action, and conservative
Hoeffding comparison bounds. The bounds reflect Monte Carlo sampling error,
not errors in the belief distribution or reference policy. Hidden-hand sampling
conditions on eligibility/legality, not inferred bidding-strength likelihoods.
There is no claim of optimal play, exact human win probabilities, or proof that a
loss was inevitable. Uncertain differences are labeled `inconclusive`; matching
the sampled best action is `no_detected_loss`. A forced choice has zero avoidable
decision loss even if the game is lost. `above_reference` and
`estimated_avoidable_loss` require a comparison bound exceeding 0.02 benchmark
utility. The report version identifies this policy; do not treat labels as
objective ratings or feed them into training as expert targets without validation.

The reference defaults to `heuristic`; `--reference first_legal` is available as a
weaker control. Samples default to 256, split evenly into discovery/evaluation.
Increase them for tighter estimates. Sampling has a proposal cap and fails
explicitly if too few valid states can be found. Deep analysis is for selected
decisions, not every move in a million-game arena run.

## Ownership

- `schafkopf_logic::evaluation`: outcome utility, report DTOs, public transcript
  extraction, and checked hypothetical-state reconstruction.
- `examples/evaluation_bridge`: native engine bridge, generic rollouts/sampling,
  and headless matched games. No ML runtime enters the rules engine.
- This directory: model/checkpoint orchestration, vectorized inference,
  confidence aggregation, dataset exclusion, and report files.
- Frontends: display these reports with their evaluator/uncertainty metadata.
  The engine's actual result always remains authoritative.

No Elo ladder, AIVAT estimator, reinforcement-learning reward shaping, tariff
system, or guarantee against all possible opponent strategies is provided.
