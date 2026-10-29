"""Offline build-source verification. This is not Runtime package admission."""
import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import tarfile

HERE = Path(__file__).resolve().parent
MAX_ARCHIVE = 33_554_432
MAX_EXPANDED = 335_544_320
MAX_FILES = 20_000


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def read_bounded(path, limit):
    with path.open("rb") as stream:
        data = stream.read(limit + 1)
    require(len(data) <= limit, "file budget")
    return data


def read_json(path):
    return json.loads(read_bounded(path, 65_536), object_pairs_hook=unique_object)


def git_hash(kind, data):
    # Git's upstream object format uses SHA-1. The downloaded archive also has
    # its independent SHA-256 pin; this does not introduce a new trust root.
    prefix = kind.encode() + b" " + str(len(data)).encode() + b"\0"
    return hashlib.sha1(prefix + data, usedforsecurity=False).hexdigest()


def git_objects(tag, commit, pin):
    payload = tag["verification"]["payload"].encode()
    signature = tag["verification"]["signature"].encode()
    require(git_hash("tag", payload + signature) == pin["tag"], "tag object hash")
    expected = f'object {pin["commit"]}\ntype commit\ntag n{pin["version"]}\n'.encode()
    require(payload.startswith(expected), "tag target")
    raw = commit["verification"]["payload"].encode()
    sig = commit["verification"]["signature"].encode()
    headers, message = raw.split(b"\n\n", 1)
    signed = headers + b"\ngpgsig " + sig.rstrip(b"\n").replace(b"\n", b"\n ") + b"\n\n" + message
    require(git_hash("commit", signed) == pin["commit"], "commit object hash")
    require(raw.startswith(f'tree {pin["tree"]}\n'.encode()), "commit tree")
    return payload, signature


def read_archive(data, top):
    require(len(data) <= MAX_ARCHIVE, "archive budget")
    with gzip.GzipFile(fileobj=io.BytesIO(data)) as stream:
        expanded = stream.read(MAX_EXPANDED + 1)
    require(len(expanded) <= MAX_EXPANDED, "expanded tar budget")
    entries, seen, folded = [], set(), set()
    total = 0
    with tarfile.open(fileobj=io.BytesIO(expanded), mode="r:") as package:
        for member in package:
            require(len(seen) < MAX_FILES, "archive inventory budget")
            name = member.name.rstrip("/")
            parts = name.split("/")
            require(parts[0] == top and all(p and p not in (".", "..") for p in parts), "archive path")
            require(not PurePosixPath(name).is_absolute() and "\\" not in name and ":" not in name, "archive path")
            require(all(32 <= ord(c) <= 126 for c in name), "archive name encoding")
            require(name not in seen and name.lower() not in folded, "duplicate archive path")
            seen.add(name)
            folded.add(name.lower())
            require(member.isdir() or member.isfile(), "archive special file")
            require(member.mode & 0o7000 == 0, "archive special mode")
            if member.isdir():
                continue
            require(len(parts) > 1 and 0 <= member.size <= 16_777_216, "source file budget")
            total += member.size
            require(total <= 268_435_456, "source inventory byte budget")
            source = package.extractfile(member)
            require(source is not None, "source body")
            body = source.read(member.size + 1)
            require(len(body) == member.size, "source body length")
            entries.append((tuple(parts[1:]), body, bool(member.mode & 0o111)))
    require(entries, "empty source archive")
    return entries


def tree_hash(entries):
    root = {}
    for parts, body, executable in entries:
        node = root
        for name in parts[:-1]:
            node = node.setdefault(name, {})
            require(isinstance(node, dict), "tree path collision")
        require(parts[-1] not in node, "duplicate tree path")
        node[parts[-1]] = ("100755" if executable else "100644", git_hash("blob", body))

    def walk(node):
        records = []
        for name, value in node.items():
            if isinstance(value, dict):
                mode, sha, order = "40000", walk(value), name + "/"
            else:
                mode, sha = value
                order = name
            records.append((order.encode(), mode.encode() + b" " + name.encode() + b"\0" + bytes.fromhex(sha)))
        return git_hash("tree", b"".join(record for _, record in sorted(records)))

    return walk(root)


