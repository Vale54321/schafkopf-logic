import copy
from hashlib import sha256
import json
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest
import urllib.error
import urllib.request

import numpy as np

from schafkopf_ml.features import FEATURE_VERSION, INPUT_SIZE, card_id, encode
from schafkopf_ml.serve import AgentServer, Inference
from schafkopf_ml.train import Model


class ServeTest(unittest.TestCase):
    def test_checkpoint_protocol_and_real_http(self):
        import os
        root = Path(os.environ.get("SCHAFKOPF_LOGIC_ROOT", Path(__file__).resolve().parents[2]))
        fixture = root / "target/release/examples/agent_fixture"
        self.assertTrue(fixture.exists(), "build cargo example agent_fixture first")
        request = json.loads(subprocess.check_output([str(fixture)], text=True))
        with tempfile.TemporaryDirectory(prefix="schafkopf-agent-test-") as tmp:
            path = Path(tmp) / "model.npz"
            model = Model(16, np.random.default_rng(42))
            np.savez_compressed(path, feature_version=FEATURE_VERSION, input_size=INPUT_SIZE, **model.weights)
            inference = Inference(path)
            self.assertEqual(inference.policy["checkpoint_id"], sha256(path.read_bytes()).hexdigest())
            # Confirm the service exactly uses the training feature encoder/forward pass.
            x, mask = encode(request["observation"])
            w = model.weights
            logits = np.maximum(0, np.array(x, dtype=np.float32) @ w["w1"] + w["b1"]) @ w["w2"] + w["b2"]
            expected = int(np.where(mask, logits, -np.inf).argmax())
            response = inference.act(request)
            self.assertEqual(card_id(response["decision"]["action"]["data"]), expected)
            self.assertEqual(response["request_id"], request["request_id"])
            subprocess.run([str(fixture), "--validate"], input=json.dumps(response), text=True, check=True)
            server = AgentServer(("127.0.0.1", 0), inference, ["http://localhost:8080"])
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            url = f"http://127.0.0.1:{server.server_address[1]}"
            def post(body, endpoint="act", origin=None):
                headers = {"Content-Type": "application/json"}
                if origin:
                    headers["Origin"] = origin
                return urllib.request.urlopen(urllib.request.Request(url + "/" + endpoint, data=json.dumps(body).encode(), headers=headers), timeout=3)
            try:
                with urllib.request.urlopen(url + "/info", timeout=3) as result:
                    info = json.load(result)
                self.assertEqual(info["rules"], [request["rules"]])
                self.assertEqual(info["observation_versions"], [request["observation_version"]])
                self.assertEqual(info["capabilities"], ["act"])
                with post(request, origin="http://localhost:8080") as result:
                    self.assertEqual(result.headers["Access-Control-Allow-Origin"], "http://localhost:8080")
                    self.assertEqual(json.load(result), response)
                for modify in [lambda r: r.update(protocol_version=999),
                               lambda r: r.update(revision=-1),
                               lambda r: r["observation"].update(next_player=3),
                               lambda r: r["observation"].update(legal_actions=[])]:
                    bad = copy.deepcopy(request)
                    modify(bad)
                    with self.assertRaises(urllib.error.HTTPError) as error:
                        post(bad)
                    self.assertEqual(error.exception.code, 400)
                    error.exception.close()
                with self.assertRaises(urllib.error.HTTPError) as error:
                    post(request, "declare")
                self.assertEqual(error.exception.code, 501)
                unsupported = json.load(error.exception)
                self.assertEqual(unsupported["code"], "unsupported_capability")
                self.assertEqual(unsupported["context"]["request_id"], request["request_id"])
                error.exception.close()
                with self.assertRaises(urllib.error.HTTPError) as error:
                    post(request, origin="http://wrong-origin")
                self.assertEqual(error.exception.code, 403)
                error.exception.close()
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=3)
            np.savez_compressed(path, feature_version=999, input_size=INPUT_SIZE, **model.weights)
            with self.assertRaises(ValueError):
                Inference(path)
            # The loaded service retains its immutable original checkpoint.
            self.assertEqual(inference.act(request), response)


if __name__ == "__main__":
    unittest.main()
