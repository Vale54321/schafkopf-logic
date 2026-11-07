"""Stateless local HTTP agent for the trainer's model.npz checkpoints."""
import argparse
from hashlib import sha256
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
from pathlib import Path
import threading
import zipfile

import numpy as np

from .features import FEATURE_VERSION, INPUT_SIZE, encode

PROTOCOL_VERSION = 1
OBSERVATION_VERSION = 1
RULES = "single_declaration_v1;solo>pure_wenz_geier>suit_wenz_geier>bettel>sauspiel;equal_rank_earlier_seat;all_pass_ramsch;bettel_herz_trumps;ramsch_highest_loses_all8_durchmarsch"
MAX_BODY = 256 * 1024


class Inference:
    def __init__(self, path):
        # Load/hash the exact same bytes once. Existing training runs keep their
        # write behavior; start/restart the service after the desired save finishes.
        data = Path(path).read_bytes()
        with np.load(io.BytesIO(data), allow_pickle=False) as saved:
            if int(saved["feature_version"]) != FEATURE_VERSION or int(saved["input_size"]) != INPUT_SIZE:
                raise ValueError("incompatible checkpoint feature version or input size")
            self.weights = {key: np.array(saved[key], dtype=np.float32) for key in ("w1", "b1", "w2", "b2")}
        w = self.weights
        if w["w1"].ndim != 2 or w["w1"].shape[0] != INPUT_SIZE:
            raise ValueError("invalid input weight dimensions")
        hidden = w["w1"].shape[1]
        if not 1 <= hidden <= 8192 or w["b1"].shape != (hidden,) or w["w2"].shape != (hidden, 32) or w["b2"].shape != (32,):
            raise ValueError("invalid checkpoint dimensions")
        if not all(np.isfinite(array).all() for array in w.values()):
            raise ValueError("checkpoint contains non-finite weights")
        self.policy = {"name": "mlp_imitation", "version": 1, "checkpoint_id": sha256(data).hexdigest()}
        self.lock = threading.Lock()

    def info(self):
        return {"protocol_version": PROTOCOL_VERSION, "observation_versions": [OBSERVATION_VERSION],
                "rules": [RULES], "capabilities": ["act"], "policy": self.policy,
                "feature_version": FEATURE_VERSION}

    def _prepare(self, request):
        validate_envelope(request)
        view = request["observation"]
        if not isinstance(view, dict) or type(view.get("player")) is not int or not 0 <= view["player"] < 4:
            raise ValueError("invalid observation player")
        if view.get("next_player") != view["player"] or view.get("result") is not None:
            raise ValueError("observation must belong to the acting player in an unfinished game")
        if not 1 <= len(view.get("hand", [])) <= 8 or not 1 <= len(view.get("legal_actions", [])) <= 8:
            raise ValueError("invalid hand or legal action count")
        for name in ("points", "tricks_won", "hand_sizes"):
            if len(view.get(name, [])) != 4:
                raise ValueError(f"invalid {name}")
        if len(view.get("trick", [])) > 4 or len(view.get("history", [])) > 8:
            raise ValueError("invalid public history size")
        x, legal = encode(view)
        x = np.asarray(x, dtype=np.float32)
        if not np.isfinite(x).all():
            raise ValueError("non-finite features")
        return x, legal, view

    def act_many(self, requests):
        """Vectorized local inference for evaluation; same encoder and weights as HTTP."""
        if not requests:
            return []
        prepared = [self._prepare(request) for request in requests]
        x = np.asarray([item[0] for item in prepared], dtype=np.float32)
        legal = np.asarray([item[1] for item in prepared], dtype=bool)
        with self.lock:
            w = self.weights
            logits = np.maximum(0, x @ w["w1"] + w["b1"]) @ w["w2"] + w["b2"]
        if not np.isfinite(logits).all():
            raise ValueError("non-finite model output")
        chosen = np.where(legal, logits, -np.inf).argmax(axis=1)
        from .features import card_id
        responses = []
        for request, (_, _, view), card in zip(requests, prepared, chosen):
            action = next(action for action in view["legal_actions"] if card_id(action["data"]) == int(card))
            responses.append({"protocol_version": PROTOCOL_VERSION, **context(request), "policy": self.policy,
                "decision": {"action": action, "reason": "model_policy"}})
        return responses

    def act(self, request):
        return self.act_many([request])[0]


