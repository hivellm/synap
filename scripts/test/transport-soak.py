"""Hammer every transport at once and report the moment the server stops answering.

The observed hang took HTTP, RESP3 and the periodic snapshot task down
together, which is what a stalled tokio runtime looks like rather than one
wedged handler. So this drives all three wires concurrently — with the
malformed queue publishes that preceded the hang mixed in — and a watchdog
polls /health once a second, printing the first second where it goes silent.
"""

import socket
import sys
import threading
import time
import urllib.request

HOST = "127.0.0.1"
HTTP = f"http://{HOST}:15500"
RESP3_PORT = 6379
RPC_PORT = 15501

stop = threading.Event()
first_silence = None
lock = threading.Lock()
counters = {"resp3": 0, "http": 0, "churn": 0, "errors": 0}


def resp3_cmd(sock, *parts):
    out = ("*%d\r\n" % len(parts)).encode()
    for p in parts:
        b = p.encode() if isinstance(p, str) else p
        out += b"$%d\r\n" % len(b) + b + b"\r\n"
    sock.sendall(out)
    return sock.recv(65536)


def resp3_worker(idx):
    """Valid traffic interleaved with the malformed publishes."""
    try:
        s = socket.create_connection((HOST, RESP3_PORT), 5)
        s.settimeout(10)
        resp3_cmd(s, "QCREATE", f"hang-q-{idx}")
        resp3_cmd(s, "SCREATE", f"hang-room-{idx}")
    except Exception as exc:
        with lock:
            counters["errors"] += 1
        print(f"[resp3-{idx}] setup failed: {exc}", flush=True)
        return

    i = 0
    while not stop.is_set():
        i += 1
        try:
            resp3_cmd(s, "SET", f"hang:{idx}:{i}", "v")
            resp3_cmd(s, "GET", f"hang:{idx}:{i}")
            # A JSON object where the handler wants a byte array — the shape
            # that was in flight when the server went silent.
            resp3_cmd(s, "QPUBLISH", f"hang-q-{idx}", '{"n":%d}' % i, "0", "3")
            resp3_cmd(s, "QCONSUME", f"hang-q-{idx}", f"c{idx}")
            resp3_cmd(s, "SPUBLISH", f"hang-room-{idx}", "tick", "{}")
            resp3_cmd(s, "SREAD", f"hang-room-{idx}", f"sub{idx}", "0", "10")
            resp3_cmd(s, "SSTATS", f"hang-room-{idx}")
            resp3_cmd(s, "PUBLISH", f"hang.topic.{idx}", "{}")
            # Unknown/malformed commands: the error path the log ended on.
            resp3_cmd(s, "QPUBLISH", f"missing-queue-{idx}", "{}", "0", "3")
            resp3_cmd(s, "NOSUCHCOMMAND", "x")
            # A transaction, since one was open in the run that hung.
            cid = f"hang-tx-{idx}-{i}"
            resp3_cmd(s, "MULTI", cid)
            resp3_cmd(s, "TXQUEUE", cid, "SET", f"hang:tx:{idx}", "1")
            resp3_cmd(s, "EXEC", cid)
            with lock:
                counters["resp3"] += 1
        except Exception as exc:
            with lock:
                counters["errors"] += 1
            print(f"[resp3-{idx}] died after {i} rounds: {type(exc).__name__}", flush=True)
            return


def http_worker(idx):
    i = 0
    while not stop.is_set():
        i += 1
        try:
            body = (
                '{"command":"kv.set","request_id":"h%d-%d","payload":{"key":"hangh:%d:%d","value":"v"}}'
                % (idx, i, idx, i)
            ).encode()
            req = urllib.request.Request(
                f"{HTTP}/api/v1/command", data=body,
                headers={"Content-Type": "application/json"},
            )
            urllib.request.urlopen(req, timeout=10).read()
            with lock:
                counters["http"] += 1
        except Exception as exc:
            with lock:
                counters["errors"] += 1
            print(f"[http-{idx}] failed at round {i}: {type(exc).__name__}", flush=True)
            time.sleep(0.5)


def churn_worker(idx):
    """Connect, subscribe, then vanish without unsubscribing.

    A PHP script ending does exactly this, and it is the state the pub/sub
    router has to clean up lazily: the next publish finds a closed channel and
    takes the drop path (deliver_message -> unregister_connection), which is
    the rarest lock sequence in the router.
    """
    i = 0
    while not stop.is_set():
        i += 1
        try:
            s = socket.create_connection((HOST, RESP3_PORT), 5)
            s.settimeout(10)
            resp3_cmd(s, "SUBSCRIBE", f"hang.topic.{idx % 8}")
            resp3_cmd(s, "SUBSCRIBE", "hang.*")
            # Publish once so the router has a reason to touch this subscriber,
            # then drop the socket mid-flight.
            # Publish through the same framing helper, then drop the socket
            # mid-flight so the router is left holding a dead channel.
            resp3_cmd(s, "PUBLISH", f"hang.topic.{idx % 8}", "{}")
            s.close()
            with lock:
                counters["churn"] = counters.get("churn", 0) + 1
        except Exception:
            with lock:
                counters["errors"] += 1
            time.sleep(0.1)


def watchdog(duration):
    global first_silence
    started = time.time()
    while time.time() - started < duration:
        t = time.time() - started
        try:
            urllib.request.urlopen(f"{HTTP}/health", timeout=3).read()
        except Exception as exc:
            if first_silence is None:
                first_silence = t
                print(f"\n*** /health stopped answering at t={t:.1f}s ({type(exc).__name__}) ***\n", flush=True)
        else:
            if first_silence is not None:
                print(f"    /health recovered at t={t:.1f}s", flush=True)
                first_silence = None
        if int(t) % 10 == 0:
            with lock:
                print(f"  t={t:5.1f}s  resp3={counters['resp3']:6d}  http={counters['http']:6d}  churn={counters['churn']:5d}  errors={counters['errors']}", flush=True)
        time.sleep(1)
    stop.set()


def main():
    duration = int(sys.argv[1]) if len(sys.argv) > 1 else 120
    workers = int(sys.argv[2]) if len(sys.argv) > 2 else 8

    print(f"hammering for {duration}s with {workers} RESP3 + {workers} HTTP workers", flush=True)
    threads = []
    for i in range(workers):
        threads.append(threading.Thread(target=resp3_worker, args=(i,), daemon=True))
        threads.append(threading.Thread(target=http_worker, args=(i,), daemon=True))
        threads.append(threading.Thread(target=churn_worker, args=(i,), daemon=True))
    for t in threads:
        t.start()

    watchdog(duration)
    for t in threads:
        t.join(timeout=2)

    print("\nfinal:", counters, flush=True)
    if first_silence is not None:
        print(f"SERVER WAS SILENT AT THE END (from t={first_silence:.1f}s)")
        return 1
    print("server answered throughout")
    return 0


if __name__ == "__main__":
    sys.exit(main())
