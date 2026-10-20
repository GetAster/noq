# Aster patch ledger: noq

This repository is Aster's fork of [n0-computer/noq](https://github.com/n0-computer/noq), the QUIC implementation used by iroh. Aster carries the patches listed below on top of upstream releases. Upstream maintainers are welcome to incorporate any of them, in whole or in part, at their discretion.

## Branch and tag model

- `main` is upstream `main` plus one commit: the Aster README preamble and this ledger. Nothing is built from `main`. It is refreshed with `git merge upstream/main` and is never reset or force-pushed. Ledger edits are commits on `main`, never on `upgrade/*`.
- `upgrade/<repo>-v<upstream version>` (for example `upgrade/iroh-v1.4.0`, `upgrade/iroh-blobs-v0.103.1`) is the patched integration line: the upstream release plus the patches in this ledger. A new upstream base always gets a new branch, cut from the upstream release tag; Aster-only fixes and re-pins append to the current branch. Per-minor names (`upgrade/iroh-blobs-v0.103`, `upgrade/salvo-v1.0`, …) are historical. Consumers pin commits on it. Published `upgrade/*` branches and tags are never rewritten; a failed attempt is superseded by new commits and a new version.
- Topic branches carry one change each, are named for the change (for example `feat/read-into`, `security/plumtree-bounds`), are based on the upstream release tag, and are cherry-picked with `git cherry-pick -x` onto the upgrade branch, so every upgrade-branch commit names its source.
- Version: the upstream version if Aster's registry has never served that version or a higher one in the series; otherwise the next patch above Aster's highest published version. A re-pin without a code change keeps the version and takes the tag suffix `-p<n>`.
- Tags: `aster-<repo>-v<published version>` (re-pins `aster-<repo>-v<published version>-p<n>`), created only after the Aster consumer trial passes against those exact commits, because tags cannot be moved.
- The fork manifest is `iroh-fork-manifest.toml` in the Aster repository (the only copy).
- Aster does not submit patches upstream; upstream maintainers are welcome to take any patch here.

Historical and current tag schemes in this repository:

- `aster-noq-v<version>`, for example `aster-noq-v1.0.0-rc.0`, `aster-noq-v1.0.1`, `aster-noq-v1.3.2`.
- `aster-noq-v<version>-p<n>`, a later commit at the same crate version: `aster-noq-v1.0.0-p1` (the hand-rolled Windows UDP fix, since upstreamed; historical: under the current model a code change takes a new version, and `-p<n>` is for re-pins only).

## Current release

| Field | Value |
|---|---|
| Upstream base | `noq-v1.3.0` (`c1f411562`) |
| Integration branch | `upgrade/noq-v1.3.0` |
| Tag | `aster-noq-v1.3.2` |
| Pinned commit | `e75f3a5550feae0f40df0ffde8bf438f213a5156` |
| Crate version | 1.3.2 |
| Crates | `noq`, `noq-udp`, `noq-proto` (all three published to the Aster registry) |
| Source repository | `https://forge.emrul.dev/Aster/noq.git` |

The Aster registry (`sparse+https://forge.emrul.dev/api/packages/Aster/cargo/`) also carries unmodified "bridge" republications of crates that depend on noq, so that an alternate-registry build never pulls a second, crates.io copy of noq. Each is the crates.io source with only `Cargo.toml` dependency metadata rewritten; publication verifies the crates.io archive checksum.

| Bridge | Source | Commit | Version | Rewrites |
|---|---|---|---|---|
| `irpc` | `https://github.com/n0-computer/irpc` | `0ed2b235d5c797b54d8263ecb3b0247272d055f9` | 0.17.0 | `noq` |
| `netwatch` | `https://github.com/n0-computer/net-tools` | `0030e57fc70981895bb86212ff0f059cc41c8a4e` | 0.19.3 | `noq-udp` |
| `portmapper` | `https://github.com/n0-computer/net-tools` | `0030e57fc70981895bb86212ff0f059cc41c8a4e` | 0.19.3 | `netwatch` |

`netwatch` still depends on `noq-udp`, so its bridge had to be republished at 0.19.3 when iroh 1.3.0 moved net-tools to 0.19.3.

Bootstrap pin block for source-tree builds (consumers install a versioned Aster SDK release instead and must not copy this):

