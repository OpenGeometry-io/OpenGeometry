# Golden targets

Each `<target>.json` maps every golden scene key to the SHA-256 of its record text,
sorted, one key per line. The `golden` test binary (`tests/golden/`) builds every
scene, hashes it, and fails on any key that is missing from the file, any key in the
file with no scene, and any key whose hash differs. The texts of the differing scenes
are written under `target/tmp/golden/<target>/`.

The file stem is the target triple with `_` written as `-`, because file names in the
tree are kebab-case.

## Recorded targets

| Target | File | How it was produced |
| --- | --- | --- |
| aarch64-apple-darwin | `aarch64-apple-darwin.json` | `OG_GOLDEN_UPDATE=1 cargo test --offline --test golden` from `opengeometry/`, debug profile, rustc 1.88.0 (6b00bc388 2025-06-23), macOS 26.4.1 (25E253), Apple silicon |
| x86-64-unknown-linux-gnu | `x86-64-unknown-linux-gnu.json` | not recorded yet: the file is `{}`, so the test fails with the record instruction until a manual start of `.github/workflows/verify.yml` with `record: goldens` uploads it as an artifact and the file is reviewed and committed |

CI's macOS job compares with `aarch64-apple-darwin.json`, which was recorded on the macOS
version the table names; a runner on another macOS version can differ if its math library
rounds differently (the first of the 'Known sources of difference between targets').

## Recording

Only `OG_GOLDEN_UPDATE=1` writes a golden file. It is refused when `GITHUB_ACTIONS`
is set, unless `GITHUB_EVENT_NAME` is `workflow_dispatch`, so a CI run can never
record the file it then compares against. Review the diff of the recorded file before
committing it. On CI the recording is the `record` job of `.github/workflows/verify.yml`,
started by hand with `record: goldens`: it uploads the golden files as the
`goldens-<os>` artifact and never commits.

`OG_GOLDEN_DUMP=<dir>` (used by `npm run snapshot:kernel -- <dir>`) writes the record
text of every case and the three handler listings into an empty directory, skips the
golden comparison and still applies the corpus checks.

## Debug profile only

The goldens are recorded and compared in the debug profile. Release builds let the
compiler fuse and reorder floating-point work, which moves low bits; at the time of
recording, 82 of the 461 cases differ between debug and release. A release run of the
golden binary fails by design.

## Known sources of difference between targets

- The platform math library: `sin`, `cos`, `atan2`, `sqrt`-based helpers and `powf`
  may round differently on another libm, which moves curved-surface tessellation,
  classification grids and STEP reals.
- Release-mode fusion (see above).
- Architecture-specific code generation for fused multiply-add.

## Not recorded

wasm32 is not recorded. It waits for the embedded fixtures of the deferred wasm comparison
work: the golden test reads the boolean matrix rows, the three batch-matrix fallback rows
and the kernel's boolean handler source files from disk, which a wasm test run cannot do.

## Differing keys per target

Fill this when a second target is recorded: list every key whose hash differs from the
aarch64-apple-darwin file, and the reason.

| Target | Differing keys | Reason |
| --- | --- | --- |
| x86-64-unknown-linux-gnu | not recorded yet | |

## What the goldens carry

- Every case of the corpus (461): builders, profiles, the boolean matrix (its rows read
  from `tests/fixtures/cases/boolean-matrix`) and the five single-body boolean cases
  (`booleans.fixture.*`, built in code in `tests/golden/boolean_fixtures.rs`; they read no
  file), the cylinder, sphere, direct, planar, oblique and curved booleans, the
  batch and staged batch subtractions, shells, creating operations and the world graph
  session, operate and scenario cases.
- The three handler listings: `handlers-expected`, `handlers-covered`,
  `handlers-uncovered`.
- `sweep-3d`: the non-coplanar rectangular sweep (BRep JSON, tessellation at 0.01,
  three classify verdicts, the STEP text and report), hashed as the earlier single
  golden did, so its value continues the earlier `aarch64-apple-darwin` hash.
- `fallback.<row>`: the three batch-matrix fallback rows run through
  `multi_tool_boolean` (result or error, handlers, tessellation, classification, STEP
  in metre and millimetre).
- `level.step.<unit>.<axis>`: the acceptance level exported as STEP in metre and
  millimetre with Y and Z up (text and report).

## Adding a target

A further target needs its own `include_str!` line in the `COMMITTED` table of
`tests/golden/golden_file.rs`, a recorded `<target>.json`, and a row in the tables
above.
