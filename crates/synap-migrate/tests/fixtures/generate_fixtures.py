#!/usr/bin/env python3
"""Generate synap-migrate's data-directory fixtures from a real Synap server.

The fixture is what a real server leaves on disk, not something written by
the code under test. For one server binary this script:

1. writes a configuration derived from that release's own config/config.yml,
   with persistence on (WAL fsync "always", snapshots only when asked) and
   every listener but HTTP off, into a fresh temporary directory;
2. starts the server and drives every datatype through the
   /api/v1/command envelope (phase "before_snapshot");
3. forces a snapshot (POST /snapshot);
4. writes more data, including deletes, which only the WAL holds (phase
   "after_snapshot");
5. records the baseline: what the server serves for every resource right
   before it stops;
6. stops the server with Ctrl-C (SIGINT on Unix): 1.4.0 drains and fsyncs its
   WAL, 1.3.2 has no shutdown hook and exits, its WAL already fsynced;
7. copies the data directory (snapshots/ and wal/) to
   <fixtures>/<label>/data and writes <fixtures>/<label>/baseline.json.

Usage (from the repository root):

    python crates/synap-migrate/tests/fixtures/generate_fixtures.py \
        --label v1.3.2 --server <1.3.2 synap-server> --base-config <1.3.2 checkout>/config/config.yml
    python crates/synap-migrate/tests/fixtures/generate_fixtures.py \
        --label v1.4.0 --server target/debug/synap-server --base-config config/config.yml

Requires Python 3.9+ and PyYAML. Message IDs, event IDs and timestamps
differ between runs; the procedure and the dataset do not.
"""

import argparse
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

import yaml

# Long enough that the fixture's expiring keys outlive any test run.
TEN_YEARS_SECS = 10 * 365 * 24 * 3600
# Short enough that the key has expired by the time any test runs.
SHORT_TTL_SECS = 60
# Queue messages must not time out while the fixture is generated: a
# requeue after the ACK deadline would add records the dataset does not
# describe.
ACK_DEADLINE_SECS = 3600


class CommandError(RuntimeError):
    pass