```toml
[patch.crates-io]
noq       = { git = "https://forge.emrul.dev/Aster/noq.git", rev = "e75f3a5550feae0f40df0ffde8bf438f213a5156" }
noq-udp   = { git = "https://forge.emrul.dev/Aster/noq.git", rev = "e75f3a5550feae0f40df0ffde8bf438f213a5156" }
noq-proto = { git = "https://forge.emrul.dev/Aster/noq.git", rev = "e75f3a5550feae0f40df0ffde8bf438f213a5156" }
```

The Aster iroh fork pins this commit in its own `[patch.crates-io]` (see the iroh fork's ledger, row 3), and the iroh-blobs, iroh-docs, iroh-gossip and iroh-util forks pin it in theirs.

### `upgrade/noq-v1.3.0` stack

| Commit | Subject | Source |
|---:|---|---|
| `cd67e998d` | feat(proto,noq): add `read_into(&mut [u8])` for zero-copy reads into caller buffers | `-x` of `ceef5cfd3` (row 1) |
| `6cfa2568a` | feat(noq): add poll-based connection driver for FFI/non-Tokio consumers | `-x` of `ff6e737d8` (row 2) |
| `b2711a973` | chore: run cargo fmt after 1.0.1 patch port | `-x` of `84a8df77a` (conflict, see History) |
| `c15dc5544` | fix: address rust 1.96 clippy lints in read_into and poll_driver patches | `-x` of `c84091f4c` |
| `30c50860c` | fix(noq): split GSO batches and surface peer-opened streams in poll_driver | new (row 2) |
| `67c136445` | test(noq): cover read_into edge cases and poll_driver loopback | new |
| `5d67f7195` | fix(noq): read_to_end after end of stream returns empty, not ClosedStream | new (row `eos`, regression in upstream #794) |
| `867a3a9a3` | chore: release 1.3.1 with the read_to_end end-of-stream fix | new |
| `f4ae698fc` | test(proto): the test driver can hold several connections per endpoint | new (row 31) |
| `2c65fd452` | fix(proto): close a connection on a stateless reset from any of its paths | new (row 31; stand-in for upstream #437) |
| `e75f3a555` | chore: release 1.3.2 with the incoming stateless reset fix | new |

## Patches

Row numbers are stable patch ids shared across Aster's iroh-family forks (iroh, iroh-blobs, iroh-docs, iroh-gossip, iroh-util, noq), so a commit message that says "row 31" resolves here. Patches without a number have a short name instead.

| Id | Topic branch | What | Why / what in Aster depends on it | Commits (topic / upgrade) | Status | Conflict or porting notes |
|---|---|---|---|---|---|---|
| 1 | none recorded (original commit `b2982a787`) | Adds `read_into(&mut [u8])` and `poll_read_into` so receive data is copied straight into caller-owned buffers without intermediate `Bytes` handles. Files: `noq-proto/src/connection/assembler.rs`, `noq-proto/src/connection/streams/recv.rs`, `noq/src/recv_stream.rs`. | `aster_transport_core::CoreRecvStream::read_exact_into` uses `noq::RecvStream::read_into` for lower-allocation frame reads; the FFI and the Python/TypeScript native call paths use `read_exact_into`. | `b2982a787` (original) / `cd67e998d` on `upgrade/noq-v1.3.0` (`-x` of `ceef5cfd3`), plus lint fixes `c15dc5544` and tests `67c136445` | carried | High conflict risk in `noq-proto` stream/assembler internals. Preserve ordered-only semantics, cancel safety, EOF as `Ok(None)` at the public async layer, and the tests/proptests that prove byte correctness. Port before row 2. Tests: `read_into_after_cancelled_read_to_end_is_closed` (a dropped `read_to_end` leaves the stream unordered; `read_into` must return `ClosedStream`, not panic) and `read_into_roundtrip_and_eof` (200 KB uni stream byte-exact, EOF as `Ok(None)` repeatedly), in `noq/src/tests.rs`. |
| 2 | none recorded (original commit `00cd03857`) | Adds `noq::poll_driver`, a synchronous poll-based QUIC driver over `noq-proto` for FFI or foreign runtimes that do not want a Tokio-owned endpoint. Since 1.3.0 it also has `PollEvent::StreamOpened` + `PollDriver::accept_stream`, and sends each GSO segment as its own datagram (`30c50860c`). Files: `noq/src/lib.rs`, `noq/src/poll_driver.rs`. | Intended for the Go/Java/.NET/C ABI bridges. Re-exported as `iroh::poll_driver` (iroh fork row 4) so FFI code depends on one public crate. No Aster Rust code consumes `poll_driver` today; only plans in the Aster repository's `ffi_spec/` reference it. | `00cd03857` (original) / `6cfa2568a` (`-x` of `ff6e737d8`) + fix `30c50860c` on `upgrade/noq-v1.3.0`, plus lint fixes `c15dc5544` and tests `67c136445` | carried | Very high conflict risk on every noq release: it touches the sans-IO endpoint/connection public APIs. Port after row 1; re-check `ConnectionHandle`, `StreamId`, endpoint events, path events and transmit APIs against the new `noq-proto`. `poll_driver_loopback_bi_stream_echo` must pass (two `PollDriver`s on non-blocking std sockets driven synchronously: handshake, open bi, 256 KiB under flow control, server accepts via `StreamOpened`/`accept_stream`, reads to EOF via `stream_recv`, echoes, client reads the echo to EOF; 10/10 consecutive passes when added). `30c50860c` is an additive public-API change on an Aster-only module. |
| `fmt` | none (bookkeeping) | `cargo fmt` over the patch code. | Upstream tags are not rustfmt-clean under the current toolchain once the patches are applied (`noq-v1.3.0` itself is not rustfmt-clean under rustfmt 1.96 at the line the fixup rewraps). | `84a8df77a` (1.0.1 wave) / `b2711a973` on `upgrade/noq-v1.3.0` | carried | Conflicted on the 1.3.0 port in `noq-proto/src/connection/state.rs` (see History). Re-run rather than cherry-pick if it conflicts. |
| `lints` | none | Fixes rust 1.96 clippy lints that fire on the row 1 and row 2 code: `manual_let_else`, `needless_range_loop`, `useless_nonzero_new_unchecked`, `collapsible_if`, and `#[allow(clippy::large_enum_variant)]` on `PollEvent`. | Keeps `cargo clippy --workspace --all-targets -- -D warnings` clean. | `c84091f4c` (1.0.1 wave) / `c15dc5544` on `upgrade/noq-v1.3.0` | carried | Cherry-pick rows 1 and 2 from the latest upgrade branch, which already include these fixes. |
| `eos` | none recorded | `read_to_end` on a stream whose end has already been read returns an empty buffer instead of `Read(ClosedStream)`; a stored reset was masked the same way and is fixed too. Released as noq 1.3.1 (`867a3a9a3`), because 1.3.0 was already published. | Regression in upstream #794: `read_to_end` queried proto's ordered mode before the wrapper's `all_data_read` flag, and proto frees a stream once its FIN is read, so draining an already-finished stream with `read_to_end(0)` errored. iroh-docs' close sequence does exactly that: its `tests/sync.rs` (`sync_simple`, `test_sync_via_relay`, `sync_redrives_known_missing_content_after_resync`) failed every sync's accept-side close. | none / `5d67f7195` + release `867a3a9a3` | carried | Upstream `main` (`5af2505d9`) had no fix when this was written. Drop when an upstream release returns empty from `read_to_end` after end of stream. Test: `read_to_end_after_end_of_stream_is_empty`. Validation: noq lib 37 (3 ignored), clippy `-D warnings`. |
| 31 | `fix/incoming-reset-tokens` (based on `867a3a9a3`, not on the upstream tag) | A stateless reset was accepted only when it matched path 0's transport-parameter token. A path opened with a fresh CID never had its token installed at the endpoint; the endpoint routed every reset to `PathId::ZERO`, where the connection dropped it as coming from an unexpected remote or a discarded path; and the connection compared only the TP token, not in constant time. Now `create_path` and a first PATH_NEW_CONNECTION_ID for an open path install that path's token, registered for that path's remote; the reset table records the token's path and routes there; and the connection compares the datagram in constant time with the active token of every open, non-abandoned path to that remote (`is_stateless_reset`). Datagrams under 21 bytes are never resets (RFC 9000 §10.3). A match on any path closes the connection; an abandoned path's retired token never matches (draft-ietf-quic-multipath-21 §3.4, §3.4.2). Files: `noq-proto/src/connection/mod.rs`, `noq-proto/src/endpoint.rs`, `noq-proto/src/connection/packet_crypto.rs`, `noq-proto/src/cid_queue.rs`, `noq-proto/src/lib.rs`, `noq-proto/src/tests/{stateless_reset,util,mod}.rs`. | Root cause 2 of the Aster restart/reconnect investigation (Aster repository, `docs/_internal/investigations/2026-10-15-restart-reconnect.md`): once iroh selects a direct path (path ≥ 1), a restarted peer's reset was ignored and the dead connection stayed Active until the 30 s idle timeout. Needed together with iroh fork row 32 (deterministic reset key). | `f4ae698fc` + `2c65fd452` / same commits on `upgrade/noq-v1.3.0` (fast-forward); released as 1.3.2 at `e75f3a555` | carried | **Stand-in for upstream n0-computer/noq#437. Drop when a released noq containing the fix is pinned.** Medium conflict risk in the endpoint reset table and `create_path`/NEW_CONNECTION_ID handling. #296 (closed as misguided) and #275 (CID/token cleanup on discard) are not prerequisites. The test-driver commit gives `TestEndpoint::drive_outgoing` per-connection events and timers (needed for two live connections). Tests: 5 in `tests::stateless_reset` (unknown-CID reset closes only the token holder; wrong token, wrong remote and 20-byte datagram ignored, 21-byte closes; reset on path 1 with path 0 open; reset after path 0 discarded; abandoned-path token ignored). Both multipath cases fail without the change and without each of its three parts. noq-proto 435, noq 37 (3 ignored), clippy `--workspace --all-targets -D warnings` and fmt clean. |

Bookkeeping and documentation commits (not needed at runtime):

| Commits | Files | Purpose | Status |
|---|---|---|---|
| `0f797670a`, `f303f21fd`, `485a71caa`, `617899fe7` | `README.md`, `.gitignore`, transient roadmap files | Added, then moved, internal FFI/perf/security roadmap docs under an ignored `docs/aster/`. Net committed effect: fork README context and `docs/aster/` ignored. | Not on `upgrade/noq-v1.3.0`. Optional; if carried forward, keep only committed fork context and the `.gitignore` entry, and keep internal roadmaps untracked. Superseded by the `main` README preamble under the current model. |

### Upstreamed or dropped

| Patch | Topic branch | Commits (topic / upgrade) | Status | When and how |
|---|---|---|---|---|
| Windows UDP busy-loop root cause: `UdpSocketState::new` disables `SIO_UDP_CONNRESET` (`WSAIoctl(…, FALSE)`) so a Windows UDP socket stops failing `recv` with `WSAECONNRESET` (os error 10054) after an ICMP "port unreachable". This caused the Aster/iroh endpoint busy-loop (~120% CPU plus a log flood of `recv error … os error 10054`) when a peer or candidate became unreachable mid-session. | `fix/windows-busy-loop` (based on `noq-v1.0.0`) | `2678dcd69` / `d8daa2b18` (on the 1.0.0 upgrade branch; released as the hand-rolled `-p1`) | upstreamed | Absorbed in `noq-v1.0.1` (2026-07-02) by upstream PR n0-computer/noq#707, filed from Aster's issue #737. Upstream disables both `SIO_UDP_CONNRESET` and `SIO_UDP_NETRESET`, a superset of the Aster patch, so the patch was dropped from the stack in the 1.0.1 wave. |

### Closed security-review branches

These topic branches came from an April 2026 security review. The fixes from that review that held up were merged upstream, so these branches are not carried on the upgrade branch. They are kept for reference.

| Branch |
|---|
| `fix/ack-delay-shift-overflow` |
| `fix/ack-range-dos-and-pn-panic` |

## Porting to a new upstream release

noq is ported first: the iroh fork's `[patch.crates-io]` (iroh row 3) and the `poll_driver` re-export (iroh row 4) depend on it, and the satellite forks (iroh-blobs, iroh-docs, iroh-gossip, iroh-util) pin noq by revision in their own patch blocks. Full order across the iroh family:

1. Port the noq patches (this repository).
2. Port the iroh patches that depend on the noq fork.
3. Port iroh-blobs, iroh-docs and iroh-gossip (and iroh-util), including their dependency patch blocks. Because those blocks reference other forks by revision, re-pin in dependency order: noq → iroh → iroh-blobs → iroh-docs / iroh-gossip.
4. Reapply optional docs/`.gitignore` patches where useful.
5. Update the root `[patch.crates-io]` revs in the Aster repository to the new fork commits.
6. Update `iroh-fork-manifest.toml` in the Aster repository (the only copy), and this ledger's Current release section on `main` (merge upstream `main` first).

Before touching branches:

1. Read the upstream release notes and changelog for the target tag.
2. Inspect the upstream range from the previous base to the target, e.g. `git log --oneline noq-v1.0.0-rc.0..noq-v1.0.0-rc.1` and `git diff --stat` over the same range.
3. For each patch above decide: keep, drop because upstream absorbed it, or rewrite because the APIs it touches changed. Check rows `eos` and 31 against upstream #794 and #437.

Port:

```bash
git fetch --all --tags --prune
git push origin <upstream-tag>              # mirror the upstream tag (e.g. noq-v1.4.0) to the forge
git switch -c upgrade/noq-v<upstream version> <upstream-tag>
# cherry-pick from the current upgrade branch, which carries the lint and test fixes
git cherry-pick -x cd67e998d 6cfa2568a b2711a973 c15dc5544 30c50860c 67c136445
git cherry-pick -x 5d67f7195          # row eos, unless upstream fixed #794
git cherry-pick -x f4ae698fc 2c65fd452  # row 31, unless upstream fixed #437
# version bump, only if the version rule requires it (Aster's registry has already
# served <upstream version> or higher in the series): set noq, noq-udp and noq-proto
# to the next patch above Aster's highest published, then
git commit -am "chore: release <published version>"
git push origin upgrade/noq-v<upstream version>
# do not tag yet: tag after the consumer trial (below)
```

Prefer `cherry-pick -x` for provenance. If a patch no longer applies, use the "What" and "Conflict or porting notes" columns above as the specification and reimplement. Published `upgrade/*` branches and tags are never rewritten; a failed attempt is superseded by new commits and a new version.

An Aster-only fix on the current base is a topic branch based on the upstream release tag, cherry-picked `-x` onto the current upgrade branch, followed by a version-bump commit to the next patch above Aster's highest published version (the release commits for 1.3.1 and 1.3.2 followed rows `eos` and 31). A re-pin without a code change is a commit on the current upgrade branch with no version bump, tagged `aster-noq-v<current version>-p<n>` after the consumer trial.

Validate in this repository:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p noq-proto
cargo test -p noq --all-features
```

Then, in the iroh fork, re-pin noq (iroh row 3), run iroh's suite, and re-pin the satellites. Whenever noq moves, re-pin the iroh-docs fork's own `[patch.crates-io]` to the same revisions and run its full suite (`cargo test --all-features`, 118 tests including `tests/sync.rs`): iroh-docs exercises the whole stack together, and it is what caught row `eos`.

Gotcha when re-pinning a satellite: `cargo update -p iroh` alone can keep crates.io iroh and report the patches as unused. `cargo update -p iroh --precise <version>` (or temporarily requiring the exact version and running `cargo update -p <crate>`) moves the transitive dependencies.

Aster validation after updating pins, from the Aster repository root:

```bash
cargo fmt --check
cargo clippy --manifest-path bindings/python/rust/Cargo.toml -- -D warnings
cargo test -p aster_transport_core
cargo test -p aster --all-features
uv run maturin develop -m bindings/python/rust/Cargo.toml
uv run pytest tests/python/ -v --timeout=30
```

At minimum verify the frame-read paths that use `CoreRecvStream::read_exact_into` and the FFI/PyO3 fast paths: they are the main consumers of row 1.

Tag only after the Aster consumer trial passes against these exact commits (tags cannot be moved), then record the release:

```bash
git tag -a aster-noq-v<published version> -m "Aster noq v<published version>" <release commit>
git push origin aster-noq-v<published version>
# re-pin only: git tag -a aster-noq-v<current version>-p<n> -m "..." <re-pin commit>
```

Update `iroh-fork-manifest.toml` in the Aster repository (the only copy), and this ledger's Current release section on `main` (merge upstream `main` first).

Local checkouts: downstream workspaces that use `[patch.crates-io]` path overrides into a local checkout of this fork (for example portal-sync) build whatever is checked out, not the pinned rev. After publishing, switch the local checkout to the released tag (`git switch --detach aster-noq-v<version>`), check `git status --short --branch` and `git tag --points-at HEAD`, and run at least one path-override consumer check (for example `cargo check --locked -p portal-cas` in portal-sync).

Before the current branch model, fork `main` mirrored upstream `main` and was reset to it on each port (`git reset --hard upstream/main`, `git push origin main --force-with-lease`, the only place a forced push was allowed). Under the current model `main` is upstream `main` plus one commit (README preamble and this ledger), refreshed with `git merge upstream/main`, and is never reset or force-pushed.

## History

### 1.3.0 wave (2026-10-03 onward), current

- Upstream released `noq-v1.3.0`. The fork was re-ported onto it as `upgrade/noq-v1.3.0` (base `c1f411562`), validated, tagged `aster-noq-v1.3.0` and pushed to Forgejo. Fork `main` was fast-forwarded to upstream `main` (`5af2505d9`, no force) and the upstream `noq-v1.3.0` tag was mirrored to Forgejo.
- **Conflict:** the fmt fixup (`84a8df77a`) conflicted in `noq-proto/src/connection/state.rs` because upstream added `use crate::shared::EndpointEventInner;` beside the import the fixup rewraps. Resolved in the cherry-pick itself (no fixup commit): keep the new upstream import and the rustfmt-wrapped `use crate::{ApplicationClose, …};` block. The other three noq patches applied clean.
- **poll_driver had two real defects**, found by its first end-to-end test (`30c50860c`): `flush_transmits` asked `poll_transmit` for up to 64 datagrams but sent the GSO batch with one plain `send_to`, so the peer got one oversized datagram and dropped the trailing packets (a 256 KiB transfer stalled at ~2 KB); and `StreamEvent::Opened` was discarded with no way to accept a remote stream, so a server could never discover a peer-opened stream (`noq-proto` announces its first data only through `Opened`). Fixed by sending each GSO segment as its own datagram and adding `PollEvent::StreamOpened` + `PollDriver::accept_stream`.
- New tests `67c136445` (row 1 and row 2 entries above).
- Validation (macOS, rust 1.96; patchbay tests are Linux-only and were not run): `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test -p noq-proto` 430 + 3 passed; `cargo test -p noq` 36 passed / 3 ignored (lib) plus doc and integration targets green.
- **Same day: noq 1.3.1** (`867a3a9a3`, tag `aster-noq-v1.3.1`). Re-pinning the satellites exposed the upstream #794 regression (row `eos`). iroh `1974e958` (tag `aster-iroh-v1.3.0-p1`) only re-pins noq, so iroh stayed 1.3.0; its `noq = "1.3.0"` requirement admits 1.3.1. The satellites' own patch blocks were moved to noq `867a3a9a` (lock and pin changes only). iroh-blobs also has an intermediate `aster-iroh-blobs-v0.103.5-p1` (`712ce693`) on noq 1.3.0, superseded the same day; do not pin it. Validation: noq lib 37 (3 ignored) including `read_to_end_after_end_of_stream_is_empty`, clippy `-D warnings`; iroh clippy and lib tests 124 single-threaded; iroh-blobs 133, iroh-docs 118 (sync 27/27 twice) and iroh-gossip 25, each clippy clean.
- **noq 1.3.2** (`e75f3a555`, tag `aster-noq-v1.3.2`): row 31, the stateless-reset fix from the 2026-10-15 restart/reconnect investigation, released together with iroh 1.3.2 (deterministic reset key). iroh-docs 0.101.5-p2 (2026-10-16) and iroh-gossip 0.101.1-p1 / iroh-util 0.6.3-p1 (2026-10-18) re-pinned their own patch blocks to noq `e75f3a55`.

### 1.0.1 wave (2026-07-02)

| Upstream base | Tag | Pinned commit | Branch |
|---|---|---|---|
| `noq-v1.0.1` | `aster-noq-v1.0.1` | `c84091f4c1d62f3c9025e40014fc2db59f239ffc` | `upgrade/noq-v1.0.1` |

- `noq-v1.0.1` absorbed the Windows UDP busy-loop fix (upstream PR n0-computer/noq#707, from Aster issue #737), so the Aster patch was dropped (see Upstreamed or dropped).
- Carried: `read_into` and `poll_driver` (cherry-picked clean), the fmt fixup, and new commit `c84091f4c` fixing rust 1.96 clippy lints on the patch code. Future ports should cherry-pick the versions of these patches from the latest upgrade branch, which include the lint fixes.
- The iroh fork's patch block pinned noq at `c84091f4c`.
- On 2026-08-13 the iroh-family forks' own `[patch.crates-io]` blocks, which had named the GitHub mirror at the 1.0.0-wave noq (`ddb8829d`), were swept to Forgejo at this wave's revisions, in dependency order noq → iroh → iroh-blobs → iroh-docs / iroh-gossip.
- Validation: fmt, clippy (`-D warnings`, `--all-features --tests`), noq-proto 385 and noq 35 tests; then downstream `aster_transport_core` (242) and `aster --all-features` (94) Rust tests, PyO3 clippy clean, `maturin develop` and 1117 Python tests.
- Local checkout for path-override consumers at the end of the cycle: `aster-noq-v1.0.1`.

### Windows UDP busy-loop fix (2026-07-01)

- Fixed the endpoint busy-loop (~120% CPU plus log flood, `recv error … os error 10054`) that developed on Windows when a peer or candidate became unreachable mid-session. A root-cause write-up and a deterministic repro were kept by Aster outside the fork.
- The fix was an isolated, code-only commit on `fix/windows-busy-loop`, based on the upstream release tag `noq-v1.0.0` so it would cherry-pick onto any later base: `2678dcd69`, cherry-picked `-x` onto the live upgrade branch as `d8daa2b18`. The iroh fork carried a companion recv-error log throttle on its own `fix/windows-busy-loop` (iroh row 5).
- Validation: on a Windows 11 host (rustc 1.96.0) the deterministic repro returned 10054 on every recv by default and `WouldBlock` once `SIO_UDP_CONNRESET` was disabled; `cargo check -p noq-udp` compiled natively. On macOS: `cargo check -p noq-udp --target x86_64-pc-windows-msvc`, clippy and fmt clean.
- Resolution (2026-07-02): absorbed upstream in `noq-v1.0.1` and dropped.

### 1.0 wave (2026-06-21)

| Upstream base | Tag | Pinned commit | Branch |
|---|---|---|---|
| `noq-v1.0.0` | `aster-noq-v1.0.0` | `ddb8829dbc1cd401f8643ba9b93c542ffee43577` | `upgrade/noq-v1.0.0` |

- Upstream shipped a coordinated 1.0 wave; every fork was re-ported. The patch stack was carried forward unchanged from rc.1 and cherry-picked clean.
- This cycle every tag pointed at exactly the commit Aster pinned (pin == tag == validated code; no docs-only `-pN` retags).
- Validation: per-fork fmt/clippy/tests, then `aster_transport_core` (229) and `aster` (68) Rust tests and `maturin develop` + 1117 Python tests.
- Local checkout at the end of the cycle: `aster-noq-v1.0.0`.

### rc.1 port (2026-06-07)

- Before the port Aster pinned `617899fe7e` (tag `aster-noq-v1.0.0-rc.0`), upstream base `noq-v1.0.0-rc.0` (`6ee7cf2f8`), branch `upgrade/noq-v1.0.0-rc.0`; stack range `noq-v1.0.0-rc.0..aster-noq-v1.0.0-rc.0`. Target `noq-v1.0.0-rc.1` (`c80da2500`).
- Ported and published: fork `main` mirrored upstream `main` (`33dba68d3`); branch `upgrade/noq-v1.0.0-rc.1` and tag `aster-noq-v1.0.0-rc.1` were pushed to `aster-rpc/noq`, based on `noq-v1.0.0-rc.1` (`c80da2500`).

| Commit | Purpose |
|---:|---|
| `7e0f3a7d8` | Cherry-pick of `b2982a787`: `read_into(&mut [u8])` / `poll_read_into`. |
| `6c1137d29` | Cherry-pick of `00cd03857`: poll-based `noq::poll_driver`. |
| `e968659c4` | `cargo fmt` after the rc1 port; the upstream rc1 checkout was not rustfmt-clean under the local toolchain once patched. |

- The iroh fork's patch block was pinned to `e968659c47057b18210a4a03dc2366c65fe8f654` instead of `branch = "main"`, because fork `main` then mirrored upstream.
- Validation: `cargo fmt --check`, `cargo test -p noq-proto --all-features`, `cargo test -p noq --all-features`.
- The rc.1 port command sequence was `git cherry-pick -x b2982a787 00cd03857` (optionally the docs commits `485a71caa 617899fe7`) onto `upgrade/noq-v1.0.0-rc.1`, then tag `aster-noq-v1.0.0-rc.1`. The Aster root patch block then pointed `noq`, `noq-udp` and `noq-proto` at `https://github.com/aster-rpc/noq`; it now points at the Forgejo repository.
