# QuickJS-NG engine source

- Upstream: <https://github.com/quickjs-ng/quickjs>
- Release: `v0.17.0`
- Commit: `6d46d07d04041b40f4f49eaa7fdebe44c314c699`
- Source archive: <https://codeload.github.com/quickjs-ng/quickjs/zip/6d46d07d04041b40f4f49eaa7fdebe44c314c699>
- License: [MIT](./LICENSE); original file copyright notices retained.
- `SHA256SUMS` records each unmodified necessary engine source/header/license.
  `build.rs` checks every listed file before compilation.

Only `quickjs.c`, `dtoa.c`, `libregexp.c`, `libunicode.c` and their required
headers are built. No upstream CLI, `quickjs-libc`, `std`, `os`, native module
loader, test tool or bytecode input interface is included. The three upstream
`builtin-*.h` files are required fixed engine implementation data, verified like
the engine; they do not accept package bytecode. Atomics are disabled for this
single-threaded host. This provenance is reproducibility evidence, not an audit
or production security approval of the engine.
