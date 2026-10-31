# Build-only real-media candidate

This directory prepares a controlled Windows/x64 FFmpeg candidate for the separate
LPAC acceptance workspace. It grants no Runtime trust, installation, executable
identity or production approval. The reviewed production list stays empty.

Use a developer Linux/WSL build root with Python 3, GnuPG, native GCC, the MinGW
x86_64 GCC/binutils toolchain, make, NASM and pkgconf. No package installer is run
by this recipe. Place the three exact archives from `source-lock.json` in
`<build-root>/sources/`. Source downloads are data; inspect/verify them before
running the build. The verifier has no network or unsigned-source fallback.

```sh
python3 -m unittest discover -s scripts/media-candidate -p 'test_*.py'
python3 -O -m unittest discover -s scripts/media-candidate -p 'test_*.py'
bash scripts/media-candidate/build.sh /absolute/developer/build-root
```

The verifier checks original archive hashes, the pinned upstream public certificate
and tag signature, exact tag/commit objects and the archive's complete Git tree.
Existing source trees must match every expected file. Build output is under that
root's `build`, `prefix` and `candidate`; source materials and logs are retained.
The `candidate` contains two static executables and license materials, never an
installer script. Build tooling still requires its own source/license review.

The recorded candidate hashes identify a particular local build. Reproducibility
across machines/build roots has not been demonstrated. A different binary hash
must be reviewed as a new candidate and intentionally recorded; preparation never
accepts a caller-supplied hash or automatically refreshes the tracked lock.

After the exact candidate has been recorded, on Windows from repository root:

```powershell
pwsh -NoProfile -File scripts/test-media-candidate.ps1 -BuildRoot 'C:\path\to\build-root'
```

This requires the build's matching `source-verification.json`. It checks both
images before writing fixed ignored fixture outputs, then runs fmt, nonempty Rust
inventory, warnings-as-errors Clippy and all six real-process tests. Missing images,
changed bytes or any failed command fail the task. Neither root CI nor ordinary
Runtime build/test silently invokes or skips this separate acceptance workspace.
See [actual scope, failures and evidence](../../docs/validation/g6-media-candidate.md).