def verify_or_extract(destination, entries, extract):
    if destination.exists():
        require(destination.is_dir() and not destination.is_symlink(), "existing source directory")
        actual = set()
        for parent, dirs, files in os.walk(destination, followlinks=False):
            require(not any((Path(parent) / d).is_symlink() for d in dirs), "source directory link")
            for name in files:
                path = Path(parent) / name
                require(path.is_file() and not path.is_symlink(), "source special file")
                actual.add(tuple(path.relative_to(destination).parts))
                require(len(actual) <= MAX_FILES, "existing source inventory budget")
        require(actual == {parts for parts, _, _ in entries}, "existing source inventory")
        for parts, body, _ in entries:
            require(read_bounded(destination.joinpath(*parts), 16_777_216) == body, "existing source changed")
    elif extract:
        destination.mkdir()
        for parts, body, executable in entries:
            output = destination.joinpath(*parts)
            output.parent.mkdir(parents=True, exist_ok=True)
            with output.open("xb") as stream:
                stream.write(body)
            output.chmod(0o755 if executable else 0o644)


def verify(root, extract):
    lock = read_json(HERE / "source-lock.json")
    require(lock["formatVersion"] == 1 and lock["maturity"] == "prototype", "source lock version")
    ffmpeg = lock["ffmpeg"]
    key = read_bounded(HERE / "git-tag-key.asc", 65_536)
    require(hashlib.sha256(key).hexdigest() == ffmpeg["keySha256"], "source key bytes")
    payload, signature = git_objects(read_json(HERE / "ffmpeg-tag.json"), read_json(HERE / "ffmpeg-commit.json"), ffmpeg)
    keyring = root / "gpg-source-verification"
    keyring.mkdir(mode=0o700, exist_ok=True)
    require(keyring.is_dir() and not keyring.is_symlink(), "build keyring directory")
    gpg = shutil.which("gpg")
    require(gpg is not None, "GnuPG build prerequisite missing")
    command = [gpg, "--no-options", "--homedir", str(keyring), "--batch", "--no-auto-key-retrieve"]
    fingerprints = subprocess.run(command + ["--with-colons", "--import-options", "show-only", "--import", str(HERE / "git-tag-key.asc")], check=True, capture_output=True).stdout
    pins = [line.split(b":")[9].decode() for line in fingerprints.splitlines() if line.startswith(b"fpr:")]
    require(pins and pins[0] == ffmpeg["signer"], "source signer fingerprint")
    subprocess.run(command + ["--import", str(HERE / "git-tag-key.asc")], check=True, capture_output=True)
    payload_path, signature_path = keyring / "tag.payload", keyring / "tag.signature"
    payload_path.write_bytes(payload)
    signature_path.write_bytes(signature)
    status = subprocess.run(command + ["--status-fd", "1", "--verify", str(signature_path), str(payload_path)], check=True, capture_output=True).stdout
    require(f'[GNUPG:] VALIDSIG {ffmpeg["signer"]} '.encode() in status, "source signature")
    require(not any(marker in status for marker in (b"EXPSIG", b"EXPKEYSIG", b"REVKEYSIG")), "expired or revoked source signature")
    specifications = (
        ("ffmpeg", f'FFmpeg-n{ffmpeg["version"]}', "ffmpeg-verified-n8.1.2"),
        ("opus", "opus-1.6.1", "opus-1.6.1"),
        ("libvpx", "libvpx-" + lock["libvpx"]["commit"], "libvpx-" + lock["libvpx"]["commit"]),
    )
    verified = []
    for name, top, destination in specifications:
        pin = lock[name]
        archive = read_bounded(root / "sources" / pin["archive"], MAX_ARCHIVE)
        require(hashlib.sha256(archive).hexdigest() == pin["sha256"], "source archive hash: " + name)
        entries = read_archive(archive, top)
        if name == "ffmpeg":
            require(tree_hash(entries) == ffmpeg["tree"], "archive Git tree")
        verified.append((root / "sources" / destination, entries))
    # All archives pass before any source directory is created. Existing trees
    # must match every expected byte and contain no unreferenced file.
    for destination, entries in verified:
        verify_or_extract(destination, entries, extract)
    receipt = {"formatVersion": 1, "maturity": "prototype", "sourceLockSha256": hashlib.sha256((HERE / "source-lock.json").read_bytes()).hexdigest(), "signer": ffmpeg["signer"], "tag": ffmpeg["tag"], "commit": ffmpeg["commit"], "tree": ffmpeg["tree"], "sourceFiles": [len(entries) for _, entries in verified]}
    print(json.dumps(receipt, sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("build_root", type=Path)
    parser.add_argument("--extract", action="store_true")
    arguments = parser.parse_args()
    verify(arguments.build_root.resolve(strict=True), arguments.extract)
