# Aster adoption: Noq, BBRv3 and Salvo

Adopt the fork changes as separate patches, then update Aster's transport code
and pins together. Canonical patch ledgers stay on each fork's `main`;
integration branches carry code and release commits. No default or release
record has been changed by this evaluation.

## Noq patch sources

| Patch | Topic branch | Source commit | Base |
|---|---|---|---|
| Reset BBR probe feedback | `fix/bbr3-probe-feedback` | [a63916257](https://github.com/GetAster/noq/commit/a6391625784621fba53e7259d57ef3d109123245) | Current integration `535bd884` |
| Packet-sized probe growth | `fix/bbr3-probe-growth` | [910abb85c](https://github.com/GetAster/noq/commit/910abb85cbbfc42f2174558e2a0f8b83e1c60776) | Prerequisite `a63916257` |
| Receive-buffer coalescing | `fix/stream-buffer-compaction` | [69a1df075](https://github.com/GetAster/noq/commit/69a1df075d7c13265ae25cf809523b674db9a36d) | Current integration `535bd884`; regression tests use row 1's `read_into` |
| Preserve unordered offsets during compaction | Same topic | [3ae189bba](https://github.com/GetAster/noq/commit/3ae189bba41024a6daf29a353460f373860c0e26) | Prerequisite `69a1df075` |

The clean BBR topics contain the same controller changes as the report's
original commits `8113467` and `1e0c42c`, with their provenance retained.
The stream-buffer fix is independent of BBR. It coalesces adjacent receive
buffers under chunk pressure and preserves the 1024-distinct-span cap. Its
follow-up preserves unread lower offsets after unordered high-offset reads;
carry both compaction commits.

Append these patches to `upgrade/noq-v1.4.0` using `cherry-pick -x`, in table
order. The benchmark history remains on `investigate/bbr3-recovery`.
Keep the BBR reset and growth corrections together: reset alone fails the
existing tenfold bandwidth-recovery test.

Create a separate `bbr3-default` topic commit changing
`TransportConfig::default()` to `Bbr3Config::default()`. Both Iroh's default
builder and Salvo's listener start with that transport configuration, so the
fork default applies to both. Check for explicit controller overrides during
the consumer port. Keep CUBIC selectable for comparisons and diagnosis.

Allocate fresh registry versions for the changed Noq crates under the ledger's
version rule. Record the final integration commit after the default and release
commits; it will differ from every evaluation pin above.

## Salvo and Aster

Salvo's candidate is `feat/noq-backend-v1.0.1` in
`salvo-worktrees/noq-backend`. Its code commit is `39564f65`; `12904e0e` adds the
explicit `salvo::proto::noq` re-export and review documentation. The base is
the assembled 1.0.5 line `80c472ac`, on upstream v1.0.1; its vendored H3 and
WebTransport patches are prerequisites.

Append the candidate to `upgrade/salvo-v1.0.1` with `cherry-pick -x`. Replace
its evaluation Noq pin in both the workspace and vendored `h3-quinn` manifest
with the final Noq integration pin. Update the documented controller default
when making that pin change.

The Aster port needs the socket ownership/lifecycle changes in
`aster-transport-salvo/src/listener.rs` and the stats conversion in
`raw_webtransport.rs`. Use connection-wide counters plus the active browser
path (`PathId::ZERO` with multipath disabled), retaining snapshots after close.
`salvo::proto::quinn` remains a compatibility name for Noq types; upstream
Quinn types are incompatible with it.

Update Aster's dependency pins and `iroh-fork-manifest.toml`, and propagate
Noq re-pins through Iroh and the satellite forks. Every normal server build
should resolve one Noq source/revision. Upstream Quinn can remain in independent
client dev-dependencies. Update the native-stack publisher's Noq dependency
rewrites and allocate fresh versions throughout the changed Salvo dependency
family before publishing.

Run Aster's transport/edge suites and the existing Salvo acceptance checks:
socket rebind after stop, cancellation, reset/FIN handling, datagrams,
owned-session shutdown, and Chrome/Safari negotiation captures. The separate
Quinn-versus-Noq H3/WebTransport comparison still belongs to the backend port;
the direct-Noq congestion-controller benchmarks do not cover it. Compare the
backends with an explicit common controller before attributing a difference
to BBRv3.

## Ledger changes at adoption

In Noq's `main:docs/aster-patches.md`, add these stable short-name entries;
fill the integration-commit column after cherry-picking:

| Id | Reason | Porting and validation notes |
|---|---|---|
| `bbr3-probe-feedback` | End probe feedback once; clear stale sample/ACK-phase state. Source `a63916257`, related to upstream #819. | Reset for app-limited samples and outside ProbeBW; advance the bandwidth filter conditionally. Three transition regressions. Carry with the growth fix. |
| `bbr3-probe-growth` | Preserve packet-sized inflight growth while accounting ACK budgets in bytes. Source `910abb85c`, prerequisite previous row. | Preserve ACK remainder and the SMSS floor; test packet steps and unchanged tenfold recovery target/round bound. |
| `stream-buffer-spans` | Prevent legitimate large-window transfers from exhausting a raw-buffer count behind one gap, and preserve unread lower offsets during unordered compaction. Sources `69a1df075`, `3ae189bba`. | VecDeque assembler patch, based on `535bd884` for row 1 compatibility. Preserve ordered/unordered byte correctness, allocation reuse and the distinct-gap cap. Retain gap, duplicate and overlap-flood tests. |
| `bbr3-default` | Use the corrected controller across Iroh and Salvo. Source commit assigned during adoption. | Apply after both BBR fixes; record CPU, ECN and reordering tradeoffs from the report. Verify the consumer's selected controller. |

In Salvo's `main:docs/aster-patches.md`, add the backend substitution using
the next available patch id. Record the assembled base and prerequisites,
source and integration commits, the Noq/Quinn type-alias warning, and required
Aster socket/stats changes. State that this supersedes the 2026-05-22
two-QUIC-stack decision. Its benefit is a common QUIC implementation and patch
set; its maintenance cost is rechecking the adapter on Salvo/Noq upgrades.

Update both ledgers' current-release tables only when those assemblies are
accepted, including exact versions, integration commits and tags. Update the
existing Noq dependency-pin entries in the other fork ledgers and Aster's
single fork manifest. Code changes require a fresh crate version rather than
a re-pin-only tag suffix. Create immutable release tags after the exact Aster
consumer trial passes. All changes stay pending until that integration work
is complete.
