from argparse import Namespace
import copy
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import numpy as np
import pyarrow as pa
import pyarrow.parquet as pq

from schafkopf_ml.evaluation.arena import evaluate, Policy, EvaluationFailure
from schafkopf_ml.evaluation.bridge import Bridge
from schafkopf_ml.evaluation.statistics import Aggregate
from schafkopf_ml.features import FEATURE_VERSION, INPUT_SIZE
from schafkopf_ml.serve import Inference
from schafkopf_ml.train import Model


class EvaluationTests(unittest.TestCase):
    def setUp(self):
        root = Path(os.environ.get("SCHAFKOPF_LOGIC_ROOT", Path(__file__).resolve().parents[2]))
        self.binary = root / "target/release/examples/evaluation_bridge"
        self.assertTrue(self.binary.exists(), "build the evaluation_bridge example")

    def args(self, output, **changes):
        values = dict(output=output, bridge_bin=self.binary, threads=2, scope="card-play", seed=4_000_000_000,
                      deals=8, baseline="heuristic", opponents=["heuristic", "first_legal", "mixed"], batch_deals=3,
                      candidate="heuristic", model=None, agent_url=None, timeout=1.0, training_dataset=None)
        values.update(changes)
        return Namespace(**values)

    def test_cluster_uncertainty_does_not_count_seats_as_independent_deals(self):
        stats = Aggregate()
        for sign in [-1, 1]:
            pair = {"candidate": {"utility": sign/2, "won": sign > 0},
                    "baseline": {"utility": -sign/2, "won": sign < 0}}
            stats.add_cluster([pair] * 100)
        report = stats.report()
        self.assertEqual(report["pairs"], 200)
        self.assertEqual(report["deal_clusters"], 2)
        self.assertEqual(report["mean_utility_delta"], 0)
        self.assertGreater(report["confidence95"]["high"], 1)

    def test_self_matches_are_exactly_zero_across_scopes_and_batch_sizes(self):
        with tempfile.TemporaryDirectory() as tmp:
            for scope in ["card-play", "full-player"]:
                reports = []
                for batch in [1, 5]:
                    args = self.args(Path(tmp)/f"{scope}-{batch}.json", scope=scope, batch_deals=batch)
                    reports.append(evaluate(args))
                for report in reports:
                    self.assertTrue(report["complete"])
                    self.assertEqual(report["overall"]["pairs"], 8*4*3)
                    self.assertEqual(report["overall"]["deal_clusters"], 8)
                    self.assertEqual(report["overall"]["mean_utility_delta"], 0)
                    self.assertEqual(report["overall"]["confidence95"], {"low": 0, "high": 0})
                self.assertEqual(reports[0]["overall"], reports[1]["overall"])
                self.assertEqual(reports[0]["by_mode_role"], reports[1]["by_mode_role"])
                suite = Path(reports[0]["metadata"]["exact_deals_file"])
                replayed = evaluate(self.args(Path(tmp)/f"{scope}-suite.json", scope=scope, batch_deals=4, suite=suite, seed=999))
                self.assertEqual(replayed["overall"], reports[0]["overall"])
                self.assertEqual(replayed["metadata"]["exact_deals_sha256"], reports[0]["metadata"]["exact_deals_sha256"])
                self.assertEqual(replayed["benchmark_id"], reports[0]["benchmark_id"])


    def test_batched_checkpoint_inference_and_strict_failure_reporting(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            model = folder / "model.npz"
            weights = Model(8, np.random.default_rng(4)).weights
            np.savez_compressed(model, feature_version=FEATURE_VERSION, input_size=INPUT_SIZE, **weights)
            report = evaluate(self.args(folder/"model.json", model=model, candidate=None, deals=4))
            self.assertTrue(report["complete"])
            self.assertGreater(report["inference"]["decisions"], 0)
            self.assertGreater(report["inference"]["batches"], 0)
            self.assertEqual(report["inference"]["fallbacks"], 0)
            self.assertEqual(report["candidate"], Inference(model).policy)
            args = self.args(folder/"invalid.json", model=model, candidate=None)
            with patch.object(Policy, "decide", side_effect=EvaluationFailure("illegal_responses", "illegal action")):
                with self.assertRaises(EvaluationFailure):
                    evaluate(args)
            failed = json.loads(args.output.read_text())
            self.assertFalse(failed["complete"])
            self.assertEqual(failed["inference"]["illegal_responses"], 1)
            self.assertEqual(failed["inference"]["fallbacks"], 0)
            with self.assertRaises(ValueError):
                evaluate(self.args(folder/"bidding.json", model=model, candidate=None, scope="full-player"))
            self.assertFalse(json.loads((folder/"bidding.json").read_text())["complete"])

    def test_training_deal_overlap_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp)
            with Bridge(self.binary, 1) as bridge:
                message = bridge.call({"command": "start", "config": {"seed": 42, "deals": 1, "offset": 0,
                    "opponent": "heuristic", "candidate": "heuristic", "baseline": "heuristic", "scope": "card-play"}})
            dataset = folder/"training"
            dataset.mkdir()
            pq.write_table(pa.table({"deal_id": [message["deals"][0]["deal_id"]]}), dataset/"train.parquet")
            (dataset/"dataset.json").write_text(json.dumps({"complete": True, "files": ["train.parquet"]}))
            args = self.args(folder/"overlap.json", seed=42, deals=1, training_dataset=dataset)
            with self.assertRaises(EvaluationFailure):
                evaluate(args)
            self.assertFalse(json.loads(args.output.read_text())["complete"])

    def test_full_player_scope_really_allows_candidate_bidding(self):
        with Bridge(self.binary, 2) as bridge:
            message = bridge.call({"command": "start", "config": {"seed": 0, "deals": 8, "offset": 0,
                "opponent": "heuristic", "candidate": "external", "baseline": "heuristic", "scope": "full-player"}})
            results = []
            bids = 0
            while True:
                results += message["results"]
                if message["finished"]:
                    break
                responses = []
                for query in message["requests"]:
                    request = query["request"]
                    # Protocol headers must not expose the shuffle seed or a
                    # sequential deal index that would reconstruct hidden hands.
                    self.assertEqual(len(request["game_id"]), len("arena:") + 32)
                    self.assertTrue(request["request_id"].startswith(request["game_id"] + ":"))
                    self.assertNotIn("deal_order", request)

                    if query["phase"] == "declare":
                        decision = None
                        bids += 1
                    else:
                        decision = {"action": request["observation"]["legal_actions"][0], "reason": "model_policy"}
                    responses.append({"id": query["id"], "response": {"protocol_version": 1,
                        **{key: request[key] for key in ["request_id", "game_id", "revision"]},
                        "policy": {"name": "test", "version": 1, "checkpoint_id": "fixed"}, "decision": decision}})
                message = bridge.call({"command": "respond", "responses": responses})
            self.assertGreater(bids, 0)
            self.assertEqual(len(results), 32)
            self.assertTrue(any(p["baseline"]["mode"] != p["candidate"]["mode"] for p in results))

    def test_analysis_uses_only_observation_and_rejects_illegal_choices(self):
        import subprocess
        fixture = self.binary.with_name("agent_fixture")
        request = json.loads(subprocess.check_output([str(fixture)], text=True))
        view = request["observation"]
        with Bridge(self.binary, 1) as bridge:
            input = {"observation": view, "action": view["legal_actions"][0], "samples": 16, "seed": 7, "reference": "heuristic"}
            report = bridge.call({"command": "analyze", "input": input})
            self.assertEqual(report["knowledge"], "player_information")
            self.assertEqual(report["samples"], 16)
            self.assertTrue(report["limitations"])
            self.assertTrue(all(a["samples"] == 8 for a in report["actions"]))
            # Hidden/full-game annotations cannot change the result; they are not
            # part of the deserialized player observation used by the analyzer.
            changed = copy.deepcopy(input)
            changed["observation"]["initial_hands"] = [["secret"]]
            changed["actual_outcome"] = "win"
            self.assertEqual(bridge.call({"command": "analyze", "input": changed}), report)
            bad = copy.deepcopy(input)
            bad["action"] = {"type": "play_card", "data": {"suit": "invalid", "rank": "ass"}}
            with self.assertRaises(ValueError):
                bridge.call({"command": "analyze", "input": bad})


if __name__ == "__main__":
    unittest.main()