class Client:
    def __init__(self, base_url):
        self.base_url = base_url
        self.counter = 0

    def request(self, method, path, body=None):
        data = None if body is None else json.dumps(body).encode()
        request = urllib.request.Request(
            self.base_url + path,
            data=data,
            method=method,
            headers={"content-type": "application/json"},
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            text = response.read().decode()
            return json.loads(text) if text else None

    def envelope(self, command, payload):
        self.counter += 1
        return self.request(
            "POST",
            "/api/v1/command",
            {"command": command, "request_id": f"fixture-{self.counter}", "payload": payload},
        )

    def cmd(self, command, **payload):
        response = self.envelope(command, payload)
        if not response.get("success"):
            raise CommandError(f"{command} {payload}: {response.get('error')}")
        return response.get("payload")

    def read(self, command, **payload):
        """A read whose refusal is part of the baseline (a missing key)."""
        response = self.envelope(command, payload)
        if response.get("success"):
            return response.get("payload")
        return {"error": response.get("error")}


def free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def write_config(base_config, workdir, port):
    with open(base_config, encoding="utf-8") as f:
        config = yaml.safe_load(f)
    data = workdir / "data"
    config["server"]["host"] = "127.0.0.1"
    config["server"]["port"] = port
    config["auth"]["enabled"] = False
    config["auth"]["require_auth"] = False
    config["logging"]["level"] = "info"
    config["logging"]["format"] = "pretty"
    for listener in ("resp3", "synap_rpc"):
        if listener in config:
            config[listener]["enabled"] = False
    for section in ("replication", "cluster", "hub"):
        if section in config:
            config[section]["enabled"] = False
    config["persistence"] = {
        "enabled": True,
        "wal": {
            "enabled": True,
            "path": str(data / "wal" / "synap.wal"),
            "buffer_size_kb": 64,
            "fsync_mode": "always",
            "fsync_interval_ms": 1000,
            "max_size_mb": 1024,
        },
        # The background task snapshots only after `interval_secs`, so the
        # only snapshot is the one this script asks for. With an operation
        # threshold of 1, POST /snapshot (which both releases route through
        # "snapshot if due") always takes it.
        "snapshot": {
            "enabled": True,
            "directory": str(data / "snapshots"),
            "interval_secs": 86400,
            "operation_threshold": 1,
            "max_snapshots": 10,
            "compression": False,
        },
    }
    path = workdir / "config.yml"
    with open(path, "w", encoding="utf-8") as f:
        yaml.safe_dump(config, f, sort_keys=False)
    return path


def start_server(server, config_path, workdir):
    log = open(workdir / "server.log", "wb")
    kwargs = {"cwd": workdir, "stdout": log, "stderr": subprocess.STDOUT}
    if os.name == "nt":
        # Children inherit "ignore Ctrl-C" (set when this interpreter was
        # started in a new process group, as many launchers do), and a process
        # that ignores it never sees the Ctrl-C stop_server sends. Clear it.
        import ctypes

        ctypes.WinDLL("kernel32").SetConsoleCtrlHandler(None, False)
        # A console of its own, so Ctrl-C can be sent to the server alone.
        startupinfo = subprocess.STARTUPINFO()
        startupinfo.dwFlags |= subprocess.STARTF_USESHOWWINDOW
        startupinfo.wShowWindow = 0  # SW_HIDE
        kwargs["creationflags"] = subprocess.CREATE_NEW_CONSOLE
        kwargs["startupinfo"] = startupinfo
    else:
        kwargs["start_new_session"] = True
    process = subprocess.Popen([str(server), "--config", str(config_path)], **kwargs)
    return process, log


# Run in a separate interpreter: attaching to the server's console means
# leaving our own.
WINDOWS_CTRL_C = r"""
import ctypes, sys
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
kernel32.FreeConsole()
if not kernel32.AttachConsole(int(sys.argv[1])):
    sys.exit(f"AttachConsole failed: {ctypes.get_last_error()}")
kernel32.SetConsoleCtrlHandler(None, True)
if not kernel32.GenerateConsoleCtrlEvent(0, 0):
    sys.exit(f"GenerateConsoleCtrlEvent failed: {ctypes.get_last_error()}")
"""


def stop_server(process):
    if os.name == "nt":
        subprocess.run([sys.executable, "-I", "-c", WINDOWS_CTRL_C, str(process.pid)], check=True)
    else:
        process.send_signal(signal.SIGINT)
    try:
        return process.wait(timeout=60)
    except subprocess.TimeoutExpired:
        process.kill()
        raise RuntimeError("the server did not stop within 60 s of Ctrl-C")


def wait_healthy(client, process, workdir):
    deadline = time.time() + 60
    while time.time() < deadline:
        if process.poll() is not None:
            log = (workdir / "server.log").read_text(errors="replace")
            raise RuntimeError(f"the server exited with {process.returncode}:\n{log}")
        try:
            return client.request("GET", "/health")
        except (urllib.error.URLError, ConnectionError):
            time.sleep(0.2)
    raise RuntimeError("the server did not become healthy within 60 s")


def snapshot_files(workdir):
    directory = workdir / "data" / "snapshots"
    if not directory.exists():
        return []
    return sorted(p.name for p in directory.iterdir() if p.name.endswith(".bin"))


def payload(text):
    return list(text.encode())


def drive_before_snapshot(c, journal):
    """Every datatype, plus deletes, before the snapshot."""
    c.cmd("kv.set", key="plain", value="value-a")
    c.cmd("kv.set", key="ttl-long", value="lives", ttl=TEN_YEARS_SECS)
    c.cmd("kv.set", key="deleted", value="soon gone")
    c.cmd("kv.del", key="deleted")

    c.cmd("hash.set", key="user:1", field="name", value="Ada")
    c.cmd("hash.set", key="user:1", field="email", value="ada@example.com")
    c.cmd("hash.set", key="user:1", field="scratch", value="x")
    c.cmd("hash.del", key="user:1", fields=["scratch"])

    c.cmd("list.rpush", key="events", values=["a", "b", "c"])
    c.cmd("list.lpop", key="events", count=1)

    c.cmd("set.add", key="tags", members=["x", "y", "z"])
    c.cmd("set.rem", key="tags", members=["y"])

    c.cmd("sortedset.zadd", key="scores", member="alice", score=10)
    c.cmd("sortedset.zadd", key="scores", member="bob", score=20.5)
    c.cmd("sortedset.zadd", key="scores", member="carol", score=5)

    c.cmd(
        "geospatial.geoadd",
        key="places",
        locations=[
            {"lat": -23.5505, "lon": -46.6333, "member": "sao-paulo"},
            {"lat": 48.8566, "lon": 2.3522, "member": "paris"},
        ],
    )

    c.cmd("hyperloglog.pfadd", key="visitors", elements=["u1", "u2", "u3", "u4", "u5"])
    for offset in (1, 7, 100):
        c.cmd("bitmap.setbit", key="flags", offset=offset, value=1)

    queue_config = {"ack_deadline_secs": ACK_DEADLINE_SECS, "default_max_retries": 3}
    c.cmd("queue.create", name="jobs", config=queue_config)
    published = {}
    for body, priority in (("job-1", 5), ("job-2", 9), ("job-3", 5), ("job-4", 1)):
        published[body] = c.cmd("queue.publish", queue="jobs", payload=payload(body), priority=priority)["message_id"]
    # Deliveries go by priority: job-2, then job-1 and job-3.
    delivered = [c.cmd("queue.consume", queue="jobs", consumer_id="worker")["message"]["id"] for _ in range(3)]
    c.cmd("queue.ack", queue="jobs", message_id=delivered[0])
    # A NACK with requeue: the message waits again, with retry_count 1.
    c.cmd("queue.nack", queue="jobs", message_id=delivered[2], requeue=True)

    # A message with no retries left is dead-lettered by its first NACK.
    c.cmd("queue.create", name="dead", config=queue_config)
    dead_id = c.cmd("queue.publish", queue="dead", payload=payload("poison"), priority=5, max_retries=0)["message_id"]
    c.cmd("queue.consume", queue="dead", consumer_id="worker")
    c.cmd("queue.nack", queue="dead", message_id=dead_id, requeue=True)

    c.cmd("stream.create", room="feed")
    for n in range(3):
        c.cmd("stream.publish", room="feed", event="created", data={"n": n})

    journal["queue_jobs"] = {
        "published": published,
        "acked": [delivered[0]],
        "requeued_by_nack": [delivered[2]],
        # Delivered and not acknowledged when the snapshot is taken.
        "in_flight_at_snapshot": [delivered[1]],
    }
    journal["queue_dead"] = {"dead_lettered": [dead_id]}


def drive_after_snapshot(c, journal):
    """Writes only the WAL holds: the tail replayed after the snapshot."""
    c.cmd("kv.set", key="plain", value="value-b")
    c.cmd("kv.set", key="tail-plain", value="after")
    c.cmd("kv.set", key="ttl-tail", value="tail lives", ttl=TEN_YEARS_SECS)
    c.cmd("kv.set", key="ttl-short", value="expires", ttl=SHORT_TTL_SECS)
    c.cmd("kv.set", key="tail-deleted", value="x")
    c.cmd("kv.del", key="tail-deleted")

    c.cmd("hash.set", key="user:1", field="city", value="London")
    c.cmd("list.rpush", key="events", values=["d"])
    c.cmd("set.add", key="tags", members=["w"])
    c.cmd("sortedset.zadd", key="scores", member="dave", score=1)
    c.cmd("hyperloglog.pfadd", key="visitors", elements=["u6"])
    c.cmd("bitmap.setbit", key="flags", offset=200, value=1)

    jobs = journal["queue_jobs"]
    # Settle one message delivered before the snapshot: an ACK by ID.
    c.cmd("queue.ack", queue="jobs", message_id=jobs["in_flight_at_snapshot"][0])
    jobs["acked"].append(jobs["in_flight_at_snapshot"][0])
    for body, priority in (("job-5", 5), ("job-6", 7)):
        jobs["published"][body] = c.cmd("queue.publish", queue="jobs", payload=payload(body), priority=priority)["message_id"]
    tail_delivery = c.cmd("queue.consume", queue="jobs", consumer_id="worker")["message"]
    jobs["in_flight_at_stop"] = [tail_delivery["id"]]

    # A queue created and deleted, and one created with its own
    # configuration, after the snapshot.
    c.cmd("queue.create", name="temp", config={"ack_deadline_secs": ACK_DEADLINE_SECS})
    c.cmd("queue.delete", queue="temp")
    c.cmd("queue.create", name="late", config={"ack_deadline_secs": ACK_DEADLINE_SECS, "max_depth": 500})
    late_id = c.cmd("queue.publish", queue="late", payload=payload("late-1"), priority=3)["message_id"]
    journal["queue_late"] = {"published": {"late-1": late_id}}

    # Streams are not in the WAL: this event lives only in memory.
    c.cmd("stream.publish", room="feed", event="after-snapshot", data={"n": 3})


def capture_baseline(c, journal, captured_at_ms):
    """What the server serves for every resource of the dataset."""
    kv = {}
    for key in ("plain", "ttl-long", "deleted", "tail-plain", "ttl-tail", "ttl-short", "tail-deleted"):
        value = c.read("kv.get", key=key)
        ttl = c.read("kv.ttl", key=key)
        # The TTL is whole seconds remaining when it was read, so the key's
        # deadline is before ttl_read_at_ms + (ttl + 1) * 1000.
        kv[key] = {"get": value, "ttl": ttl, "ttl_read_at_ms": int(time.time() * 1000)}
    baseline = {
        "captured_at_ms": captured_at_ms,
        "kv": kv,
        "hashes": {"user:1": c.cmd("hash.getall", key="user:1")},
        "lists": {"events": c.cmd("list.lrange", key="events", start=0, stop=-1)},
        "sets": {"tags": c.cmd("set.members", key="tags")},
        "sorted_sets": {
            "scores": c.cmd("sortedset.zrange", key="scores", start=0, stop=-1, withscores=True),
            "places": c.cmd("sortedset.zrange", key="places", start=0, stop=-1, withscores=True),
        },
        "geo": {"places": c.cmd("geospatial.geopos", key="places", members=["sao-paulo", "paris"])},
        "hyperloglogs": {"visitors": c.cmd("hyperloglog.pfcount", key="visitors")},
        "bitmaps": {
            "flags": {
                "bitcount": c.cmd("bitmap.bitcount", key="flags"),
                "bits": {str(o): c.cmd("bitmap.getbit", key="flags", offset=o) for o in (0, 1, 7, 100, 200)},
            }
        },
        "queues": {
            "jobs": {"stats": c.cmd("queue.stats", queue="jobs"), **journal["queue_jobs"]},
            "dead": {"stats": c.cmd("queue.stats", queue="dead"), **journal["queue_dead"]},
            "late": {"stats": c.cmd("queue.stats", queue="late"), **journal["queue_late"]},
        },
        "queue_list": c.cmd("queue.list"),
        "streams": {
            "feed": c.cmd("stream.consume", room="feed", subscriber_id="baseline", from_offset=0, limit=100)
        },
    }
    return baseline


def generate(args):
    fixtures = Path(args.out).resolve()
    workdir = Path(tempfile.mkdtemp(prefix=f"synap-fixture-{args.label}-"))
    client = Client(f"http://127.0.0.1:{free_port()}")
    port = int(client.base_url.rsplit(":", 1)[1])
    config_path = write_config(Path(args.base_config), workdir, port)
    process, log = start_server(Path(args.server).resolve(), config_path, workdir)
    try:
        health = wait_healthy(client, process, workdir)
        journal = {}
        drive_before_snapshot(client, journal)
        client.request("POST", "/snapshot")
        taken = snapshot_files(workdir)
        if len(taken) != 1:
            raise RuntimeError(f"expected one snapshot after POST /snapshot, found {taken}")
        drive_after_snapshot(client, journal)
        baseline = capture_baseline(client, journal, int(time.time() * 1000))
    except BaseException:
        process.kill()
        process.wait()
        log.close()
        print((workdir / "server.log").read_text(errors="replace"), file=sys.stderr)
        raise
    exit_code = stop_server(process)
    log.close()
    after_stop = snapshot_files(workdir)
    if after_stop != taken:
        raise RuntimeError(f"the snapshot set changed after the forced snapshot: {taken} -> {after_stop}")

    target = fixtures / args.label
    if target.exists():
        shutil.rmtree(target)
    shutil.copytree(workdir / "data", target / "data")
    baseline["source"] = {
        "label": args.label,
        "server_binary": Path(args.server).name,
        "health": health,
        "exit_code_after_ctrl_c": exit_code,
        "snapshot": taken[0],
        "dataset": "see generate_fixtures.py: drive_before_snapshot, POST /snapshot, drive_after_snapshot",
    }
    with open(target / "baseline.json", "w", encoding="utf-8", newline="\n") as f:
        json.dump(baseline, f, indent=2, sort_keys=True)
        f.write("\n")
    shutil.rmtree(workdir, ignore_errors=True)
    print(f"{args.label}: wrote {target} (snapshot {taken[0]}, server exit code {exit_code})")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--label", required=True, help="fixture name, e.g. v1.3.2")
    parser.add_argument("--server", required=True, help="path to the synap-server binary")
    parser.add_argument("--base-config", required=True, help="that release's config/config.yml")
    parser.add_argument(
        "--out",
        default=str(Path(__file__).resolve().parent),
        help="fixtures directory (default: next to this script)",
    )
    generate(parser.parse_args())


if __name__ == "__main__":
    main()
