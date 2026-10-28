# G7 launcher search and explicit input contexts

2026-10-28, P3.2 implementation, prototype. Manual G7 acceptance remains waived,
not passed. File organization/media processing and the complete G7 gate remain
separate; search metadata never authorizes execution.

`@flowtools/sdk/search` provides one bounded metadata index. Registration caches
normalized title, description, aliases, keywords, pinyin and initials. Exact,
prefix, substring, alias, keyword, description, pinyin/initials and subsequence
fuzzy matches have stable reasons and deterministic ties. Pin, open frequency
and recency contribute bounded bonuses. Multiword terms are ANDed. The SDK
CommandRegistry updates the index on register/replacement/removal, including
immediate plugin disable/unload projection. Desktop rebuilds from its enabled
inventory, and the shared CommandPalette and CLI `search` use the same engine.
CLI search reads fixed build-owned metadata without importing any executor or
starting Runtime; invalid search produces JSON failure and a nonzero exit.

Regex is an explicit mode using pinned RE2JS 2.9.0, never user-created native JS
RegExp. Limits: 10k records, 4k normalized characters per record, 128 query
characters, eight terms, 512 compiled instructions, no counted repetitions or
Unicode property expansion, a 35ms scan budget, and at most 100 returned results
(Desktop shows 36). Syntax/budget failures return no partial results. Pinyin uses
pinned pinyin-pro 3.29.5. Both dependencies declare MIT licenses. This is not a
package trust/security certification. Primary references:
[pinyin API](https://pinyin-pro.cn/use/pinyin.html),
[RE2JS API and syntax limits](https://github.com/le0pard/re2js).

Desktop exposes per-command aliases/pins, bounded recent opens/frequencies,
opt-out and explicit clearing. The formatVersion-1 record stores IDs/counts and
user-selected aliases only, never queries, clipboard contents or command input.
It rejects oversized/unknown/inconsistent records, preserves unreadable bytes
until an explicit selection/reset, reports failed writes and leaves controls
usable. Clearing removes only these preferences' history plus the old known
recent-ID key. Recently opened does not claim successful business execution.

Context acquisition is gesture-only: entered text, this window's selected text,
explicit clipboard text, one selected UTF-8 text file or PNG/JPEG/GIF/WebP image.
Text is bounded to 64 KiB and image bytes to 256 KiB, with size agreement, UTF-8
refusal and image header checks. No mount/shortcut/background read, OS foreground
selection scraping, directory access, file path, write grant or data persistence
is added. Fixed Host bindings transfer text to Base64/hash/JSON/regex input and
images to image-Base64 input; other/HTML commands do not receive the data. The
handoff is consumed once into a preview. A separate Run action still uses the
existing schema, Runtime identity/grant/epoch and execution envelope. It never
auto-executes. Late reads cannot update a discarded context; failed acquisition
clears the previous context. These are input contexts, not filesystem handles
or the file/media office workflows.

Keyboard paths ignore IME composition Enter. The shared palette has a named
modal surface, bounded rows, focus restoration/trapping and separate mode
selection; Enter in the mode selector cannot trigger a command.

Validation: all 790 SDK tests, 101 CLI tests, seven pinned Chromium search/context
tests and two Desktop preference tests passed. Browser coverage uses the actual
LauncherView with native inventory/navigation mocked, actual shared search,
clipboard gestures, a real browser-generated PNG, preview-before-run and actual
Base64 execution. The existing four execution-panel and six schedule tests also
passed with the new preview prop. Types/lint passed for the edited workspaces.
One early SDK Node fixture hit its unchanged 5s bound; the complete final SDK
suite passed, including that fixture at 255ms. No timeout/assertion was relaxed.
An initial CLI JSON-error regression was fixed; the complete CLI suite passed.

Reproduce benchmark: `bun run --cwd packages/sdk build`, then
`bun run --cwd packages/sdk benchmark:search`. Actual Node 24.16.0 / Windows x64 /
Core Ultra 9 275HX / 32 GiB fixture: 10k mixed Chinese/English commands, 25 warmups
and 100 measured samples per mode. Build 54.02ms; text p95 4.76ms, regex p95
21.08ms, both below 50ms. Indexed heap delta 11,969,384 bytes; whole Node RSS
78,053,376 before / 120,094,720 indexed / 157,573,120 after queries. The standalone
JSON is `execution-validation/g7-search/benchmark-20261028.json` (exit 0).
These are this fixture/process's observations, not whole-Desktop memory, cold
launch or hotkey-wake SLA evidence. The first integrated gate on `6016f26` failed
the runner frame fixture's unchanged 30-second bound; its actual Node Host repair
and retained assertions are recorded [separately](./g7-runner-protocol-host.md).
Complete root validation of the repaired tree is next.
