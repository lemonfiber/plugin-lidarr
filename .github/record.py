#!/usr/bin/env python3
"""Record every conformance case of the contract the adapter speaks.

**This is CI harness, not plugin content.** Nothing under `.github/` is
installed, and lemonfiber never runs any of it (`F3-R6`).

Each recording under `conformance/<capability>@<major>/<case>.json` is the
adapter's answer to one case of that contract's `conformance.json` (ARCH-R217),
asked with the case's own `asked`, its placeholders filled, in the order the
contract lists them. It is taken against a throwaway Servarr service at the
digest `plugin.toml` pins for the plugin's own service, on a network with no
route off the host, so no answer depends on a catalogue or an indexer. The
download client the cases register is a stand-in at the address they name,
answering as SABnzbd answers, so Lidarr takes it and tests it without anything
being downloaded. The adapter runs from this repository's Dockerfile, as
lemonfiber runs it: the caller's uid, read-only, no capabilities, the key and
the upstream credential owner-only in its configuration directory.

This script stands where lemonfiber stands: it mints the service's API key as
32 lowercase hex characters, hands it to the service through its environment,
and writes it to the adapter's `upstream.json` beside `lemonfiber.key`. The
cases asked are every case bound to no media type and every one bound to a
media type the service files, as `plugin.toml` declares them, and a recording
left from a case no longer asked is removed. A run whose answers hold the key,
the API key or a credential the run minted writes nothing.

    python3 .github/record.py            write conformance/
    python3 .github/record.py --check    record again and compare

Needs Docker. Exit 0 = recorded (or matched), 1 = it did not.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import re
import secrets
import subprocess
import sys
import tempfile
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import uuid

ROOT = pathlib.Path(__file__).resolve().parents[1]
RECORDINGS = ROOT / "conformance"
CONTRACTS = "https://raw.githubusercontent.com/lemonfiber/lemonfiber/{rev}/contract/capabilities/{capability}/v{major}/conformance.json"
JSON = "application/json"
STARTUP_S = 180
ANSWER_S = 60
ADAPTER_PORT = 8080
SEGMENT = re.compile(r"[a-z][a-z0-9_.]*")
MAJOR = re.compile(r"[1-9]\d*")
PLACEHOLDER = re.compile(r"\{([^{}]+)\}")
ANSWERED = re.compile(r"([a-z][a-z0-9_.]*@[1-9]\d*/[a-z0-9_-]+)#(/.*)")
MINTED = "minted"
UPSTREAM = "upstream:"
API_KEY = "api_key"
DATA_FOLDER = "conformance"
CLIENT_HOST = "conformance-download"
CLIENT_CATEGORY = "music"
CLIENT_IMAGE = "python:3.13-alpine@sha256:bf12ecc0bea3f171d55145dd02c78b5eeb09f30ee2ac9cc2b4c9e4e8f5e30741"
CLIENT = """
import http.server, json, os, urllib.parse
CATEGORY = os.environ["CATEGORY"]
SAID = {
    "version": {"version": "4.5.1"},
    "get_config": {"config": {"misc": {"complete_dir": "/downloads/complete", "pre_check": False,
                                       "enable_tv_sorting": False, "enable_movie_sorting": False,
                                       "enable_date_sorting": False},
                              "categories": [{"name": name, "dir": ""} for name in ("*", CATEGORY)],
                              "sorters": []}},
    "queue": {"queue": {"slots": [], "paused": False}},
    "history": {"history": {"slots": []}},
}
class Answering(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        mode = urllib.parse.parse_qs(urllib.parse.urlsplit(self.path).query).get("mode", [""])[0]
        body = json.dumps(SAID.get(mode, {"status": True})).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(body)
http.server.ThreadingHTTPServer(("", 80), Answering).serve_forever()
"""
NOTES = {
    "refuses-without-the-key": "Asked without the plugin's key: the kit refuses it before the adapter is reached.",
    "identity-answers": "Who a fresh Lidarr says it is, and the release it runs.",
    "register_download_client-answers": "A SABnzbd client as the case names it, which Lidarr tests against the stand-in and keeps.",
    "download_clients-answers": "That client, by where it is reached and the category it files under.",
    "update_download_client-answers": "That client written again as the case names it.",
    "set_client_field-answers": "The case's field written on that client, the rest of it as Lidarr gave it.",
    "test_download_clients-answers": "Every client Lidarr tests: the one registered, which the stand-in answers.",
    "register_root_folder-answers-music": "The folder the case names, taken as a root folder with Lidarr's first metadata and quality profiles.",
    "root_folders-answers": "That folder, read back with the id Lidarr gave it.",
    "quality_profiles-answers": "The quality profiles a fresh Lidarr holds.",
    "hardlinks-answers": "Whether a fresh Lidarr imports by hard link: it does.",
    "set_hardlinks-answers": "Importing by hard link set, every other media-management setting kept.",
    "records-answers-music": "The artists a fresh Lidarr holds, for carrying elsewhere: none.",
    "carry-answers-music": "A carried artist whose record is empty: refused before Lidarr is asked.",
    "indexer_count-answers": "How many indexers a fresh Lidarr searches: none.",
    "queue-answers": "What a fresh Lidarr has queued: nothing.",
    "apply_music_format-answers": "Lossless applied to every quality profile Lidarr holds.",
}


def docker(*args: str, check: bool = True, given: str | None = None) -> str:
    done = subprocess.run(["docker", *args], capture_output=True, text=True, check=False, input=given)
    if check and done.returncode != 0:
        raise RuntimeError(f"docker {args[0]}: {done.stderr.strip()}")
    return done.stdout.strip()


def manifest() -> tuple[str, str, int, int, list[str]]:
    """The plugin's own service: its id, its pinned image, the port it listens on, its API version and the media types it files."""
    read = tomllib.loads((ROOT / "plugin.toml").read_text(encoding="utf-8"))
    plugin = read["plugin"]["id"]
    upstream = next(one for one in read["service"] if one["id"] == plugin)
    return (plugin, f"{upstream['image']}@{upstream['digest']}", upstream["listens"],
            upstream["api"]["version"], upstream["media_types"])


def core_rev() -> str:
    read = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    return read["dependencies"]["lemonfiber-adapter-kit"]["rev"]


def published_cases(spoken: str) -> list[dict]:
    capability, major = spoken.split("@")
    url = CONTRACTS.format(rev=core_rev(), capability=capability, major=major)
    with urllib.request.urlopen(url, timeout=ANSWER_S) as answer:
        return json.load(answer)["cases"]


def asked_of(spoken: str, files: list[str]) -> list[dict]:
    """The cases of `spoken` an upstream filing `files` is asked: unbound, or bound to one it files."""
    return [case for case in published_cases(spoken) if case.get("media_type") in (None, *files)]


def asked(base: str, path: str, key: str | None, body: dict | None = None) -> tuple[int, str, str]:
    request = urllib.request.Request(
        f"{base}{path}",
        data=None if body is None else json.dumps(body).encode(),
        method="GET" if body is None else "POST",
        headers={"Content-Type": JSON, "Accept": JSON} | ({"Authorization": f"Bearer {key}"} if key else {}),
    )
    try:
        with urllib.request.urlopen(request, timeout=ANSWER_S) as answer:
            return answer.status, answer.headers.get("content-type", ""), answer.read().decode()
    except urllib.error.HTTPError as refused:
        return refused.code, refused.headers.get("content-type", ""), refused.read().decode()


def recording(pinned: str, path: str, status: int, kind: str, body: str, note: str) -> dict:
    response: dict = {"status": status}
    if kind:
        response["headers"] = {"content-type": kind}
    if body.strip() == "null":
        response["body_starts_with"] = "null"
    elif body.strip():
        response["json"] = json.loads(body)
    return {
        "recorded_from": pinned,
        "note": note,
        "request": {"method": "POST", "path": path, "accept": JSON},
        "response": response,
    }


def owner_only(directory: pathlib.Path) -> None:
    for file in directory.iterdir():
        file.chmod(0o600)
    directory.chmod(0o700)


def waited(what: str, ready) -> object:
    deadline = time.monotonic() + STARTUP_S
    while time.monotonic() < deadline:
        found = ready()
        if found:
            return found
        time.sleep(2)
    raise RuntimeError(f"{what} within {STARTUP_S}s")


def in_service(container: str, listens: int, version: int, api_key: str, method: str, path: str,
               body: dict | None = None) -> str:
    """The status the service answers `path` under its versioned API with, its key read from standard input."""
    command = ["exec", "-i", container, "curl", "-sS", "-o", "/dev/null", "-w", "%{http_code}",
               "-X", method, "-H", "@-", f"http://localhost:{listens}/api/v{version}{path}"]
    if body is not None:
        command[-1:-1] = ["-H", f"Content-Type: {JSON}", "--data-binary", json.dumps(body)]
    return docker(*command, check=False, given=f"X-Api-Key: {api_key}\n")


class Filling:
    """What each placeholder in a case's ask is filled with, and every credential the run made."""

    def __init__(self, upstream: dict[str, str]) -> None:
        self.upstream = upstream
        self.answers: dict[str, object] = {}
        self.made: list[str] = []

    def filled(self, value: object) -> object:
        if isinstance(value, str):
            found = PLACEHOLDER.fullmatch(value)
            return self.placeholder(found.group(1)) if found else value
        if isinstance(value, list):
            return [self.filled(item) for item in value]
        if isinstance(value, dict):
            return {name: self.filled(member) for name, member in value.items()}
        return value

    def placeholder(self, inner: str) -> str:
        if inner == MINTED:
            fresh = secrets.token_hex(16)
            self.made.append(fresh)
            return fresh
        if inner.startswith(UPSTREAM):
            field = inner.removeprefix(UPSTREAM)
            if field not in self.upstream:
                raise RuntimeError(f"upstream.json holds no {field}")
            return self.upstream[field]
        answered = ANSWERED.fullmatch(inner)
        if answered is None:
            raise RuntimeError(f"{{{inner}}} is not a placeholder this script fills")
        case, pointer = answered.groups()
        if case not in self.answers:
            raise RuntimeError(f"{case} was not asked before it")
        found = pointed(self.answers[case], pointer)
        if not isinstance(found, str):
            raise RuntimeError(f"{case} answered no string at {pointer}")
        return found


def pointed(value: object, pointer: str) -> object:
    for token in pointer.split("/")[1:]:
        token = token.replace("~1", "/").replace("~0", "~")
        if isinstance(value, dict):
            value = value.get(token)
        elif isinstance(value, list) and token.isdigit() and int(token) < len(value):
            value = value[int(token)]
        else:
            return None
    return value


def recorded_live(plugin: str, pinned: str, listens: int, version: int,
                  files: list[str]) -> tuple[list[str], dict[tuple[str, str], dict]]:
    """What the adapter speaks, and every case asked of it in front of a throwaway service."""
    name = f"{plugin}-record-{uuid.uuid4().hex[:8]}"
    inside, outside = f"{name}-inside", f"{name}-outside"
    key = secrets.token_hex(24)
    api_key = secrets.token_hex(16)
    with tempfile.TemporaryDirectory() as held:
        held = pathlib.Path(held)
        upstream, data, adapter = held / "upstream", held / "data", held / "adapter"
        for directory in (upstream, data / DATA_FOLDER, adapter):
            directory.mkdir(parents=True)
        try:
            docker("network", "create", "--internal", inside)
            docker("network", "create", outside)
            docker("run", "-d", "--name", f"{name}-client", "--network", inside,
                   "--network-alias", CLIENT_HOST, "-e", f"CATEGORY={CLIENT_CATEGORY}",
                   CLIENT_IMAGE, "python3", "-c", CLIENT)
            docker("run", "-d", "--name", f"{name}-upstream", "--network", inside,
                   "--network-alias", plugin, "-e", f"PUID={os.getuid()}", "-e", f"PGID={os.getgid()}",
                   "-e", "TZ=Etc/UTC", "-e", f"{plugin.upper()}__AUTH__APIKEY={api_key}",
                   "-v", f"{upstream}:/config", "-v", f"{data}:/data", pinned)
            waited("the service did not answer with the key minted for it",
                   lambda: in_service(f"{name}-upstream", listens, version, api_key, "GET",
                                      "/system/status") == "200")
            (adapter / "lemonfiber.key").write_text(key, encoding="utf-8")
            (adapter / "upstream.json").write_text(json.dumps({API_KEY: api_key}), encoding="utf-8")
            owner_only(adapter)
            docker("build", "-q", "-t", f"{name}:adapter", str(ROOT))
            docker("create", "--name", f"{name}-adapter", "--network", outside,
                   "--user", f"{os.getuid()}:{os.getgid()}", "--read-only", "--cap-drop", "ALL",
                   "--security-opt", "no-new-privileges", "-v", f"{adapter}:/config:ro",
                   "-p", f"127.0.0.1::{ADAPTER_PORT}", f"{name}:adapter")
            docker("network", "connect", inside, f"{name}-adapter")
            docker("start", f"{name}-adapter")
            port = docker("port", f"{name}-adapter", str(ADAPTER_PORT)).splitlines()[0].rsplit(":", 1)[1]
            base = f"http://127.0.0.1:{port}"
            waited("the adapter was not ready", lambda: answering(base, key))
            speaks = json.loads(asked(base, "/lemonfiber/adapter/v1/about", key)[2])["speaks"]
            filling = Filling({API_KEY: api_key})
            said = recorded(base, key, pinned, speaks, files, filling)
            secret = (key, api_key, *filling.made)
            leaked = [case for case, one in said.items()
                      if any(made in json.dumps(one) for made in secret if len(made) >= 16)]
            if leaked:
                raise RuntimeError(f"an answer holds a credential, so nothing is written: {leaked}")
            return speaks, said
        finally:
            for container in (f"{name}-adapter", f"{name}-upstream", f"{name}-client"):
                docker("rm", "-fv", container, check=False)
            docker("rmi", f"{name}:adapter", check=False)
            for network in (inside, outside):
                docker("network", "rm", network, check=False)
            docker("run", "--rm", "-v", f"{held}:/held", "--entrypoint", "chown", pinned,
                   "-R", f"{os.getuid()}:{os.getgid()}", "/held", check=False)


def answering(base: str, key: str) -> bool:
    try:
        status, _, body = asked(base, "/lemonfiber/adapter/v1/ready", key)
    except (urllib.error.URLError, ConnectionError):
        return False
    return status == 200 and body.strip() == "true"


def recorded(base: str, key: str, pinned: str, speaks: list[str], files: list[str],
             filling: Filling) -> dict[tuple[str, str], dict]:
    """Every case of every contract spoken that the upstream is asked, in the order each contract lists them."""
    said: dict[tuple[str, str], dict] = {}
    for spoken in speaks:
        capability, major = spoken.split("@")
        for case in asked_of(spoken, files):
            path = operation_path(capability, major, case["operation"])
            status, kind, body = asked(base, path, key if case["keyed"] else None,
                                       filling.filled(case["asked"]))
            note = NOTES.get(case["case"], "")
            if not note:
                raise RuntimeError(f"{spoken}/{case['case']} has no note to record it with")
            one = recording(pinned, path, status, kind, body, note)
            said[(spoken, case["case"])] = one
            filling.answers[f"{spoken}/{case['case']}"] = one["response"].get("json")
    return said


def operation_path(capability: str, major: str, operation: str) -> str:
    """Where an operation is asked, each segment held to the shape the contract writes it in."""
    for segment, shape in ((capability, SEGMENT), (major, MAJOR), (operation, SEGMENT)):
        if not shape.fullmatch(segment):
            raise RuntimeError(f"{segment!r} is not a segment a contract names")
    quoted = [urllib.parse.quote(segment, safe="") for segment in (capability, major, operation)]
    return "/lemonfiber/{}/v{}/{}".format(*quoted)


def filed(said: dict[tuple[str, str], dict], spoken: str, case: str, checking: bool) -> str | None:
    """One case written to conformance/, or compared with what is there; the fault, if any."""
    got = said.get((spoken, case))
    path = RECORDINGS / spoken / f"{case}.json"
    if got is None:
        return f"{spoken}/{case} was not asked"
    if checking:
        held = json.loads(path.read_text(encoding="utf-8")) if path.is_file() else None
        return None if held == got else f"{path.relative_to(ROOT)} is not what the service answers now"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(got, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return None


def left(said: dict[tuple[str, str], dict], spoken: str, checking: bool) -> str | None:
    """Recordings of cases no longer asked: removed, or named where they are only compared."""
    held = RECORDINGS / spoken
    stale = sorted(path for path in held.glob("*.json") if (spoken, path.stem) not in said) if held.is_dir() else []
    if checking:
        return f"{', '.join(path.name for path in stale)} in {held.relative_to(ROOT)} answer no case asked" if stale else None
    for path in stale:
        path.unlink()
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="record again and compare with conformance/")
    checking = parser.parse_args().check

    plugin, pinned, listens, version, files = manifest()
    speaks, said = recorded_live(plugin, pinned, listens, version, files)
    faults = [fault for spoken in speaks for case in asked_of(spoken, files)
              if (fault := filed(said, spoken, case["case"], checking)) is not None]
    faults += [fault for spoken in speaks if (fault := left(said, spoken, checking)) is not None]
    for fault in faults:
        print(f"::error::{fault}")
    print(f"{len(said)} cases asked against {pinned}; {len(faults)} fault(s).")
    return 1 if faults else 0


if __name__ == "__main__":
    sys.exit(main())
