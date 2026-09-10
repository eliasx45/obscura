#!/usr/bin/env python3
"""Deterministic local benchmark for Obscura browser-context scaling.

The benchmark keeps one Obscura serve process, one local fixture, and one
viewport while varying only the number of browser contexts. It measures
context creation, navigation, a state/worker/header check, teardown, and RSS.
It does not contact live sites or create account activity.

Usage:
  OBSCURA_BIN=./target/release/obscura python3 benchmarks/context-isolation.py
"""

import argparse
import http.client
import json
import os
import statistics
import subprocess
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

import psutil
from playwright.sync_api import sync_playwright


LEVELS = (1, 5, 10, 20)
FIXTURE_HTML = """<!doctype html>
<meta charset="utf-8">
<title>Obscura context benchmark</title>
<body>context benchmark</body>
"""


class FixtureHandler(BaseHTTPRequestHandler):
    requests = []
    requests_lock = threading.Lock()

    def do_GET(self):  # noqa: N802 - BaseHTTPRequestHandler API
        query = parse_qs(urlparse(self.path).query)
        instance = query.get("instance", [""])[0]
        with self.requests_lock:
            self.requests.append(
                {"instance": instance, "context": self.headers.get("X-Context", "")}
            )
        body = FIXTURE_HTML.encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


class ProxyHandler(BaseHTTPRequestHandler):
    target_port = None
    label = ""
    requests = []
    requests_lock = threading.Lock()

    def do_GET(self):  # noqa: N802 - BaseHTTPRequestHandler API
        parsed = urlparse(self.path)
        query = parse_qs(parsed.query)
        instance = query.get("instance", [""])[0]
        with self.requests_lock:
            self.requests.append(
                {
                    "instance": instance,
                    "context": self.headers.get("X-Context", ""),
                    "proxy": self.label,
                }
            )

        path = parsed.path or "/"
        if parsed.query:
            path += f"?{parsed.query}"
        connection = http.client.HTTPConnection("127.0.0.1", self.target_port, timeout=5)
        try:
            connection.request(
                "GET",
                path,
                headers={"X-Context": self.headers.get("X-Context", "")},
            )
            response = connection.getresponse()
            body = response.read()
        finally:
            connection.close()

        self.send_response(response.status)
        for name, value in response.getheaders():
            if name.lower() not in {"connection", "content-length", "transfer-encoding"}:
                self.send_header(name, value)
        self.send_header("X-Proxy-Label", self.label)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Connection", "close")
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_args):
        pass


def free_port():
    server = ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
    port = server.server_port
    server.server_close()
    return port


def rss_mb(process):
    try:
        total = process.memory_info().rss
        for child in process.children(recursive=True):
            try:
                total += child.memory_info().rss
            except psutil.Error:
                pass
        return total / (1024 * 1024)
    except psutil.Error:
        return 0.0


def percentile(values, fraction):
    ordered = sorted(values)
    if len(ordered) == 1:
        return ordered[0]
    index = min(len(ordered) - 1, int(round((len(ordered) - 1) * fraction)))
    return ordered[index]


def summarize(values):
    values = [round(value, 3) for value in values]
    return {
        "p50_ms": round(statistics.median(values), 3),
        "p95_ms": round(percentile(values, 0.95), 3),
    }


