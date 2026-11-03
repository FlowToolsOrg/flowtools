"""Offline official tarball verification for the explicit developer build."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import uuid

from verify_sources import HERE, MAX_ARCHIVE, read_archive, read_bounded, read_json, require, verify_or_extract


def verify(root, codec_root):
    pin = read_json(HERE / "official-8.1.3-lock.json")
    require(pin["version"] == "8.1.3" and pin["maturity"] == "prototype", "official source lock")
    require(pin["signer"] == "FCF986EA15E6E293A5644F10B4322F04D67658D8", "official signer")
    archive = read_bounded(root / "upstream/ffmpeg-8.1.3.tar.xz", MAX_ARCHIVE)
    key = read_bounded(HERE / "ffmpeg-release-key.asc", 16_384)
    signature = read_bounded(HERE / "ffmpeg-8.1.3.tar.xz.asc", 16_384)
    for data, expected in ((archive, pin["archiveSha256"]), (key, pin["keySha256"]), (signature, pin["signatureSha256"])):
        require(hashlib.sha256(data).hexdigest() == expected, "official source bytes")
    # gpgv uses only these freshly created public bytes. Feed the already hashed
    # archive on stdin, avoiding a second source-path read or a user keyring.
    gpgv = shutil.which("gpgv")
    require(gpgv is not None, "GnuPG verifier prerequisite missing")
    home = root / ("verification-" + uuid.uuid4().hex)
    home.mkdir(mode=0o700)
    public = home / "upstream-public-key.gpg"
    public.write_bytes(base64.b64decode(b"".join(re.findall(rb"(?m)^[A-Za-z0-9+/]{1,76}={0,2}\r?$", key)), validate=True))
    detached = home / "release.asc"
    detached.write_bytes(signature)
    status = subprocess.run([gpgv, "--homedir", str(home), "--keyring", str(public), "--status-fd", "1", str(detached), "-"], input=archive, capture_output=True, timeout=10)
    require(status.returncode == 0 and status.stdout.count(b"[GNUPG:] VALIDSIG " + pin["signer"].encode() + b" ") == 1, "official signature")
    require(not any(marker in status.stdout for marker in (b"EXPSIG", b"EXPKEYSIG", b"REVKEYSIG")), "expired or revoked official signature")
    source_lock = read_bounded(HERE / "source-lock.json", 65_536)
    require(hashlib.sha256(source_lock).hexdigest() == pin["codecSourceLockSha256"], "codec source lock")
    receipt = read_json(codec_root / "source-verification.json")
    require(receipt["formatVersion"] == 1 and receipt["maturity"] == "prototype" and receipt["sourceLockSha256"] == pin["codecSourceLockSha256"], "codec source receipt")
    entries = read_archive(archive, "ffmpeg-8.1.3", "xz")
    sources = root / "sources"
    sources.mkdir(exist_ok=True)
    verify_or_extract(sources / "ffmpeg-8.1.3", entries, True)
    print(json.dumps({"formatVersion": 1, "maturity": "prototype", "archiveSha256": pin["archiveSha256"], "signer": pin["signer"], "sourceFiles": len(entries), "codecSourceLockSha256": pin["codecSourceLockSha256"]}, sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("build_root", type=Path)
    parser.add_argument("codec_root", type=Path)
    arguments = parser.parse_args()
    verify(arguments.build_root.resolve(strict=True), arguments.codec_root.resolve(strict=True))
