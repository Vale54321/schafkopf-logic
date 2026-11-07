import json
from pathlib import Path
import subprocess


class BridgeError(ValueError):
    pass


class Bridge:
    def __init__(self, binary, threads=8):
        self.binary = Path(binary).resolve()
        self.process = subprocess.Popen([str(self.binary), "--threads", str(threads)],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        try:
            self.info = self.call({"command": "info"})
            if self.info.get("protocol_version") != 1:
                raise BridgeError("unsupported bridge protocol")
        except BaseException:
            self.close()
            raise

    def call(self, command):
        try:
            self.process.stdin.write(json.dumps(command, allow_nan=False, separators=(",", ":")) + "\n")
            self.process.stdin.flush()
            line = self.process.stdout.readline()
        except (BrokenPipeError, OSError) as error:
            raise BridgeError(f"engine bridge stopped: {error}") from error
        if not line:
            raise BridgeError("engine bridge exited without a reply")
        response = json.loads(line)
        if not response.get("ok"):
            raise BridgeError(response.get("error", "bridge failure"))
        return response["result"]

    def close(self):
        self.process.stdin.close()
        self.process.stdout.close()
        try:
            self.process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.process.terminate()
            self.process.wait(timeout=5)

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()