def run_level(browser, url, process, count, proxy_servers):
    contexts = []
    pages = []
    creation_ms = []
    navigation_ms = []
    state_ms = []
    state_failures = []
    proxy_failures = []
    page_reachability_failures = []
    peak_rss = rss_mb(process)

    for index in range(count):
        token = f"context-{count}-{index}"
        proxy_label, proxy_port = proxy_servers[index]
        started = time.perf_counter()
        context = browser.new_context(
            viewport={"width": 1280, "height": 720},
            proxy={"server": f"http://127.0.0.1:{proxy_port}"},
        )
        context.set_extra_http_headers({"X-Context": token})
        creation_ms.append((time.perf_counter() - started) * 1000)
        page = context.new_page()
        started = time.perf_counter()
        page.goto(f"{url}?instance={token}")
        navigation_ms.append((time.perf_counter() - started) * 1000)

        started = time.perf_counter()
        result = page.evaluate(
            """token => {
                localStorage.setItem('owner', token);
                sessionStorage.setItem('owner', token);
                document.cookie = 'owner=' + token + '; Path=/';
                globalThis.__owner = token;
                return new Promise(resolve => {
                    const source = "onmessage = function(event) { postMessage(event.data); }";
                    const objectUrl = URL.createObjectURL(
                        new Blob([source], {type: 'application/javascript'})
                    );
                    const worker = new Worker(objectUrl);
                    worker.onmessage = event => {
                        worker.terminate();
                        URL.revokeObjectURL(objectUrl);
                        resolve({
                            local: localStorage.getItem('owner'),
                            session: sessionStorage.getItem('owner'),
                            cookie: document.cookie,
                            document: globalThis.__owner,
                            worker: event.data,
                        });
                    };
                    worker.postMessage(token);
                });
            }""",
            token,
        )
        state_ms.append((time.perf_counter() - started) * 1000)
        expected = {
            "local": token,
            "session": token,
            "cookie": f"owner={token}",
            "document": token,
            "worker": token,
        }
        if result != expected:
            state_failures.append({"token": token, "expected": expected, "got": result})
        contexts.append(context)
        pages.append((token, page, proxy_label))
        peak_rss = max(peak_rss, rss_mb(process))

    # Re-read every context after the whole batch has been populated. This
    # catches shared storage or document state that a same-context write/read
    # would miss because the last writer could mask the leak.
    for token, page, proxy_label in pages:
        try:
            result = page.evaluate(
                """async token => ({
                    local: localStorage.getItem('owner'),
                    session: sessionStorage.getItem('owner'),
                    cookie: document.cookie,
                    document: globalThis.__owner,
                    proxy: await fetch('/fixture.html?instance=' + encodeURIComponent(token) + '&probe=post')
                        .then(response => response.headers.get('x-proxy-label')),
                })""",
                token,
            )
        except Exception as error:
            page_reachability_failures.append(
                {"token": token, "expected_after_batch": "live page", "error": str(error)}
            )
            continue
        expected = {
            "local": token,
            "session": token,
            "cookie": f"owner={token}",
            "document": token,
        }
        actual_state = {key: result.get(key) for key in expected}
        if actual_state != expected:
            state_failures.append(
                {"token": token, "expected_after_batch": expected, "got": actual_state}
            )
        if result.get("proxy") != proxy_label:
            proxy_failures.append(
                {"token": token, "expected_proxy": proxy_label, "got_proxy": result.get("proxy")}
            )

    with ProxyHandler.requests_lock:
        proxy_captured = [request for request in ProxyHandler.requests if request["instance"].startswith(f"context-{count}-")]
    for index in range(count):
        token = f"context-{count}-{index}"
        expected_proxy = proxy_servers[index][0]
        matching = [request for request in proxy_captured if request["instance"] == token]
        if not any(request["context"] == token for request in matching):
            proxy_failures.append(
                {
                    "token": token,
                    "expected_header": token,
                    "got_headers": [request["context"] for request in matching],
                }
            )
        if not any(request["proxy"] == expected_proxy for request in matching):
            proxy_failures.append(
                {
                    "token": token,
                    "expected_proxy": expected_proxy,
                    "got_proxies": [request["proxy"] for request in matching],
                }
            )

    started = time.perf_counter()
    for context in contexts:
        context.close()
    teardown_ms = (time.perf_counter() - started) * 1000
    after_teardown_rss = rss_mb(process)

    return {
        "contexts": count,
        "creation": summarize(creation_ms),
        "navigation": summarize(navigation_ms),
        "state_check": summarize(state_ms),
        "teardown_ms": round(teardown_ms, 3),
        "rss": {
            "peak_mb": round(peak_rss, 3),
            "after_teardown_mb": round(after_teardown_rss, 3),
        },
        "state_failures": state_failures,
        "proxy_failures": proxy_failures,
        "page_reachability_failures": page_reachability_failures,
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--levels",
        default=",".join(str(level) for level in LEVELS),
        help="comma-separated context counts",
    )
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    levels = [int(value) for value in args.levels.split(",")]
    if args.repetitions < 1 or any(level < 1 for level in levels):
        parser.error("levels and repetitions must be positive")

    binary = os.path.abspath(os.environ.get("OBSCURA_BIN", "target/release/obscura"))
    cdp_port = free_port()
    fixture_port = free_port()
    fixture_server = ThreadingHTTPServer(("127.0.0.1", fixture_port), FixtureHandler)
    threading.Thread(target=fixture_server.serve_forever, daemon=True).start()
    proxy_servers = []
    proxy_httpd = []
    for index in range(max(levels)):
        proxy_port = free_port()
        proxy_class = type(
            f"ContextProxy{index}",
            (ProxyHandler,),
            {"target_port": fixture_port, "label": f"proxy-{index}"},
        )
        proxy_server = ThreadingHTTPServer(("127.0.0.1", proxy_port), proxy_class)
        threading.Thread(target=proxy_server.serve_forever, daemon=True).start()
        proxy_servers.append((f"proxy-{index}", proxy_port))
        proxy_httpd.append(proxy_server)
    fixture_url = f"http://127.0.0.1:{fixture_port}/fixture.html"
    command = [
        binary,
        "serve",
        "--port",
        str(cdp_port),
        "--allow-private-network",
        "--quiet",
    ]
    os.environ.setdefault("PLAYWRIGHT_DISABLE_FORCED_CHROMIUM_PROXIED_LOOPBACK", "1")
    process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    server_process = psutil.Process(process.pid)

    all_rows = []
    try:
        with sync_playwright() as playwright:
            deadline = time.time() + 15
            browser = None
            while browser is None and time.time() < deadline:
                try:
                    browser = playwright.chromium.connect_over_cdp(
                        f"http://127.0.0.1:{cdp_port}"
                    )
                except Exception:
                    time.sleep(0.05)
            if browser is None:
                raise RuntimeError("Obscura CDP server did not start")
            try:
                for repetition in range(args.repetitions):
                    for level in levels:
                        row = run_level(browser, fixture_url, server_process, level, proxy_servers)
                        row["repetition"] = repetition + 1
                        all_rows.append(row)
            finally:
                browser.close()
    finally:
        fixture_server.shutdown()
        for proxy_server in proxy_httpd:
            proxy_server.shutdown()
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=5)

    result = {
        "binary": binary,
        "levels": levels,
        "repetitions": args.repetitions,
        "fixture": "local static HTML, 1280x720 viewport",
        "results": all_rows,
        "state_failures": sum(
            len(row["state_failures"])
            + len(row["proxy_failures"])
            + len(row["page_reachability_failures"])
            for row in all_rows
        ),
    }
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print("Obscura context benchmark")
        print(f"binary: {binary}")
        print(f"fixture: {result['fixture']}; repetitions: {args.repetitions}")
        print("contexts  nav p50/p95 ms  state p50/p95 ms  teardown ms  peak RSS MB  state")
        for level in levels:
            rows = [row for row in all_rows if row["contexts"] == level]
            nav = [row["navigation"]["p50_ms"] for row in rows]
            nav95 = [row["navigation"]["p95_ms"] for row in rows]
            state = [row["state_check"]["p50_ms"] for row in rows]
            state95 = [row["state_check"]["p95_ms"] for row in rows]
            teardown = [row["teardown_ms"] for row in rows]
            peak = [row["rss"]["peak_mb"] for row in rows]
            failures = sum(
                len(row["state_failures"])
                + len(row["proxy_failures"])
                + len(row["page_reachability_failures"])
                for row in rows
            )
            print(
                f"{level:>8}  {statistics.median(nav):>5.1f}/{statistics.median(nav95):<5.1f}"
                f"       {statistics.median(state):>5.1f}/{statistics.median(state95):<5.1f}"
                f"       {statistics.median(teardown):>7.1f}"
                f"       {statistics.median(peak):>7.1f}     {'PASS' if not failures else 'FAIL'}"
            )
    if result["state_failures"]:
        raise SystemExit(f"state leakage failures: {result['state_failures']}")


if __name__ == "__main__":
    main()
