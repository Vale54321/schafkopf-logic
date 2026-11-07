import copy
import gzip
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import numpy as np
import pyarrow.parquet as pq

from schafkopf_ml.dataset import prepare, verify_manifest
from schafkopf_ml.features import INPUT_SIZE, encode, split_for
from schafkopf_ml.train import Model, train


class PipelineTest(unittest.TestCase):
    def test_masked_model_learns_and_never_selects_illegal_actions(self):
        rng = np.random.default_rng(4)
        model = Model(16, rng)
        x = np.zeros((8, INPUT_SIZE), dtype=np.float32)
        x[:, 0] = 1
        legal = np.zeros((8, 32), dtype=bool)
        legal[:, [2, 7]] = True
        target = np.full(8, 7)
        before = model.step(x, legal, target)["loss_sum"]
        for _ in range(40):
            model.step(x, legal, target, 0.1)
        after = model.step(x, legal, target)
        self.assertLess(after["loss_sum"], before * 0.3)
        self.assertEqual(after["correct"], 8)
        with self.assertRaises(ValueError):
            model.step(x, legal, np.zeros(8, dtype=int))

    def test_collection_replay_splits_conversion_and_training(self):
        root = Path(os.environ.get("SCHAFKOPF_LOGIC_ROOT", Path(__file__).resolve().parents[2]))
        runner = root / "target/release/examples/self_play"
        replay = root / "target/release/examples/replay_dataset"
        self.assertTrue(runner.exists(), "build release examples first")
        with tempfile.TemporaryDirectory(prefix="schafkopf-ml-test-") as tmp:
            folder = Path(tmp)
            def collect(path, *extra):
                subprocess.run([str(runner), "--games", "120", "--seed", "42", "--threads", "4",
                                "--output", str(path), *extra], check=True, capture_output=True)
            compact = folder / "compact.jsonl.gz"
            full = folder / "full.jsonl.gz"
            collect(compact, "--shard-games", "41")
            collect(full, "--recording", "full")
            manifest = json.loads(Path(str(compact) + ".manifest.json").read_text())
            self.assertTrue(manifest["complete"])
            self.assertEqual(len(manifest["shards"]), 3)
            paths = [Path(shard["path"]) for shard in manifest["shards"]]
            for shard in manifest["shards"]:
                self.assertEqual(hashlib.sha256(Path(shard["path"]).read_bytes()).hexdigest(), shard["sha256"])
            self.assertLess(sum(p.stat().st_size for p in paths), full.stat().st_size / 3)
            manifest_path = Path(str(compact) + ".manifest.json")
            verify_manifest(manifest_path, paths)
            incomplete = folder / "incomplete.manifest.json"
            incomplete.write_text(json.dumps({**manifest, "complete": False}))
            with self.assertRaises(ValueError):
                verify_manifest(incomplete, paths)

            # Repeating a run must fail without changing any shard.
            with self.assertRaises(subprocess.CalledProcessError):
                collect(compact, "--shard-games", "41")
            replayed = subprocess.run([str(replay), str(paths[0])], check=True, capture_output=True, text=True)
            game = json.loads(replayed.stdout.splitlines()[0])
            observation = game["decisions"][0]["observation"]
            x, mask = encode(observation)
            self.assertEqual(len(x), INPUT_SIZE)
            self.assertTrue(any(mask))
            altered = copy.deepcopy(observation)
            altered["result"] = {"future_secret": 123}
            altered["initial_hands"] = {"hidden_secret": 456}
            self.assertEqual(encode(altered), (x, mask))
            # Card-play snapshots include all four public declarations.
            self.assertEqual(len(observation["declarations"]), 4)
            # Overlapping full/compact inputs deduplicate by episode content.
            output = folder / "dataset"
            report = prepare(paths + [full], replay, output, shard_rows=1000)
            self.assertEqual(report["games_read"], 240)
            self.assertEqual(report["unique_games"], 120)
            self.assertEqual(report["duplicate_games"], 120)
            self.assertEqual(sum(report["split_decisions"].values()), 120 * 32)
            deal_splits = {}
            for name in report["files"]:
                split = name.split("-")[0]
                table = pq.read_table(output / name)
                self.assertLessEqual(table.num_rows, 1000)
                for deal in table["deal_id"].to_pylist():
                    self.assertEqual(split_for(deal), split)
                    self.assertEqual(deal_splits.setdefault(deal, split), split)
            trained = train(output, folder / "model", epochs=2, hidden=16, batch_size=64)
            self.assertTrue(np.isfinite(trained["test"]["loss"]))
            with np.load(folder / "model/model.npz", allow_pickle=False) as saved:
                self.assertEqual(saved["w1"].shape, (INPUT_SIZE, 16))
            # Legacy full-v1 format remains convertible.
            with gzip.open(full, "rt") as f:
                old = json.loads(next(f))
            old["round"]["schema_version"] = 1
            old["round"].pop("declaration_decisions")
            for decision in old["decisions"]:
                decision["observation"].pop("declarations")
            legacy = folder / "legacy.jsonl"
            legacy.write_text(json.dumps(old) + "\n")
            self.assertEqual(prepare([legacy], replay)["unique_games"], 1)
            compacted = folder / "converted.jsonl"
            compacted.write_text(subprocess.run([str(replay), "--compact", str(legacy)],
                                                check=True, capture_output=True, text=True).stdout)
            self.assertEqual(prepare([compacted], replay)["unique_games"], 1)

            # Corrupt actions must never produce a completed dataset manifest.
            broken = json.loads(gzip.decompress(paths[0].read_bytes()).splitlines()[0])
            broken["compact"]["plays"][0][1] = 255
            bad = folder / "bad.jsonl"
            bad.write_text(json.dumps(broken) + "\n")
            with self.assertRaises(ValueError):
                prepare([bad], replay, folder / "bad-dataset")
            self.assertFalse((folder / "bad-dataset/dataset.json").exists())


if __name__ == "__main__":
    unittest.main()
