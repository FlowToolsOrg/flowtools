# G7 desktop shell engineering closeout

Date: 2026-10-27. Scope: P3.1a1–P3.1a5 desktop entry, appearance, read-only
Runtime settings/tray snapshots and bounded diagnostic preview/download.

The maintainer explicitly requested: “人工不验收了，直接快速完成这个任务进行下一任务”.
Remaining manual acceptance for this G7 work is waived by that instruction.
Do not block subsequent implementation on tray clicks, native layout, remaining
save scenarios or OS/window recovery exercises. These checks were not run and
must never be reported as passed. Earlier completed native evidence is retained.

The implemented desktop-shell scope is closed for this work sequence, at
prototype maturity. This is not completion of the unimplemented scheduler,
launcher/search or file/media/schedule workflows in G7, nor production or
independent security acceptance. Those remain separate implementation tasks.

## Automated evidence

Product `ca062a4`, evidence `ea2bedb`: full remote Windows quality gates and clean
worktree check passed in the [PR run](https://github.com/FlowToolsOrg/flowtools/actions/runs/40454217791).
The [push run](https://github.com/FlowToolsOrg/flowtools/actions/runs/40454213511)
also completed its quality and clean-worktree steps successfully.

Local Desktop 99 Bun / 35 Rust unit / six binding integration tests, all eleven
workspace builds, production artifact comparisons and Rust checks passed.
The separate local full gate remains failed at Runtime 20/24 real descendant
readiness/drain tests; cause is unestablished. The waiver concerns manual work
only and does not change that result, test assertions or Runtime execution policy.
See [exact evidence](./g7-tray-runtime-status.md).

## Next implementation

Continue P3.1b with Runtime-owned, bounded scheduling rules: explicit one-shot
and finite interval occurrences, missed-run and duplicate suppression semantics.
Pure planning can be independently validated before adding durable schedule
management, authorized dispatch and the notification adapter. No plan alone
grants execution, unattended startup or business-resource access.
