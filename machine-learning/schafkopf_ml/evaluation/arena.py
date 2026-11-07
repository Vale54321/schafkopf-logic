"""Paired held-out arena: batched inference, engine-validated play, deal-clustered CIs."""
import argparse
import gzip
from collections import defaultdict
from contextlib import ExitStack
from hashlib import sha256
import json
import math
import os
from pathlib import Path
import sqlite3
import sys
import tempfile
import time
import urllib.error
import urllib.request

from ..serve import Inference, PROTOCOL_VERSION
from .bridge import Bridge, BridgeError
from .statistics import Aggregate


class EvaluationFailure(ValueError):
    def __init__(self, kind, message):
        self.kind = kind
        super().__init__(message)


class HoldoutIndex:
    def __init__(self, folder, dataset=None):
        self.connection = sqlite3.connect(str(Path(folder) / "holdout.sqlite"))
        try:
            self.connection.execute("CREATE TABLE training (id TEXT PRIMARY KEY)")
            self.connection.execute("CREATE TABLE benchmark (id TEXT PRIMARY KEY)")
            if dataset:
                import pyarrow.compute as pc
                import pyarrow.parquet as pq
                manifest = json.loads((dataset / "dataset.json").read_text())
                if not manifest.get("complete"):
                    raise ValueError("training dataset manifest is incomplete")
                for name in manifest["files"]:
                    for batch in pq.ParquetFile(dataset / name).iter_batches(columns=["deal_id"]):
                        ids = pc.unique(batch.column(0)).to_pylist()
                        self.connection.executemany("INSERT OR IGNORE INTO training VALUES (?)", [(i,) for i in ids])
                self.connection.commit()
        except BaseException:
            self.connection.close()
            raise

    def check(self, deal_id):
        if self.connection.execute("SELECT 1 FROM training WHERE id=?", (deal_id,)).fetchone():
            raise EvaluationFailure("protocol_errors", "benchmark deal overlaps the training/validation/test preparation dataset; change the seed range")
        if self.connection.execute("INSERT OR IGNORE INTO benchmark VALUES (?)", (deal_id,)).rowcount != 1:
            raise EvaluationFailure("protocol_errors", "benchmark repeats an equivalent deal; independent clusters required")

    def close(self):
        self.connection.close()


