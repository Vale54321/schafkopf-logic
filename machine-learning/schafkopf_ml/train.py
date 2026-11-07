"""Small CPU imitation baseline. No GPU framework or engine dependency."""
import argparse
import json
from pathlib import Path

import numpy as np
import pyarrow.parquet as pq

from .features import FEATURE_VERSION, INPUT_SIZE


def batches(paths, batch_size, rng=None):
    paths = list(paths)
    if rng is not None:
        rng.shuffle(paths)
    for path in paths:
        parquet = pq.ParquetFile(path)
        metadata = parquet.schema_arrow.metadata or {}
        if metadata.get(b"feature_version") != str(FEATURE_VERSION).encode():
            raise ValueError("incompatible feature schema")
        groups = list(range(parquet.num_row_groups))
        if rng is not None:
            rng.shuffle(groups)
        for group in groups:
            # Converter row groups are bounded (4096 rows by default).
            table = parquet.read_row_group(group, columns=["features", "legal", "action"])
            x = np.asarray(table["features"].to_pylist(), dtype=np.float32)
            mask = np.asarray(table["legal"].to_pylist(), dtype=bool)
            y = np.asarray(table["action"].to_pylist(), dtype=np.int64)
            indices = np.arange(len(y))
            if rng is not None:
                rng.shuffle(indices)
            for start in range(0, len(y), batch_size):
                index = indices[start:start + batch_size]
                yield x[index], mask[index], y[index]


class Model:
    def __init__(self, hidden, rng):
        self.weights = {
            "w1": (rng.standard_normal((INPUT_SIZE, hidden)) * np.sqrt(2 / INPUT_SIZE)).astype(np.float32),
            "b1": np.zeros(hidden, dtype=np.float32),
            "w2": (rng.standard_normal((hidden, 32)) * np.sqrt(2 / hidden)).astype(np.float32),
            "b2": np.zeros(32, dtype=np.float32),
        }

    def step(self, x, mask, y, learning_rate=0):
        if x.shape[1] != INPUT_SIZE or not np.all(mask[np.arange(len(y)), y]):
            raise ValueError("invalid features or illegal targets")
        w = self.weights
        hidden = np.maximum(0, x @ w["w1"] + w["b1"])
        logits = hidden @ w["w2"] + w["b2"]
        logits = np.where(mask, logits, -np.inf)
        shifted = logits - logits.max(axis=1, keepdims=True)
        exp = np.exp(shifted)
        probabilities = exp / exp.sum(axis=1, keepdims=True)
        chosen = np.arange(len(y)), y
        loss = np.mean(-shifted[chosen] + np.log(exp.sum(axis=1)))
        prediction = probabilities.argmax(axis=1)
        correct = prediction == y
        nonforced = mask.sum(axis=1) > 1
        stats = {"loss_sum": float(loss) * len(y), "count": len(y), "correct": int(correct.sum()),
                 "nonforced": int(nonforced.sum()), "nonforced_correct": int(correct[nonforced].sum()),
                 "lowest_legal_id_correct": int((mask.argmax(axis=1) == y).sum())}
        if learning_rate:
            gradient = probabilities.copy()
            gradient[chosen] -= 1
            gradient /= len(y)
            hidden_gradient = (gradient @ w["w2"].T) * (hidden > 0)
            # Compute gradients before updating any parameter.
            gradients = {"w2": hidden.T @ gradient, "b2": gradient.sum(axis=0),
                         "w1": x.T @ hidden_gradient, "b1": hidden_gradient.sum(axis=0)}
            for name in w:
                w[name] -= learning_rate * gradients[name]
        return stats


def epoch(model, paths, batch_size, rng=None, learning_rate=0):
    totals = {key: 0 for key in ["loss_sum", "count", "correct", "nonforced", "nonforced_correct", "lowest_legal_id_correct"]}
    for x, mask, y in batches(paths, batch_size, rng):
        for key, value in model.step(x, mask, y, learning_rate).items():
            totals[key] += value
    if not totals["count"]:
        raise ValueError("empty split; prepare more distinct deals")
    return {"rows": totals["count"], "loss": totals["loss_sum"] / totals["count"],
            "accuracy": totals["correct"] / totals["count"],
            "nonforced_rows": totals["nonforced"],
            "nonforced_accuracy": totals["nonforced_correct"] / totals["nonforced"] if totals["nonforced"] else None,
            "lowest_legal_id_accuracy": totals["lowest_legal_id_correct"] / totals["count"]}


def train(dataset, output, epochs=5, hidden=128, batch_size=256, learning_rate=0.05, seed=42):
    manifest = json.loads((dataset / "dataset.json").read_text())
    if not manifest.get("complete") or manifest["feature_version"] != FEATURE_VERSION or manifest["input_size"] != INPUT_SIZE:
        raise ValueError("incomplete or incompatible dataset")
    files = {split: [dataset / name for name in manifest["files"] if name.startswith(split + "-")]
             for split in ["train", "validation", "test"]}
    if not all(files.values()):
        raise ValueError("train, validation, and test must all contain games")
    output.mkdir(parents=True, exist_ok=False)
    rng = np.random.default_rng(seed)
    model = Model(hidden, rng)
    best_loss = float("inf")
    history = []
    for number in range(epochs):
        training = epoch(model, files["train"], batch_size, rng, learning_rate)
        validation = epoch(model, files["validation"], batch_size)
        history.append({"epoch": number + 1, "train": training, "validation": validation})
        print(json.dumps(history[-1]), flush=True)
        if validation["loss"] < best_loss:
            best_loss = validation["loss"]
            np.savez_compressed(output / "model.npz", feature_version=FEATURE_VERSION,
                                input_size=INPUT_SIZE, **model.weights)
    with np.load(output / "model.npz", allow_pickle=False) as saved:
        model.weights = {key: saved[key].copy() for key in model.weights}
    test = epoch(model, files["test"], batch_size)
    report = {"model_schema": 1, "architecture": "relu_mlp", "hidden": hidden,
              "feature_version": FEATURE_VERSION, "input_size": INPUT_SIZE,
              "seed": seed, "learning_rate": learning_rate, "batch_size": batch_size,
              "dataset": manifest, "history": history, "test": test,
              "numpy_version": np.__version__}
    (output / "training.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dataset", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--epochs", type=int, default=5)
    parser.add_argument("--hidden", type=int, default=128)
    parser.add_argument("--batch-size", type=int, default=256)
    parser.add_argument("--learning-rate", type=float, default=0.05)
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()
    if min(args.epochs, args.hidden, args.batch_size) < 1 or not np.isfinite(args.learning_rate) or args.learning_rate <= 0:
        parser.error("counts and learning rate must be positive and finite")
    report = train(args.dataset, args.output, args.epochs, args.hidden, args.batch_size, args.learning_rate, args.seed)
    print(json.dumps({"test": report["test"], "checkpoint": str(args.output / "model.npz")}, indent=2))


if __name__ == "__main__":
    main()