def context(request):
    return {key: request[key] for key in ("request_id", "game_id", "revision")}


def validate_envelope(request):
    if not isinstance(request, dict):
        raise ValueError("request must be an object")
    if type(request.get("protocol_version")) is not int or type(request.get("observation_version")) is not int or request.get("protocol_version") != PROTOCOL_VERSION or request.get("observation_version") != OBSERVATION_VERSION or request.get("rules") != RULES:
        raise ValueError("unsupported protocol, observation, or rules version")
    for name in ("request_id", "game_id"):
        if not isinstance(request.get(name), str) or not 1 <= len(request[name]) <= 256:
            raise ValueError(f"invalid {name}")
    if type(request.get("revision")) is not int or not 0 <= request["revision"] <= 2**64 - 1:
        raise ValueError("invalid revision")
    if "observation" not in request:
        raise ValueError("observation is required")


class AgentServer(ThreadingHTTPServer):
    daemon_threads = True
    def __init__(self, address, model, origins=()):
        self.model, self.origins = model, frozenset(origins)
        super().__init__(address, Handler)


class Handler(BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(5)

    def respond(self, status, payload):
        data = json.dumps(payload, allow_nan=False, separators=(",", ":")).encode()
        self.send_response(status)
        origin = self.headers.get("Origin")
        if origin in self.server.origins:
            self.send_header("Access-Control-Allow-Origin", origin)
            self.send_header("Vary", "Origin")
            self.send_header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
            self.send_header("Access-Control-Allow-Headers", "Content-Type")
        self.send_header("Content-Type", "application/json")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        try:
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            pass

    def error(self, status, code, message, request=None):
        request_context = None
        if isinstance(request, dict):
            try:
                request_context = context(request)
            except KeyError:
                pass
        self.respond(status, {"protocol_version": PROTOCOL_VERSION, "context": request_context,
                              "code": code, "message": message})

    def allowed(self):
        origin = self.headers.get("Origin")
        if origin and origin not in self.server.origins:
            self.error(403, "origin_denied", "origin is not configured")
            return False
        return True

    def do_OPTIONS(self):
        if self.allowed():
            self.respond(200, {})

    def do_GET(self):
        if not self.allowed():
            return
        if self.path == "/info":
            self.respond(200, self.server.model.info())
        else:
            self.error(404, "not_found", "unknown endpoint")

    def do_POST(self):
        self.close_connection = True
        if not self.allowed():
            return
        request = None
        try:
            size = int(self.headers.get("Content-Length", "0"))
            if not 0 < size <= MAX_BODY:
                self.error(413, "invalid_size", "request body must be 1..262144 bytes")
                return
            if self.headers.get("Content-Type", "").split(";")[0].strip() != "application/json":
                self.error(415, "content_type", "expected application/json")
                return
            data = self.rfile.read(size)
            if len(data) != size:
                raise ValueError("incomplete request body")
            def reject_constant(value):
                raise ValueError(f"invalid JSON constant: {value}")
            request = json.loads(data, parse_constant=reject_constant)
            if self.path == "/declare":
                validate_envelope(request)
                self.error(501, "unsupported_capability", "this checkpoint supports card play only; use local bidding", request)
            elif self.path == "/act":
                self.respond(200, self.server.model.act(request))
            else:
                self.error(404, "not_found", "unknown endpoint", request)
        except (ValueError, KeyError, IndexError, TypeError, StopIteration, OverflowError) as error:
            self.error(400, "invalid_request", str(error), request)
        except (TimeoutError, ConnectionError):
            self.close_connection = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--allow-origin", action="append", default=[], help="Exact browser frontend origin; repeat as needed")
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error("port must be in 1..65535")
    try:
        model = Inference(args.model)
    except (OSError, ValueError, KeyError, EOFError, zipfile.BadZipFile) as error:
        parser.error(f"cannot load checkpoint (wait until its save finishes): {error}")
    with AgentServer((args.host, args.port), model, args.allow_origin) as server:
        print(json.dumps({"listening": f"http://{args.host}:{args.port}", **model.info()}), flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == "__main__":
    main()
