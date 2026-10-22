# Aster patch ledger: noq

This repository is Aster's fork of [n0-computer/noq](https://github.com/n0-computer/noq), the QUIC implementation used by iroh. Aster carries the patches listed below on top of upstream releases. Upstream maintainers are welcome to incorporate any of them, in whole or in part, at their discretion.

## Branch and tag model

- `main` is upstream `main` plus one commit: the Aster README preamble and this ledger. Nothing is built from `main`. It is refreshed with `git merge upstream/main` and is never reset or force-pushed. Ledger edits are commits on `main`, never on `upgrade/*`.
- `upgrade/<repo>-v<upstream version>` (for example `upgrade/iroh-v1.4.0`, `upgrade/iroh-blobs-v0.103.1`) is the patched integration line: the upstream release plus the patches in this ledger. A new upstream base always gets a new branch, cut from the upstream release tag; Aster-only fixes and re-pins append to the current branch. Per-minor names (`upgrade/iroh-blobs-v0.103`, `upgrade/salvo-v1.0`, …) are historical. Consumers pin commits on it. Published `upgrade/*` branches and tags are never rewritten; a failed attempt is superseded by new commits and a new version.
- Topic branches carry one change each, are named for the change (for example `feat/read-into`, `security/plumtree-bounds`), are based on the upstream release tag (or, for a patch that builds on another patch, on that prerequisite's commit, recorded in its row), and are cherry-picked with `git cherry-pick -x` onto the upgrade branch, so every upgrade-branch commit names its source.
- Version: the upstream version if Aster's registry has never served that version or a higher one in the series; otherwise the next patch above Aster's highest published version. A re-pin without a code change keeps the version and takes the tag suffix `-p<n>`.
- Tags: `aster-<repo>-v<published version>` (re-pins `aster-<repo>-v<published version>-p<n>`), created only after the Aster consumer trial passes against those exact commits, because tags cannot be moved.
- The fork manifest is `iroh-fork-manifest.toml` in the Aster repository (the only copy).
- Aster does not submit patches upstream; upstream maintainers are welcome to take any patch here.

Historical and current tag schemes in this repository:

- `aster-noq-v<version>`, for example `aster-noq-v1.0.0-rc.0`, `aster-noq-v1.0.1`, `aster-noq-v1.3.2`, `aster-noq-v1.4.0`, `aster-noq-v1.4.1`, `aster-noq-v1.4.2`.
- `aster-noq-v<version>-p<n>`, a later commit at the same crate version: `aster-noq-v1.0.0-p1` (the hand-rolled Windows UDP fix, since upstreamed; historical: under the current model a code change takes a new version, and `-p<n>` is for re-pins only).

## Current release

| Field | Value |
|---|---|
| Upstream base | `noq-v1.4.0` (`34f75c49f`) |
| Integration branch | `upgrade/noq-v1.4.0` |
| Tag | `aster-noq-v1.4.2` |
| Pinned commit | `a3c70b7afe459cbf01122928f4f90d72063ab972` |
| Crate version | 1.4.2 (published with the next Aster release; the registry serves up to 1.4.0, and 1.4.1 was tagged but never published) |
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
noq       = { git = "https://forge.emrul.dev/Aster/noq.git", rev = "a3c70b7afe459cbf01122928f4f90d72063ab972" }
noq-udp   = { git = "https://forge.emrul.dev/Aster/noq.git", rev = "a3c70b7afe459cbf01122928f4f90d72063ab972" }
noq-proto = { git = "https://forge.emrul.dev/Aster/noq.git", rev = "a3c70b7afe459cbf01122928f4f90d72063ab972" }
```

The Aster iroh fork pins this commit in its own `[patch.crates-io]` (see the iroh fork's ledger, row 3), and the iroh-blobs, iroh-docs, iroh-gossip and iroh-util forks pin it in theirs.

### `upgrade/noq-v1.4.0` stack

Upstream `noq-v1.4.0` already carried version 1.4.0, which Aster's registry had never served, so the 1.4.0 release had no release commit. 1.4.1 and 1.4.2 each end in a release commit.

| Commit | Subject | Source |
|---:|---|---|
| `f614cd043` | feat(proto,noq): add `read_into(&mut [u8])` for zero-copy reads into caller buffers | `-x` of `cd67e998d` (row 1; rewritten onto upstream's `VecDeque` assembler) |
| `d378ece18` | feat(noq): add poll-based connection driver for FFI/non-Tokio consumers | `-x` of `6cfa2568a` (row 2) |
| `c80473141` | chore: run cargo fmt after 1.0.1 patch port | `-x` of `b2711a973` (row `fmt`; hunks in upstream-only files dropped) |
| `72c1d777a` | fix: address rust 1.96 clippy lints in read_into and poll_driver patches | `-x` of `c15dc5544` (row `lints`) |
| `5c77621f1` | fix(noq): split GSO batches and surface peer-opened streams in poll_driver | `-x` of `30c50860c` (row 2) |
| `5da9ef194` | test(noq): cover read_into edge cases and poll_driver loopback | `-x` of `67c136445` |
| `7dcc5579c` | fix(noq): read_to_end after end of stream returns empty, not ClosedStream | `-x` of `5d67f7195` (row `eos`) |
| `4fe94b847` | test(proto): let the test driver hold several connections per endpoint | `-x` of `f4ae698fc` (row 31) |
| `4be718e40` | fix(proto): close a connection on a stateless reset from any of its paths | `-x` of `2c65fd452` (row 31; merged with upstream's `retire_cids()`) |
| `dd9c07514` | chore: run cargo fmt over the patch code after the 1.4.0 port | new (row `fmt`; rewraps a row 1 test in `assembler.rs`) |
| `b392ac5a4` | fix(proto): row 31 on noq 1.4.0: alloc/core imports for no_std noq-proto, drop unused import | new (row 31) |
| `535bd8840` | fix(proto): read_into with an empty buffer returns Ok(0) and leaves the stream unchanged | `-x` of `e138284e8` (row 1) |
| `786f7ccda` | experiment(bbr3): reset probe feedback state per draft 06 | `-x` of `a63916257` (row `bbr3-probe-feedback`) |
| `ae28c2bae` | fix(bbr3): grow the probe inflight cap in packet-sized increments | `-x` of `910abb85c` (row `bbr3-probe-growth`) |
| `f5e70a515` | fix(proto): coalesce receive spans before enforcing the chunk cap | `-x` of `69a1df075` (row `stream-buffer-spans`) |
| `bde811195` | fix(proto): preserve lower offsets when compacting unordered reads | `-x` of `3ae189bba` (row `stream-buffer-spans`) |
| `21d11b4b9` | fix(proto): never return a zero pacing delay at a controller-supplied rate | `-x` of `fcfe0994b` (row `pacer-zero-delay`) |
| `cf212124f` | feat(proto): default TransportConfig to BBRv3 | `-x` of `31cdf75ac` (row `bbr3-default`) |
| `13dcb5b7f` | chore: release 1.4.1 with the BBRv3 fixes, stream-buffer compaction and BBRv3 default | release commit (tag `aster-noq-v1.4.1`) |
| `b591a4994` | perf(proto): maintain BBRv3 packet history incrementally | `-x` of `57c19a744` (row `bbr3-incremental-history`) |
| `a3c70b7af` | chore: release 1.4.2 with incremental BBRv3 packet history | release commit (tag `aster-noq-v1.4.2`) |

## Patches

Row numbers are stable patch ids shared across Aster's iroh-family forks (iroh, iroh-blobs, iroh-docs, iroh-gossip, iroh-util, noq), so a commit message that says "row 31" resolves here. Patches without a number have a short name instead.

| Id | Topic branch | What | Why / what in Aster depends on it | Commits (topic / upgrade) | Status | Conflict or porting notes |
|---|---|---|---|---|---|---|
| 1 | none recorded (original commit `b2982a787`) | Adds `read_into(&mut [u8])` and `poll_read_into` so receive data is copied straight into caller-owned buffers without intermediate `Bytes` handles. Files: `noq-proto/src/connection/assembler.rs`, `noq-proto/src/connection/streams/recv.rs`, `noq/src/recv_stream.rs`. | `aster_transport_core::CoreRecvStream::read_exact_into` uses `noq::RecvStream::read_into` for lower-allocation frame reads; the FFI and the Python/TypeScript native call paths use `read_exact_into`. | `b2982a787` (original) / `f614cd043` on `upgrade/noq-v1.4.0` (`-x` of `cd67e998d`), plus lint fixes `72c1d777a`, tests `5da9ef194` and the empty-buffer fix `e138284e8` on `fix/read-into-empty-buffer` (based on `b392ac5a4`, the upgrade branch it fixes) / `535bd8840` | carried | High conflict risk in `noq-proto` stream/assembler internals. Preserve ordered-only semantics, cancel safety, EOF as `Ok(None)` at the public async layer, and the tests/proptests that prove byte correctness. Port before row 2. On 1.4.0 upstream replaced the assembler's `BinaryHeap` with a sorted `VecDeque` (#823), so `Assembler::read_into` was rewritten onto it; the rewrite was reviewed independently and found sound. `Chunks::read_into` with an empty buffer returns `Ok(0)` before the stream-state checks, as `next(0)` does (before the fix it reported `Blocked` while data was buffered, or finished the stream before the caller had seen its end). Tests: `read_into_after_cancelled_read_to_end_is_closed` (a dropped `read_to_end` leaves the stream unordered; `read_into` must return `ClosedStream`, not panic) and `read_into_roundtrip_and_eof` (200 KB uni stream byte-exact, EOF as `Ok(None)` repeatedly), in `noq/src/tests.rs`, and `read_into_empty_buffer_is_a_no_op` in `noq-proto/src/tests/mod.rs`. |
| 2 | none recorded (original commit `00cd03857`) | Adds `noq::poll_driver`, a synchronous poll-based QUIC driver over `noq-proto` for FFI or foreign runtimes that do not want a Tokio-owned endpoint. Since 1.3.0 it also has `PollEvent::StreamOpened` + `PollDriver::accept_stream`, and sends each GSO segment as its own datagram (`30c50860c`). Files: `noq/src/lib.rs`, `noq/src/poll_driver.rs`. | Intended for the Go/Java/.NET/C ABI bridges. Re-exported as `iroh::poll_driver` (iroh fork row 4) so FFI code depends on one public crate. No Aster Rust code consumes `poll_driver` today; only plans in the Aster repository's `ffi_spec/` reference it. | `00cd03857` (original) / `d378ece18` (`-x` of `6cfa2568a`) + fix `5c77621f1` (`-x` of `30c50860c`) on `upgrade/noq-v1.4.0`, plus lint fixes `72c1d777a` and tests `5da9ef194` | carried | Very high conflict risk on every noq release: it touches the sans-IO endpoint/connection public APIs. Port after row 1; re-check `ConnectionHandle`, `StreamId`, endpoint events, path events and transmit APIs against the new `noq-proto`. `poll_driver_loopback_bi_stream_echo` must pass (two `PollDriver`s on non-blocking std sockets driven synchronously: handshake, open bi, 256 KiB under flow control, server accepts via `StreamOpened`/`accept_stream`, reads to EOF via `stream_recv`, echoes, client reads the echo to EOF; 10/10 consecutive passes when added). `30c50860c` is an additive public-API change on an Aster-only module. |
| `fmt` | none (bookkeeping) | `cargo fmt` over the patch code. | Upstream tags are not rustfmt-clean under the current toolchain once the patches are applied (`noq-v1.3.0` itself is not rustfmt-clean under rustfmt 1.96 at the line the fixup rewraps). | `84a8df77a` (1.0.1 wave) / `c80473141` (`-x` of `b2711a973`) + `dd9c07514` on `upgrade/noq-v1.4.0` | carried | Conflicted on the 1.3.0 port in `noq-proto/src/connection/state.rs` (see History). On 1.4.0 its hunks in the upstream-only files `noq-proto/src/connection/state.rs` and `noq-proto/src/packet.rs` were dropped: the fmt commits touch only files the patches change, never upstream-only files. Upstream `noq-v1.4.0` is not rustfmt-clean under our toolchain, so `cargo fmt --all --check` cannot pass; check only the files the patches touch (see Validate). Re-run rather than cherry-pick if it conflicts. |
| `lints` | none | Fixes rust 1.96 clippy lints that fire on the row 1 and row 2 code: `manual_let_else`, `needless_range_loop`, `useless_nonzero_new_unchecked`, `collapsible_if`, and `#[allow(clippy::large_enum_variant)]` on `PollEvent`. | Keeps `cargo clippy --workspace --all-targets -- -D warnings` clean. | `c84091f4c` (1.0.1 wave) / `72c1d777a` (`-x` of `c15dc5544`) on `upgrade/noq-v1.4.0` | carried | Cherry-pick rows 1 and 2 from the latest upgrade branch, which already include these fixes. |
| `eos` | none recorded | `read_to_end` on a stream whose end has already been read returns an empty buffer instead of `Read(ClosedStream)`; a stored reset was masked the same way and is fixed too. Released as noq 1.3.1 (`867a3a9a3`), because 1.3.0 was already published. | Regression in upstream #794: `read_to_end` queried proto's ordered mode before the wrapper's `all_data_read` flag, and proto frees a stream once its FIN is read, so draining an already-finished stream with `read_to_end(0)` errored. iroh-docs' close sequence does exactly that: its `tests/sync.rs` (`sync_simple`, `test_sync_via_relay`, `sync_redrives_known_missing_content_after_resync`) failed every sync's accept-side close. | none / `5d67f7195` + release `867a3a9a3` on `upgrade/noq-v1.3.0`; `7dcc5579c` (`-x` of `5d67f7195`) on `upgrade/noq-v1.4.0` | carried | Upstream `main` (`5af2505d9`) had no fix when this was written, nor did `noq-v1.4.0`. Drop when an upstream release returns empty from `read_to_end` after end of stream. Test: `read_to_end_after_end_of_stream_is_empty`. Validation: noq lib 37 (3 ignored), clippy `-D warnings`. |
| 31 | `fix/incoming-reset-tokens` (based on `867a3a9a3`, not on the upstream tag) | A stateless reset was accepted only when it matched path 0's transport-parameter token. A path opened with a fresh CID never had its token installed at the endpoint; the endpoint routed every reset to `PathId::ZERO`, where the connection dropped it as coming from an unexpected remote or a discarded path; and the connection compared only the TP token, not in constant time. Now `create_path` and a first PATH_NEW_CONNECTION_ID for an open path install that path's token, registered for that path's remote; the reset table records the token's path and routes there; and the connection compares the datagram in constant time with the active token of every open, non-abandoned path to that remote (`is_stateless_reset`). Datagrams under 21 bytes are never resets (RFC 9000 §10.3). A match on any path closes the connection; an abandoned path's retired token never matches (draft-ietf-quic-multipath-21 §3.4, §3.4.2). Files: `noq-proto/src/connection/mod.rs`, `noq-proto/src/endpoint.rs`, `noq-proto/src/connection/packet_crypto.rs`, `noq-proto/src/cid_queue.rs`, `noq-proto/src/lib.rs`, `noq-proto/src/tests/{stateless_reset,util,mod}.rs`. | Root cause 2 of the Aster restart/reconnect investigation (Aster repository, `docs/_internal/investigations/2026-10-15-restart-reconnect.md`): once iroh selects a direct path (path ≥ 1), a restarted peer's reset was ignored and the dead connection stayed Active until the 30 s idle timeout. Needed together with iroh fork row 32 (deterministic reset key). | `f4ae698fc` + `2c65fd452` / same commits on `upgrade/noq-v1.3.0` (fast-forward), released as 1.3.2 at `e75f3a555`; `4fe94b847` + `4be718e40` (`-x`) + no_std import fix `b392ac5a4` on `upgrade/noq-v1.4.0` | carried | **Stand-in for upstream n0-computer/noq#437. Drop when a released noq containing the fix is pinned.** Medium conflict risk in the endpoint reset table and `create_path`/NEW_CONNECTION_ID handling. #296 (closed as misguided) and #275 (CID/token cleanup on discard) are not prerequisites. On 1.4.0 the change was merged with upstream's new `retire_cids()` (#823, CID retirement limits); the merge was reviewed independently and found sound, and `b392ac5a4` adds the `alloc`/`core` imports no_std `noq-proto` needs. The test-driver commit gives `TestEndpoint::drive_outgoing` per-connection events and timers (needed for two live connections). Tests: 5 in `tests::stateless_reset` (unknown-CID reset closes only the token holder; wrong token, wrong remote and 20-byte datagram ignored, 21-byte closes; reset on path 1 with path 0 open; reset after path 0 discarded; abandoned-path token ignored). Both multipath cases fail without the change and without each of its three parts. noq-proto 435, noq 37 (3 ignored), clippy `--workspace --all-targets -D warnings` and fmt clean. |
| `bbr3-probe-feedback` | `fix/bbr3-probe-feedback` (based on integration `535bd8840`, not on the upstream tag: the topics share the `read_into` stack because `stream-buffer-spans`' regression tests use row 1's `read_into`, and the BBR topics were cut from the same base) | End BBRv3 probe feedback once and clear stale sample/ACK-phase state: at the end of the feedback round clear `bw_probe_samples` and reset `ack_phase` to `Init`, including for application-limited samples and outside ProbeBW; the bandwidth filter advances conditionally. File: `noq-proto/src/congestion/bbr3/mod.rs`. | Related to upstream n0-computer/noq#819. Without it, stale probe feedback distorts later rounds. Prerequisite of `bbr3-probe-growth` and `bbr3-default`. Evidence: the cc-compare qualification on `investigate/bbr3-recovery` (report's original commit `8113467`). | `a63916257` (`-x` of `8113467`) / `786f7ccda` on `upgrade/noq-v1.4.0` | carried (released in 1.4.1, `aster-noq-v1.4.1`) | Carry with `bbr3-probe-growth`: the reset alone fails the existing tenfold bandwidth-recovery test (`probe_up_rediscovers_full_bw_after_10x_increase`). Three transition regressions: `probe_feedback_ends_and_advances_bandwidth_filter_only_once`, `probe_feedback_resets_outside_probe_bw_but_waits_for_round_start`, `probe_feedback_resets_even_when_sample_is_app_limited`. The source commit's subject is `experiment(bbr3): …`, kept for provenance. Drop when an upstream release fixes #819 equivalently. |
| `bbr3-probe-growth` | `fix/bbr3-probe-growth` (based on `a63916257`, prerequisite previous row) | Preserve packet-sized inflight growth while accounting ACK budgets in bytes: the ProbeUp inflight cap grows in SMSS steps, keeping the ACK remainder and the SMSS floor. File: `noq-proto/src/congestion/bbr3/mod.rs`. | Completes `bbr3-probe-feedback`; together they restore the tenfold bandwidth recovery. Report's original commit `1e0c42c`. | `910abb85c` (`-x` of `1e0c42c`) / `ae28c2bae` on `upgrade/noq-v1.4.0` | carried (released in 1.4.1, `aster-noq-v1.4.1`) | Test packet steps (`probe_up_cap_grows_in_packet_steps_and_preserves_acked_remainder`) and the unchanged tenfold recovery target and round bound (`probe_up_rediscovers_full_bw_after_10x_increase`). Port together with the previous row. |
| `stream-buffer-spans` | `fix/stream-buffer-compaction` (based on integration `535bd8840`, not on the upstream tag: its regression tests use row 1's `read_into`) | Prevent legitimate large-window transfers from exhausting the raw-buffer count behind one gap: coalesce adjacent receive buffers under chunk pressure while preserving the 1024-distinct-span cap, and preserve unread lower offsets when compacting after unordered high-offset reads. File: `noq-proto/src/connection/assembler.rs`. | With 32 MiB stream / 64 MiB connection receive windows, transfers failed with `too many gaps in stream buffer` under both controllers (cc-compare qualification report, "Stream-buffer failure"). Independent of BBR. | `69a1df075` + `3ae189bba` / `f5e70a515` + `bde811195` on `upgrade/noq-v1.4.0` | carried (released in 1.4.1, `aster-noq-v1.4.1`) | Patch to upstream's `VecDeque` assembler (#823); carry both commits. Preserve ordered/unordered byte correctness, allocation reuse and the distinct-gap cap. Retain the gap, duplicate and overlap-flood tests (upstream's `assemble_*_compact` and `compact`, plus `contiguous_packets_behind_one_gap_survive_chunk_pressure`, `compacted_large_span_does_not_hide_excess_gaps`, `chunk_pressure_preserves_multiple_runs_and_retransmits` and `unordered_chunk_pressure_preserves_offsets_below_bytes_read`). Medium conflict risk wherever upstream touches the assembler. |
| `pacer-zero-delay` | `fix/pacer-zero-delay` (based on `noq-v1.4.0`) | `Pacer::delay_at_rate` never returns a zero delay: the shortfall is converted to whole nanoseconds rounded up with a floor of one, and tokens are earned with integer arithmetic so waiting the returned delay always covers the shortfall. File: `noq-proto/src/connection/pacing.rs`. | Found while adopting `bbr3-default`, not part of the cc-compare evaluation. At a controller-supplied rate (BBRv3) a shortfall worth under a nanosecond (a few bytes at multi-GB/s on a low-RTT path) truncated to a zero delay: the pacing timer fired at the same instant, earned nothing and re-armed. noq-proto's deterministic driver spins forever (`stream_gso`, `datagram_gso` hang with BBRv3); a real clock escapes only after time moves, spinning meanwhile. Upstream code (`64a7e4cb9`, #785), unfixed on upstream `main` when written. Prerequisite of `bbr3-default`. | `fcfe0994b` / `21d11b4b9` on `upgrade/noq-v1.4.0` | carried (released in 1.4.1, `aster-noq-v1.4.1`) | Low conflict risk. Test `sub_nanosecond_deficit_at_high_rate_still_delays` (fails without the fix); `default_controller_completes_stream_transfer` (row `bbr3-default`) covers it end to end. Re-measured after this fix (`results/pacer-fix/report.md`): it does not account for BBRv3's higher sender CPU; keep it as a correctness fix. Drop when an upstream release never returns a zero rate-path delay. |
| `bbr3-default` | `fix/bbr3-default` (based on integration `21d11b4b9`, after `bbr3-probe-feedback`, `bbr3-probe-growth` and `pacer-zero-delay`) | `TransportConfig::default()` builds `congestion::Bbr3Config::default()` instead of CUBIC; CUBIC and NewReno stay selectable through `congestion_controller_factory`. noq-proto's deterministic test driver pins CUBIC via `tests::util::cubic_transport_config()`, because about thirty tests count datagrams, steps and ACK timing that assume an unpaced initial window. Files: `noq-proto/src/config/transport.rs`, `noq-proto/src/tests/{util,mod,proptests}.rs`. | Use the corrected controller across Iroh and Salvo: Iroh's default endpoint builder and Salvo's Noq listener both start from this configuration. Tradeoffs from the cc-compare qualification (measured on `535bd884` plus the two BBR fixes, before `pacer-zero-delay`): better goodput under independent random loss (1.93 → 17.07 Mbps at 1 %, 80 ms) and much lower deep-queue RPC p95 (537 → 83 ms); clean-link goodput 1–4 % lower; higher sender CPU (5.21 s vs 4.00 s median, Hetzner→OVH with GSO). Re-measured on 1.4.1 with `pacer-zero-delay`, the premium remains and is not the zero-delay spin (`results/pacer-fix/report.md`: 5.36 s vs 4.12 s with GSO, 5.71 s vs 5.60 s without), and it depends on workload and CPU governor (`results/batching-investigation/report.md`: 2.16 vs 1.07 CPU s per UDP GiB in performance mode). Pacer-batching prototypes saved at most 8.7 % cycles per GiB and were not adopted; the next candidate is incremental packet-history maintenance instead of per-ACK history walks; regressions under 1 % correlated reordering with 5 ms jitter (11.96 → 9.33 Mbps, p95 156 → 170 ms) and fq_codel/ECN (p95 91 → 148 ms; 104–114 CE marks per trial vs 21). ECN response and the reordering deficit remain open. | `31cdf75ac` / `cf212124f` on `upgrade/noq-v1.4.0`, released with the others as 1.4.1 at `13dcb5b7f` | carried (released in 1.4.1, `aster-noq-v1.4.1`) | Apply after both BBR fixes and `pacer-zero-delay`. Tests: `default_congestion_controller_is_bbr3`, `cubic_remains_selectable`, `default_controller_completes_stream_transfer`. On upstream test changes, new driver tests inherit CUBIC through the harness constructors; a test that builds `TransportConfig::default()` itself and counts packets needs `cubic_transport_config()`. Consumers: verify the selected controller (explicit `congestion_controller_factory` or `transport_config_tuner` overrides in Iroh, Salvo and Aster), and select a common controller explicitly when comparing backends. |
| `bbr3-incremental-history` | `bbr3-incremental-history` (based on integration `13dcb5b7f`, the 1.4.1 release it optimises) | Maintains BBRv3's per-space packet history incrementally: newly acknowledged packet numbers go into a reusable per-space vector; at ACK-batch end the previous batch's stale prefix is removed with `pop_front`, only the new records are marked, and only the expired prefix is aged. A per-space stale count bounds earlier stale records; when prefix removal does not account for it (reordered holes, loss removals), the existing linear `retain` runs, so heavily reordered traffic keeps the old linear compaction bound. No new per-packet fields; no pacer, controller-model, runtime or transport-parameter change. File: `noq-proto/src/congestion/bbr3/mod.rs`. | Cuts the BBRv3 sender CPU premium that `bbr3-default` brought to Iroh and Salvo. Headline numbers (six interleaved pairs, k2 → OVH, unlimited bulk plus a 1 KiB RPC every 50 ms, normalised per sender UDP GiB, 95 % paired bootstrap CIs): cycles −28.4 % (−20.4 to −35.7 %), CPU seconds −27.0 % (2.674 → 1.953), instructions −46.8 %; throughput −0.3 % (CI −1.2 to +0.3 %), RPC p95 −0.071 ms (CI −0.235 to +0.084); syscall and wakeup counts unchanged. The history `retain` cost falls from 0.5256 to 0.0007 CPU s/GiB, and history/ACK processing supplies 95 % of the saving. In the three blocks with matched CUBIC controls it removes 35.1 % (CI 16.7–45.7 %) of BBRv3's CPU premium over CUBIC (1.151 / 2.550 / 2.059 CPU s/GiB for CUBIC / 1.4.1 BBRv3 / this patch); it does not remove BBR's syscall and wakeup premium. Report: `bench/cc-compare/results/incremental-history/report.md` in the `bbr-incremental-history` worktree (local, with raw perf recordings; not published). | `57c19a744` / `b591a4994` on `upgrade/noq-v1.4.0`, released as 1.4.2 at `a3c70b7af` | carried (released in 1.4.2, `aster-noq-v1.4.2`) | Low to medium conflict risk in `bbr3/mod.rs` packet-history bookkeeping; port after the BBR rows. Three tests check equivalence with the former full-scan algorithm over 400 batches in every packet-number space, wrapped queues, reordered ACKs, loss removals, the expiry boundary, duplicate-ACK bookkeeping, ACK before loss and delayed removal. Acknowledged and aged records keep the existing one-cleanup delay before removal. noq-proto 471, noq 39 (3 ignored), clippy and the scoped fmt check clean. |

Bookkeeping and documentation commits (not needed at runtime):

| Commits | Files | Purpose | Status |
|---|---|---|---|
| `0f797670a`, `f303f21fd`, `485a71caa`, `617899fe7` | `README.md`, `.gitignore`, transient roadmap files | Added, then moved, internal FFI/perf/security roadmap docs under an ignored `docs/aster/`. Net committed effect: fork README context and `docs/aster/` ignored. | Not on `upgrade/noq-v1.3.0` or `upgrade/noq-v1.4.0`. Optional; if carried forward, keep only committed fork context and the `.gitignore` entry, and keep internal roadmaps untracked. Superseded by the `main` README preamble under the current model. |

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
git cherry-pick -x f614cd043 d378ece18 c80473141 72c1d777a 5c77621f1 5da9ef194
git cherry-pick -x 7dcc5579c          # row eos, unless upstream fixed #794
git cherry-pick -x 4fe94b847 4be718e40 b392ac5a4  # row 31, unless upstream fixed #437
git cherry-pick -x 535bd8840          # row 1, empty-buffer fix
git cherry-pick -x 786f7ccda ae28c2bae  # bbr3-probe-feedback, bbr3-probe-growth (keep together)
git cherry-pick -x f5e70a515 bde811195  # stream-buffer-spans
git cherry-pick -x 21d11b4b9 cf212124f  # pacer-zero-delay, then bbr3-default
git cherry-pick -x b591a4994          # bbr3-incremental-history
git cherry-pick -x dd9c07514          # fmt; or re-run cargo fmt over the patch files only
# commits that already carry -x lines naming published commits: replay them as they
# are; do not add another -x that would name an unpublished trial commit
# version bump, only if the version rule requires it (Aster's registry has already
# served <upstream version> or higher in the series): set noq, noq-udp and noq-proto
# to the next patch above Aster's highest published, then
git commit -am "chore: release <published version>"
git push origin upgrade/noq-v<upstream version>
# do not tag yet: tag after the consumer trial (below)
```

Prefer `cherry-pick -x` for provenance. If a patch no longer applies, use the "What" and "Conflict or porting notes" columns above as the specification and reimplement. Published `upgrade/*` branches and tags are never rewritten; a failed attempt is superseded by new commits and a new version.

An Aster-only fix on the current base is a topic branch based on the upstream release tag (or on the commit of the patch it builds on, as `fix/read-into-empty-buffer` is based on `b392ac5a4`), cherry-picked `-x` onto the current upgrade branch, followed by a version-bump commit to the next patch above Aster's highest published version (the release commits for 1.3.1 and 1.3.2 followed rows `eos` and 31). A re-pin without a code change is a commit on the current upgrade branch with no version bump, tagged `aster-noq-v<current version>-p<n>` after the consumer trial.

Validate in this repository:

```bash
# fmt only the files the patches touch; upstream-only files are left as upstream formatted them.
# Expect no output (rustfmt follows `mod` declarations, so filter cargo fmt's report instead):
cargo fmt --all --check 2>/dev/null | grep '^Diff in' |
  grep -F -f <(git diff --name-only --diff-filter=d <upstream-tag> -- '*.rs')
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p noq-proto -p noq -p noq-udp    # the suite Aster's fork_compat gate runs (with --locked)
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

### 1.4.1 and 1.4.2 (2026-10-22), current

| Upstream base | Tag | Pinned commit | Branch |
|---|---|---|---|
| `noq-v1.4.0` (`34f75c49f`) | `aster-noq-v1.4.2` | `a3c70b7afe459cbf01122928f4f90d72063ab972` | `upgrade/noq-v1.4.0` |
| `noq-v1.4.0` (`34f75c49f`) | `aster-noq-v1.4.1` | `13dcb5b7f60240d729580014356fca6ab7f18ce3` | `upgrade/noq-v1.4.0` |

- **1.4.1** (`13dcb5b7f`): rows `bbr3-probe-feedback`, `bbr3-probe-growth`, `stream-buffer-spans`, `pacer-zero-delay` and `bbr3-default`, then a release commit (the registry already served 1.4.0). noq-proto 468, noq 39 (3 ignored), noq-udp 1, integration 9 (1 ignored), doctests 4; clippy and the scoped fmt check clean. The iroh family and Salvo 1.0.6 re-pinned to it.
- **1.4.2** (`a3c70b7af`): row `bbr3-incremental-history`, then a release commit. 1.4.1 was tagged and pinned but never published, so the version rule's "next patch above the highest served" (1.4.1) was already allocated; 1.4.2 is the next free version. Validation (macOS, 2026-10-22, `--locked`): noq-proto 471, noq 39 (3 ignored), noq-udp 1, integration 9 (1 ignored), doctests 4; clippy `--workspace --all-targets -D warnings` clean; the scoped fmt check reports only upstream's import order in `noq-proto/src/config/transport.rs`, unchanged since `noq-v1.4.0`. iroh, iroh-util, iroh-gossip, iroh-blobs, iroh-docs and Salvo re-pin to it (`-p<n>` tags); see their ledgers.

### 1.4.0 wave (2026-10-21)

| Upstream base | Tag | Pinned commit | Branch |
|---|---|---|---|
| `noq-v1.4.0` (`34f75c49f`) | `aster-noq-v1.4.0` | `535bd88409193ae577f445a3207351bb83525f4e` | `upgrade/noq-v1.4.0` |

- Upstream released `noq-v1.4.0` (2026-10-20), required by iroh 1.4.0. The fork was re-ported onto it as `upgrade/noq-v1.4.0`; every carried row still applies (upstream fixed neither #794 nor #437). Published as 1.4.0, the upstream version, since Aster's registry has never served it; no release commit.
- **Row 1** was rewritten onto upstream's `VecDeque` assembler (#823) and **row 31** merged with upstream's new `retire_cids()`; both were reviewed independently and found sound. `b392ac5a4` fixes row 31's imports for no_std `noq-proto`.
- **New: empty-buffer `read_into`** (row 1, `fix/read-into-empty-buffer` @ `e138284e8`, based on `b392ac5a4` because it fixes the ported row 1): `Chunks::read_into` returns `Ok(0)` for an empty buffer and leaves the stream unchanged. Test `read_into_empty_buffer_is_a_no_op`.
- **fmt:** the fmt commits (`c80473141`, `dd9c07514`) are limited to files the patches change. Upstream `noq-v1.4.0` is not rustfmt-clean under our toolchain, so `cargo fmt --all --check` fails on upstream-only files; the fmt check is scoped to the patch files.
- Validation (macOS, 2026-10-21): clippy clean; noq lib 39 passed (3 ignored), noq-proto 456, noq-udp 9 (1 ignored), integration 1 + 1 (1 ignored), doctests 3. Aster consumer: `cargo check --workspace --all-targets` and preflight green; the full suites run in the release candidate.

### 1.3.0 wave (2026-10-03 onward)

| Upstream base | Tag | Pinned commit | Branch |
|---|---|---|---|
| `noq-v1.3.0` (`c1f411562`) | `aster-noq-v1.3.2` | `e75f3a5550feae0f40df0ffde8bf438f213a5156` | `upgrade/noq-v1.3.0` |

- Stack on `upgrade/noq-v1.3.0`: `cd67e998d` (row 1), `6cfa2568a` (row 2), `b2711a973` (fmt), `c15dc5544` (lints), `30c50860c` (row 2), `67c136445` (tests), `5d67f7195` (row `eos`), `867a3a9a3` (release 1.3.1), `f4ae698fc` + `2c65fd452` (row 31), `e75f3a555` (release 1.3.2).

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