class Policy:
    def __init__(self, args, rules):
        self.model = Inference(args.model) if args.model else None
        self.url = args.agent_url.rstrip("/") if args.agent_url else None
        self.timeout = args.timeout
        if self.model:
            self.info = self.model.info()
        elif self.url:
            self.info = self.http("info")
        else:
            self.info = {"policy": {"name": args.candidate, "version": 1, "checkpoint_id": f"builtin:{args.candidate}:v1"}, "capabilities": ["act", "declare"]}
        if self.model or self.url:
            if self.info.get("protocol_version") != PROTOCOL_VERSION or rules not in self.info.get("rules", []) or 1 not in self.info.get("observation_versions", []) or "act" not in self.info.get("capabilities", []):
                raise ValueError("candidate does not support the engine protocol/rules/observations")
        if args.scope == "full-player" and "declare" not in self.info.get("capabilities", []):
            raise ValueError("full-player evaluation requires declaration inference; this model supports card-play only")
        self.identity = self.info["policy"]
        if not self.identity.get("name") or not self.identity.get("checkpoint_id"):
            raise ValueError("candidate lacks an immutable policy identity")

    def http(self, endpoint, body=None):
        request = urllib.request.Request(self.url + "/" + endpoint,
                    data=json.dumps(body).encode() if body is not None else None,
                    headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                data = response.read(64 * 1024 + 1)
                if len(data) > 64 * 1024:
                    raise EvaluationFailure("protocol_errors", "agent response too large")
                return json.loads(data)
        except (TimeoutError, OSError) as error:
            timed_out = isinstance(error, TimeoutError) or isinstance(getattr(error, "reason", None), TimeoutError)
            raise EvaluationFailure("timeouts" if timed_out else "transport_errors", str(error)) from error
        except (ValueError, TypeError) as error:
            raise EvaluationFailure("protocol_errors", str(error)) from error

    def decide(self, queries):
        try:
            if self.model:
                if any(q["phase"] != "act" for q in queries):
                    raise EvaluationFailure("protocol_errors", "card model cannot bid")
                responses = self.model.act_many([q["request"] for q in queries])
            else:
                responses = [self.http(q["phase"], q["request"]) for q in queries]
        except EvaluationFailure:
            raise
        except (ValueError, KeyError, TypeError) as error:
            raise EvaluationFailure("protocol_errors", str(error)) from error
        result = []
        for query, response in zip(queries, responses):
            request = query["request"]
            if not isinstance(response, dict) or "decision" not in response or response.get("protocol_version") != PROTOCOL_VERSION or response.get("policy") != self.identity or any(response.get(key) != request[key] for key in ["request_id", "game_id", "revision"]):
                raise EvaluationFailure("protocol_errors", "response context/version/checkpoint mismatch")
            decision = response.get("decision")
            if query["phase"] == "act":
                if not isinstance(decision, dict) or decision.get("action") not in request["observation"]["legal_actions"]:
                    raise EvaluationFailure("illegal_responses", "candidate returned an illegal card action")
            elif decision is not None and decision not in request["observation"]["legal_modes"]:
                raise EvaluationFailure("illegal_responses", "candidate returned an illegal declaration")
            result.append({"id": query["id"], "response": response})
        return result


def evaluate(args):
    args.output.parent.mkdir(parents=True, exist_ok=True)
    # An exclusive report file claims the run. Failure leaves complete=false.
    with args.output.open("x+", encoding="utf8") as output, Bridge(args.bridge_bin, args.threads) as bridge, tempfile.TemporaryDirectory(prefix="schafkopf-arena-") as tmp:
        config = {"scope": args.scope, "seed": args.seed, "deals": args.deals, "baseline": args.baseline,
                  "opponents": args.opponents, "batch_deals": args.batch_deals, "utility": bridge.info["utility"]}
        metadata = {"bridge_binary_sha256": sha256(args.bridge_bin.read_bytes()).hexdigest(),
                    "confidence_method": "asymptotic normal 95% ratio-estimator CI clustered by canonical deal; null below 2 clusters",
                    "strata": "mode/role refer to baseline contract; candidate contract may differ in full-player mode",
                    "opening_seats": "rotates with deal index; candidate evaluated at all four seats on every deal",
                    "candidate_declarations": "candidate" if args.scope == "full-player" else "frozen heuristic bidding",
                    "training_exclusion_dataset": str(args.training_dataset.resolve()) if args.training_dataset else "NOT CHECKED; use --training-dataset",
                    "inference": "local vectorized NPZ" if args.model else "sequential HTTP" if args.agent_url else "Rust local policy",
                    "forced_actions": "single-legal-action card turns and forced passes bypass policy inference"}
        benchmark = {"config": {key: config[key] for key in ["scope", "baseline", "opponents", "utility"]}, "engine": metadata["bridge_binary_sha256"]}
        stats = {key: 0 for key in ["decisions", "batches", "illegal_responses", "timeouts", "protocol_errors", "transport_errors", "fallbacks"]}
        stats.update(total_seconds=0.0, max_batch_seconds=0.0)
        overall, strata, opponents = Aggregate(), defaultdict(Aggregate), defaultdict(Aggregate)
        report = {"schema_version": 1, "complete": False, "rules": bridge.info["rules"], "config": config,
                  "candidate": {"name": args.candidate or "unavailable", "version": 1, "checkpoint_id": "unavailable"}, "benchmark_id": sha256(json.dumps(benchmark, sort_keys=True).encode()).hexdigest(),
                  "metadata": metadata, "overall": overall.report(), "by_mode_role": {}, "by_opponent": {},
                  "inference": stats, "failure": None}
        def save():
            report["overall"] = overall.report()
            report["by_mode_role"] = {key: value.report() for key, value in sorted(strata.items())}
            report["by_opponent"] = {key: value.report() for key, value in sorted(opponents.items())}
            output.seek(0)
            output.truncate()
            json.dump(report, output, indent=2, allow_nan=False)
            output.write("\n")
            output.flush()
        save()
        suite_path = args.output.with_suffix(".deals.jsonl.gz")
        suite = getattr(args, "suite", None)
        started = time.perf_counter()
        try:
            policy = Policy(args, bridge.info["rules"])
            report["candidate"] = policy.identity
            with ExitStack() as stack:
                suite_output = stack.enter_context(gzip.open(suite_path, "xt", encoding="utf8"))
                suite_input = stack.enter_context(gzip.open(suite, "rt", encoding="utf8")) if suite else None
                report["metadata"]["exact_deals_file"] = str(suite_path.resolve())
                suite_hash = sha256()
                index = HoldoutIndex(tmp, args.training_dataset)
                stack.callback(index.close)
                next_progress = 1000
                for offset in range(0, args.deals, args.batch_deals):
                    count = min(args.batch_deals, args.deals-offset)
                    orders = None
                    if suite_input:
                        entries = []
                        for _ in range(count):
                            line = suite_input.readline()
                            if not line:
                                raise EvaluationFailure("protocol_errors", "suite contains too few deals")
                            entry = json.loads(line)
                            if entry["index"] != offset + len(entries):
                                raise EvaluationFailure("protocol_errors", "suite indices must start at zero and be consecutive")
                            entries.append(entry)
                        orders = [entry["deal_order"] for entry in entries]
                    cluster_pairs = defaultdict(list)
                    batch_ids = set()
                    for lineup_number, lineup in enumerate(args.opponents):
                        message = bridge.call({"command": "start", "config": {"seed": args.seed + offset, "deals": count,
                            "offset": offset, "opponent": lineup, "baseline": args.baseline,
                            "candidate": "external" if policy.model or policy.url else args.candidate, "scope": args.scope, "deal_orders": orders}})
                        if lineup_number == 0:
                            for deal in message["deals"]:
                                index.check(deal["deal_id"])
                                batch_ids.add(deal["deal_id"])
                                line = json.dumps(deal, sort_keys=True, separators=(",", ":")) + "\n"
                                suite_output.write(line)
                                # Hash actual ordered cards and opener, not seed labels.
                                suite_hash.update(bytes(deal["deal_order"]))
                                suite_hash.update(bytes([deal["index"] % 4]))
                        elif {d["deal_id"] for d in message["deals"]} != batch_ids:
                            raise EvaluationFailure("protocol_errors", "opponent runs used different deals")
                        while True:
                            for pair in message["results"]:
                                if args.scope == "card-play" and pair["baseline"]["mode"] != pair["candidate"]["mode"]:
                                    raise EvaluationFailure("protocol_errors", "card-play contract changed")
                                cluster_pairs[pair["deal_id"]].append(pair)
                            if message["finished"]:
                                break
                            queries = message["requests"]
                            before = time.perf_counter()
                            stats["decisions"] += len(queries)
                            stats["batches"] += 1
                            try:
                                responses = policy.decide(queries)
                            finally:
                                elapsed = time.perf_counter() - before
                                stats["total_seconds"] += elapsed
                                stats["max_batch_seconds"] = max(stats["max_batch_seconds"], elapsed)
                            message = bridge.call({"command": "respond", "responses": responses})
                    for deal, pairs in cluster_pairs.items():
                        if len(pairs) != 4 * len(args.opponents):
                            raise EvaluationFailure("protocol_errors", "incomplete paired deal")
                        overall.add_cluster(pairs)
                        by_stratum, by_opponent = defaultdict(list), defaultdict(list)
                        for pair in pairs:
                            mode = pair["baseline"]["mode"]
                            label = mode if isinstance(mode, str) else ":".join(str(v) if v is not None else "pure" for item in mode.items() for v in item)
                            context = label + "/" + pair["baseline"]["role"]
                            by_stratum[context].append(pair)
                            by_opponent[pair["opponent"]].append(pair)
                        for key, values in by_stratum.items(): strata[key].add_cluster(values)
                        for key, values in by_opponent.items(): opponents[key].add_cluster(values)
                    index.connection.commit()
                    done = offset + count
                    if done >= next_progress or done == args.deals:
                        print(f"{done}/{args.deals} paired deals completed", file=sys.stderr)
                        next_progress = done + 1000
                report["metadata"]["exact_deals_sha256"] = suite_hash.hexdigest()
                benchmark["exact_deals_sha256"] = suite_hash.hexdigest()
                report["benchmark_id"] = sha256(json.dumps(benchmark, sort_keys=True).encode()).hexdigest()
                report["complete"] = True
                bridge.call({"command": "validate_report", "report": report})
        except (ValueError, OSError) as error:
            report["complete"] = False
            kind = error.kind if isinstance(error, EvaluationFailure) else "protocol_errors"
            stats[kind] += 1
            report["failure"] = f"{kind}: {error}"
            raise
        finally:
            elapsed = time.perf_counter()-started
            report["metadata"]["elapsed_seconds"] = str(elapsed)
            report["metadata"]["completed_games"] = str(overall.pairs * 2)
            report["metadata"]["completed_games_per_second"] = str(overall.pairs * 2 / elapsed if elapsed else 0)
            report["metadata"]["amortized_inference_seconds_per_requested_decision"] = str(stats["total_seconds"]/stats["decisions"] if stats["decisions"] else 0)
            save()
        return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    candidate = parser.add_mutually_exclusive_group(required=True)
    candidate.add_argument("--model", type=Path)
    candidate.add_argument("--agent-url")
    candidate.add_argument("--candidate", choices=["heuristic", "first_legal"])
    parser.add_argument("--bridge-bin", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--scope", choices=["card-play", "full-player"], default="card-play")
    parser.add_argument("--baseline", choices=["heuristic", "first_legal"], default="heuristic")
    parser.add_argument("--opponents", nargs="+", choices=["heuristic", "first_legal", "mixed"], default=["heuristic", "first_legal"])
    parser.add_argument("--deals", type=int, default=10_000)
    parser.add_argument("--seed", type=int, default=4_000_000_000)
    parser.add_argument("--batch-deals", type=int, default=64)
    parser.add_argument("--threads", type=int, default=min(os.cpu_count() or 1, 8))
    parser.add_argument("--timeout", type=float, default=2.0)
    parser.add_argument("--training-dataset", type=Path)
    parser.add_argument("--suite", type=Path, help="Replay an exact .deals.jsonl.gz suite (first --deals entries)")
    args = parser.parse_args()
    if args.deals < 1 or not 1 <= args.batch_deals <= 4096 or args.threads < 1 or not math.isfinite(args.timeout) or args.timeout <= 0 or args.seed < 0 or args.seed + args.deals - 1 > 2**64-1 or len(set(args.opponents)) != len(args.opponents):
        parser.error("invalid counts, seed range, timeout, or duplicate opponent lineups")
    args.bridge_bin = args.bridge_bin.resolve()
    try:
        report = evaluate(args)
    except (ValueError, OSError) as error:
        parser.exit(2, f"Evaluation failed: {error}. Check the report's complete flag.\n")
    print(json.dumps({"report": str(args.output), "overall": report["overall"], "inference": report["inference"]}, indent=2))


if __name__ == "__main__":
    main()
