"""Streaming ratio-estimator confidence intervals clustered by whole deal."""
from math import sqrt


class Aggregate:
    def __init__(self):
        self.pairs = self.clusters = self.candidate_wins = self.baseline_wins = 0
        self.improved = self.regressed = 0
        self.y = self.yy = self.yn = self.nn = 0.0

    def add_cluster(self, pairs):
        if not pairs:
            return
        n = len(pairs)
        y = sum(p["candidate"]["utility"] - p["baseline"]["utility"] for p in pairs)
        self.pairs += n
        self.clusters += 1
        self.y += y
        self.yy += y*y
        self.yn += y*n
        self.nn += n*n
        for pair in pairs:
            candidate, baseline = pair["candidate"]["won"], pair["baseline"]["won"]
            self.candidate_wins += int(candidate)
            self.baseline_wins += int(baseline)
            self.improved += int(candidate and not baseline)
            self.regressed += int(baseline and not candidate)

    def report(self):
        mean = self.y / self.pairs if self.pairs else None
        interval = None
        if self.clusters >= 2:
            residual = max(0, self.yy - 2*mean*self.yn + mean*mean*self.nn)
            se = sqrt(self.clusters / (self.clusters - 1) * residual) / self.pairs
            interval = {"low": max(-2, mean - 1.96*se), "high": min(2, mean + 1.96*se)}
        return {"pairs": self.pairs, "deal_clusters": self.clusters,
                "candidate_wins": self.candidate_wins, "baseline_wins": self.baseline_wins,
                "improved": self.improved, "regressed": self.regressed,
                "mean_utility_delta": mean, "confidence95": interval,
                "candidate_win_rate": self.candidate_wins/self.pairs if self.pairs else None,
                "baseline_win_rate": self.baseline_wins/self.pairs if self.pairs else None}
