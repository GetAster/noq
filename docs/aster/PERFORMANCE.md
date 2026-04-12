# noq Performance Improvement Roadmap

Tracking suspected high-value performance improvements for noq, informed by
Aeron IPC design patterns and dhat allocation profiling.

## Status of Submitted PRs

| PR | Status | Description |
|----|--------|-------------|
| #581 | Submitted | Reuse `response_buffer` across datagrams in recv loop |
| #582 | Submitted | Add dhat heap allocation profiling support |
| #583 | Submitted | Eliminate per-datagram memcpy and allocation in recv path |

## Profiling Baseline

Measured with dhat, 100 MB upload + 100 MB download, localhost:

- **Total program allocations:** 5.69M blocks / 1.16 GB
- **`poll_socket` (recv hot path):** 1.35M blocks / 681 MB
  - 677K blocks: BytesMut data copy from recv buffer (654 MB)
  - 677K blocks: `split_to` SharedVtable allocation (27 MB)
- **Uninstrumented throughput:** ~1.7 Gbps (single connection, localhost)

PR #583 reduces `poll_socket` to 668K blocks (−50.7%) and eliminates the
654 MB memcpy. Remaining 668K blocks are `BytesMut::zeroed` refills.

---

## Candidate Improvements (Prioritized)

### 1. SPSC Ring Buffer for Endpoint ↔ Connection Communication — INVESTIGATED, DEFERRED

**Status:** Investigated 2026-04-12. Deferred. Two of the roadmap's factual
premises about this item are wrong; the performance concern doesn't
materialize on measured workloads. Revisit only if paired with items 2 and 7
as a joint dispatch-rework — none of those individually show signal.

**Aeron concept:** `OneToOneRingBuffer` — pre-allocated power-of-2 buffer,
single-writer tail advance (no CAS), cached head position to reduce
cross-core cache line bouncing, padding records at wrap boundaries.

**Current noq:** `mpsc::UnboundedSender<ConnectionEvent>` per connection
(`endpoint.rs:680`), one forward channel per connection, used on the recv
dispatch hot path at `endpoint.rs:937`.

#### Investigation Findings

##### Finding 1: the "each `send()` allocates a node" claim is false

Tokio's `mpsc::unbounded` is **not** a per-message linked list — it is a
linked list of **blocks of ~32 message slots**, with an internal freelist
that recycles blocks within a channel. Per-message allocations do not
happen at steady state.

Verified against the dhat-heap.json from the item-4 baseline run
(`noq-perf` client, 100M up + 100M down, 10 s, ~167K packets/sec,
~1.67M total datagram sends):

| Frame | Blocks | Bytes |
|----|----|----|
| All tokio channel / mpsc / notify allocations combined | **38** | **25 KB** |
| `tokio::sync::mpsc::block::Block<T>::new` (the allocation per block) | **2** | ~9.5 KB |

If per-send allocations were happening as the roadmap claims, we would
expect ~1.67M or ~52K (amortized at 32 slots/block) allocations from that
frame. We see **two** over the entire run.

##### Finding 2: dispatch does not saturate at high connection counts

The roadmap predicted item 1 would be "most impactful at high connection
counts (100+) and high packet rates". Measured with `bench/bulk --clients N`
on `perf/item-4-base` (14-core M-series macOS, localhost, bidirectional):

| N clients | Wall | Agg upload | Agg download | Agg total |
|----|----|----|----|----|
| 1 | 0.29 s | 101 MiB/s | 101 MiB/s | **1.62 Gb/s** |
| 10 | 2.5 s | 130 | 112 | **1.94 Gb/s** |
| 100 | 9.0 s | 331 | 115 | **3.57 Gb/s** |
| 500 | 20.3 s | 825 | 229 | **8.43 Gb/s** |
| 1000 | 25.7 s | 1053 | 295 | **10.8 Gb/s** |

Single-endpoint server aggregate upload grows from 101 MiB/s at N=1 to
1053 MiB/s at N=1000 — a 10× increase as connection count grows 1000×.
At ~1200-byte packets, N=1000 recv dispatch is handling **~700K
packets/sec** — the exact workload item 1 was supposed to address — and
still has headroom (aggregate throughput was still growing with N, not
flattening).

The per-client throughput degrades (101 → 1.3 MiB/s) but that is driven
by 1001 threads (1 server + 1000 clients) sharing 14 cores on a single
host, not by server dispatch saturation. The widening asymmetry between
upload (1053) and download (295) at high N further suggests the
bottleneck at scale is send-side (`poll_transmit`) or client-side read,
not recv dispatch.

#### Conclusion

Item 1 attacks a bottleneck that does not exist on measured workloads.
The per-send allocation story is wrong, and the expected dispatch
saturation under connection scaling does not occur.

An SPSC ring buffer would still have two plausible arguments in its
favor, neither of which is justified in isolation:

1. **Cache locality / wake coalescing.** An SPSC ring buffer with inline
   storage has better cache behavior than a per-channel linked list of
   blocks + waker, and could reduce wakeup costs. But this only matters
   if item 7 (shared connection driver) also lands — otherwise every
   connection still has its own Tokio task with its own waker.
2. **Backpressure.** The roadmap correctly notes that `mpsc::unbounded`
   has no backpressure. A bounded SPSC ring does. But this is a
   correctness/DoS-resistance improvement, not a throughput improvement,
   and should be pursued (if at all) through a bounded `mpsc::channel`
   replacement, not a custom SPSC — the effort is an order of magnitude
   smaller and upstream-friendly.

#### Recommendation

**Skip item 1 as a standalone change.** The premise is wrong and the
measurement does not support the framing.

If dispatch overhead ever shows up as a real bottleneck on some workload
we care about, the right unit of work is items **1 + 2 + 7 together**
(SPSC + split endpoint mutex + shared driver) as a coordinated
dispatch-path rework, validated first against that workload. Until such
a workload is identified, the estimated 2–3 days for item 1 alone are
better spent elsewhere.

If the **backpressure / DoS-resistance** concern is the real motivation
(not throughput), open a separate bounded-channel proposal — it is a
much smaller change and does not need SPSC.

---

### 2. Split Endpoint Mutex (Driver-Owned vs Shared State) — INVESTIGATED, DEFERRED

**Status:** Investigated 2026-04-12. Deferred. Directly measured: cumulative
contention on the endpoint `Mutex<State>` is below 1% of wall-clock time
even at N=500 concurrent connections. The theoretical upside is far below
the ±2% measurement noise floor.

**Aeron concept:** Single-writer principle. The conductor owns mutable state
exclusively; publications/subscriptions communicate through lock-free
structures. No mutex on data paths.

**Current noq:** Single `Mutex<State>` on `EndpointInner` (`endpoint.rs:457`),
held across the entire `drive_recv` + `handle_events` cycle. Every
`accept()`, `local_addr()`, `open_connections()`, `stats()` call from
application code shares this lock.

#### Investigation Findings

The existing `noq/src/mutex.rs` tracking Mutex wraps the *connection*
state mutex, not the endpoint state mutex. For this measurement we
dropped in a throwaway `InstrumentedMutex<T>` wrapper on
`perf/experiment-mutex-instrument` that records per-acquisition wait
and hold time via `AtomicU64`, with aggregate stats printed via
`eprintln` on `Drop` (to survive tracing subscriber teardown).

Measured with `bench/bulk --clients N`, localhost, 14-core M-series
macOS, server-side endpoint `Mutex<State>`:

| N | Acquisitions | Wall | Wait total | Hold total | Wait / wall | Hold / wall | Avg wait |
|----|----|----|----|----|----|----|----|
| 1 | 3,412 | 0.4 s | 0.11 ms | 17.5 ms | **0.03%** | 4.4% | 32 ns |
| 10 | 13,178 | 1.2 s | 2.19 ms | 512.9 ms | **0.19%** | 42.7% | 166 ns |
| 100 | 117,862 | 8.2 s | 23.7 ms | 4806.1 ms | **0.29%** | 58.6% | 201 ns |
| 500 | 290,322 | 22.4 s | 167.0 ms | 10,442.9 ms | **0.75%** | 46.6% | 575 ns |

**`wait_total` is the aggregate time *any* thread spent blocked
acquiring this lock over the entire run.** It is the upper bound on
what splitting the mutex can save — you cannot save more wall-clock
than you currently lose to waiting.

#### Interpretation

**The 50% `hold/wall` ratio at N≥100 is misleading on first read.** The
endpoint driver holds the lock roughly half the wall-clock time while
processing recv batches, but "hold time" only becomes "contention" when
another thread is actually blocked waiting. Waiters see average waits
of **32–575 ns**, which is the fast-path uncontended
`std::sync::Mutex::lock` cost — real blocking waits are microseconds,
not nanoseconds. The driver holds the lock for half the wall-clock
time, but effectively nobody else is meaningfully waiting.

Contention *does* grow with N — wait/wall climbs from 0.03% at N=1 to
0.75% at N=500, a 25× increase — but the absolute level is nowhere
close to a measurable fraction of throughput. Extrapolating the trend,
it would take roughly N=5000+ concurrent connections on a single
endpoint to push contention above the ±2% noise floor, and at that
point host-CPU starvation dominates any single-lock cost anyway.

The roadmap's framing — "every `accept()`, `local_addr()`,
`open_connections()`, `stats()` call from application code contends" —
is strictly accurate at the API level, but the measured contention
shows these calls are *not actually concurrent with the driver at any
meaningful rate* on measured workloads. The driver's lock acquisitions
dominate count (~290K over 22s at N=500), application-thread
acquisitions are noise by comparison.

#### Recommendation

**Skip item 2 as a standalone change.** Splitting `State` into
`DriverState` + `SharedState` would be a correctness-risky refactor
(the `State` fields are interdependent — `inner`, `recv_state`,
`socket`, `events` all coordinate during event handling) for at most
~0.75% wall-clock improvement on the worst measured workload. Not
justified.

Revisit only if a specific workload emerges where application threads
call endpoint APIs at high rate concurrently with heavy traffic (e.g.,
a stats-scraping sidecar polling `stats()` at kHz rates during active
transfers). Aster does not have such a workload.

**Experimental branch:** `perf/experiment-mutex-instrument` — deleted
after measurement. The instrumented Mutex wrapper it added is not a
candidate for merging.

---

### 3. Send Pacing — CLOSED, ALREADY IMPLEMENTED

**Status:** Investigated 2026-04-12. **The roadmap's premise is factually
wrong.** Noq already has a complete token-bucket pacer at
`noq-proto/src/connection/pacing.rs` (326 lines, including comprehensive
tests). The roadmap claimed "there is no inter-packet pacing" — this is
false.

**Aeron concept:** `ControlledMessageDrain` — spread transmissions evenly
across RTT intervals instead of bursting.

**Actual noq implementation:** `pacing::Pacer` is a token-bucket pacer
with:
- Capacity derived from `(window * TARGET_BURST_INTERVAL) / RTT`
  targeting 2 ms burst intervals
- Token drain on each `on_transmit(packet_length)`
- `delay()` returning `Some(Duration)` when the bucket is empty,
  blocking further sends until tokens refill at 1.25× the congestion
  window per RTT (per draft-ietf-quic-recovery-34 §7.7)
- Capacity clamped between `MIN_BURST_SIZE * MTU` and
  `MAX_BURST_SIZE * MTU` (10–256 MTU-sized packets per burst)
- Integration at `connection/mod.rs:1939` via `pacing_delay()` and
  at `packet_builder.rs:316` via `pacing.on_transmit(size)`

The pacer is active for all connections, not behind a feature flag. It
has been present since the quinn-to-noq fork.

**How this went wrong:** The roadmap was written from Aeron documentation
("Aeron concept: ControlledMessageDrain") and a surface-level read of
`poll_transmit` without searching the proto crate for existing pacing
implementation. A `grep -r pacing noq-proto/src/` would have found it.

#### Recommendation

**Nothing to do.** The feature already exists and is tested.

---

### 4. BytesMut Recv Buffer Pooling — INVESTIGATED, DEFERRED

**Status:** Investigated 2026-04-12. Deferred. Not worth pursuing at
single-connection localhost workloads. May be revisited under items 1/2/7
(high connection count scaling) once those change the dispatch architecture.

**Aeron concept:** Pre-allocated term buffers that are recycled after consumers
finish processing.

**Current noq (after PR #583):** Each consumed recv slot is refilled with
`BytesMut::zeroed(chunk_size)` at `noq/src/endpoint.rs:878`. Dhat confirms this
is the dominant remaining allocation site in the recv path.

#### Investigation Findings

Measured on `perf/item-4-base` (merge of #582 dhat + #583 zero-copy),
`noq-perf` client, localhost, 100 MB upload + 100 MB download, 10 s duration.

**Baseline dhat allocations (client side):**

| Metric | Value |
|----|----|
| Total program allocations | 892,996 blocks / 239 MB |
| `poll_socket` total | 107,515 blocks / 158 MB |
| Dominant site `endpoint.rs:878` (`BytesMut::zeroed`) | 107,512 blocks / 158 MB |
| % of `poll_socket` at single site | 99.997% |
| Per-allocation size | ~1472 B (one MTU slot) |

**Baseline native throughput (5-run mean, 5 s per run):**

| Direction | Mean | Range | σ |
|----|----|----|----|
| Upload | 1619.1 Mb/s | 1588.5–1639.9 | ~20 |
| Download | 1643.5 Mb/s | 1635.8–1655.7 | ~8 |

**Run-to-run noise floor: ±2% upload, ±1% download.**

#### Cost analysis

At 1.6 Gbps per direction and ~1200 B packets, the recv path sees
~167K packets/sec. Each packet triggers one `BytesMut::zeroed(1472)` call at
line 878 (slot handoff via `mem::replace` leaves empty slots that fail the
`capacity < chunk_size` check next cycle):

- **Memset bandwidth:** 167K × 1472 B ≈ **246 MB/s** zero-fill traffic.
  Modern CPU memory bandwidth is ~20–50 GB/s, so this is ~0.5–1% of
  available bandwidth. Negligible.
- **Allocation rate:** 167K/sec. Modern allocators (jemalloc, macOS libmalloc)
  sustain >10M allocations/sec for small allocations served from thread-local
  caches. 167K/sec is ~1–2% of a thread's alloc budget. Negligible.

**Predicted upper bound on throughput improvement: <3%.**

#### Empirical verification

Built `perf/experiment-skip-zero` (throwaway) with the zero-fill replaced by
`BytesMut::with_capacity` + `unsafe set_len` — eliminates both the memset
bytes and the `BytesMut::zeroed` allocation count at line 878. This variant
is UB per Rust's memory model (creates a `&mut [u8]` over uninitialized
memory before `recvmmsg` writes it) and was only used for measurement.

**Skip-zero throughput (5-run mean, two sessions, 5 s per run):**

| Session | Upload mean | Delta vs baseline | Download mean | Delta vs baseline |
|----|----|----|----|----|
| Skip-zero session 1 | (not captured) | — | 1662.7 Mb/s | +1.2% |
| Skip-zero session 2 | 1600.0 Mb/s | −1.2% | 1643.3 Mb/s | 0.0% |

**Result: all measured deltas are within the ±2% baseline noise floor.**
Eliminating the entire recv-slot allocation+memset cost produces no
detectable throughput improvement on single-connection localhost bulk
transfer. The 107K dhat-visible allocations are real but their wall-clock
cost is dominated by other work on this workload (crypto, kernel transitions,
QUIC state machine).

#### Why the roadmap's framing was misleading

The original roadmap entry was based on dhat block counts (107K–668K,
depending on run length), which make the recv slot refill look like a hot
spot. Block counts tell us how *often* allocations happen, not how *expensive*
they are. For small, thread-local-cacheable allocations with corresponding
small memsets, frequency does not translate to wall-clock cost.

Item 4 would still matter in three scenarios, none of which apply to the
localhost bulk-transfer workload we measured:

1. **High connection count (1000+).** Per-connection recv state multiplies
   allocation pressure; may stress allocator global locks. Items 1/2/7 are
   better suited to address this since they rearchitect dispatch.
2. **Platforms with slower allocators** (e.g., default Windows heap) where
   per-allocation overhead is an order of magnitude higher than
   jemalloc/libmalloc's tcache-served small allocations.
3. **Memory-bandwidth-starved CPUs** (embedded / ARM with small caches)
   where 246 MB/s of zero-fill may displace working-set data.

None of these are Aster's primary deployment target.

#### Proper implementation (if ever resurrected)

Drop-hook recycling requires either:
- **(a)** A custom `PooledBytesMut` wrapper type replacing `BytesMut` on the
  recv → proto → connection dispatch path. Invasive (~20 files), proto API
  diverges from upstream quinn.
- **(b)** Forking the `bytes` crate to add a drop hook to `BytesMut`. Ugly
  dependency fork.
- **(c)** Restructuring proto to use out-of-place AEAD decryption so `Bytes`
  (immutable, supports `Bytes::from_owner` drop hook) works as the handoff
  type. Large proto change.

`try_reclaim` after `handle()` does not work because proto stashes buffer
refs inside `DatagramEvent::ConnectionEvent` (in `DatagramConnectionEvent`)
and dispatches them asynchronously to connection tasks — refcount is >1
by the time control returns to the recv loop.

#### Recommendation

**Skip item 4. Proceed directly to item 1** (SPSC ring buffer for
endpoint↔connection dispatch). Items 1 and 2 restructure the recv dispatch
path in ways that will likely obsolete the current recv_bufs design entirely,
making any item 4 implementation throwaway work.

**Prerequisite:** PR #583 accepted (for the zero-copy baseline these
measurements are relative to).

---

### 5. Slab Allocator for Stream State — INVESTIGATED, DEFERRED

**Status:** Investigated 2026-04-12. Deferred. Stream-count scaling
curve shows no throughput degradation up to 1000 concurrent streams.
FxHashMap is not a bottleneck.

**Aeron concept:** Arrays indexed by stream/registration ID for O(1) lookup
with cache-friendly iteration.

**Current noq:** `FxHashMap<StreamId, Send/Recv>` in the proto layer. FxHashMap
is fast but each entry is a separate heap allocation, and iteration order is
random (cache-unfriendly).

#### Investigation Findings

Measured with `bench/bulk -n N -m N` (N concurrent streams, single
connection), total data kept constant at ~20 MiB regardless of stream
count, on `perf/item-4-base`, 14-core M-series macOS:

| Streams | Per-stream | Wall | Upload | Download |
|----|----|----|----|-----|
| 1 | 20 MiB | 0.30 s | 100 MiB/s | 100 MiB/s |
| 10 | 2 MiB | 0.30 s | 97 MiB/s | 97 MiB/s |
| 100 | 205 KB | 0.28 s | 103 MiB/s | 103 MiB/s |
| 1000 | 20 KB | 0.27 s | 119 MiB/s | 119 MiB/s |

Throughput is **flat to increasing** as stream count grows 1000×.
Higher stream counts are actually marginally *faster* because per-stream
congestion-window startup cost is amortized.

The roadmap's framing that FxHashMap "is fast but each entry is a
separate heap allocation" mischaracterises how `rustc_hash::FxHashMap`
works. `FxHashMap` stores entries inline in a flat array (Robin Hood
hashing / SwissTable depending on version); there is no per-entry heap
allocation. Cache locality is already good.

#### Recommendation

**Skip item 5.** FxHashMap is not the bottleneck at any measured stream
count. The slab replacement would trade FxHashMap's flexibility (works
with any key type, supports deletion without fragmentation) for
marginal constant-factor improvement that is not visible in throughput.

Revisit only if a `perf stat` profile on a specific workload shows
L1/LLC cache misses dominated by stream-map iteration — which would
require a very different workload (10,000+ streams with frequent
iteration over the full map, not common in Aster's blob/gossip
patterns).

---

### 6. Adaptive Idle Strategy (BackoffIdleStrategy) — INVESTIGATED, DEFERRED

**Status:** Investigated 2026-04-12. Deferred. The effect item 6 targets is
real but bounded to ~10–20 µs per cold request, which is invisible on all
of Aster's expected workloads.

**Aeron concept:** Progressive backoff state machine:
`NOT_IDLE → SPINNING (10 spins) → YIELDING (5 yields) → PARKING (1µs→1ms)`.
Reset on work detection.

**Current noq:** The endpoint driver returns `Poll::Pending` and relies on
Tokio's waker, which parks the task immediately. First packet after idle pays
full task wake-up cost.

#### Investigation Findings

Measured using `noq-perf client` in request-response mode on
`perf/item-4-base`, 1024 B upload + 1024 B download per request, localhost,
14-core M-series macOS.

**Pipelined (`--bi-requests 8`, no genuine idle time — steady state):**

| Percentile | Upload duration | FBL (server processing) |
|----|----|----|
| P0 | 28 µs | 0 |
| P50 | 79 µs | 0 |
| P90 | 107 µs | 24 µs |
| P100 | 5.74 ms | 5.50 ms |

Median round-trip is 79 µs, P90 107 µs, FBL P90 24 µs. The P100 outlier
(5.74 ms upload / 5.50 ms FBL, co-located in the same request) is a
single-request scheduler hiccup, not an idle-wake phenomenon — item 6
cannot touch it.

**Sequential (`--bi-requests 1`, dominated by QUIC delayed-ACK on fin):**

Upload P50 = 26.4 ms, RPS = 38. This is driven by QUIC's default
`max_ack_delay` (25 ms) being exercised on every stream finish because
there is no pipelining to trigger an immediate ACK. Item 6 is not the
right tool for this — the right tool is tuning `ack_frequency_config`,
which is unrelated to idle strategy.

#### Cost analysis

Item 6 specifically targets the latency of the *first packet after the
driver has parked*. Tokio's task unpark on macOS is approximately
5–20 µs depending on runtime configuration and load. An adaptive idle
strategy (spin-before-park) could plausibly eliminate this on cold
requests, saving **at most ~20 µs per cold request**.

The workloads where this matters must satisfy **all three**:
1. Inter-request gap long enough for the driver to actually park (>100 µs typically)
2. Request latency dominated by the unpark cost (so <1 ms total)
3. User/application actually sensitive to sub-100-µs latency variations

Aster's real workloads do not satisfy any of these:
- Stream-heavy (blob transfer, gossip), not sparse RPC
- WAN deployment: RTT is 20–200 ms, so 20 µs is 0.01–0.1% of one round trip
- Latency envelope is already dominated by network, not local dispatch

Even for the localhost dev-loop use case (which *could* benefit), the
improvement is ~20 µs on ~80 µs requests — ~25% — but only for genuinely
idle-then-wake requests. A bench that actually exercises this pattern
does not exist and would have to be built.

#### The real localhost latency issue (out of scope for item 6)

The sequential-request test surfaced an unrelated but larger issue: the
default ack delay is 25 ms, which dominates any sub-millisecond latency
concern on single-flight request-response workloads. If latency
optimization is ever a real Aster goal, the first lever is
`AckFrequencyConfig` / `max_ack_delay`, not idle strategy. This is a
10³× larger opportunity than item 6 on the workload where it matters.

#### Recommendation

**Skip item 6.** The theoretical ceiling is ~20 µs per cold request,
which is below noise on every workload Aster cares about. If a
"localhost dev-loop cold request latency" concern is ever raised, the
fix is either (a) pin the driver to a CPU and disable parking, or
(b) tune `max_ack_delay`. Item 6 as specified is not the right
intervention.

**Effort:** 0.5 days estimated, but no reason to spend it.

---

### 7. Shared Connection Driver (Architectural) — DEFERRED, CLOSED BY ITEMS 1 + 2

**Status:** Closed analytically 2026-04-12. Items 1 and 2 established that
dispatch and mutex contention are not measurable bottlenecks at N=1000
concurrent connections. Item 7 depends on the same premise — that
per-connection task scheduling overhead is a material cost. Since dispatch
throughput kept scaling (10× aggregate from N=1 to N=1000) and mutex
contention stayed under 0.75% of wall-time, per-connection task overhead
is not the binding constraint at any measured scale.

**Aeron concept:** Single conductor thread processes all connections in a tight
loop, avoiding per-connection task scheduling overhead.

**Current noq:** Each connection spawns its own async task
(`ConnectionDriver`). At scale (thousands of connections), each task has its
own stack allocation, scheduling overhead, and waker management.

**Effort:** Very high. Major architectural change to the async wrapper layer.
Would not change the proto layer.

#### Why it's closed without a standalone experiment

Item 7's argument is: "at high connection count, per-task scheduling
overhead (stack allocation, waker management, Tokio runtime scheduling)
dominates." This only holds if the dispatch path is the bottleneck — but
item 1's scaling curve showed dispatch handles ~700K packets/sec at
N=1000 with throughput still growing, and item 2 showed the endpoint
mutex contributes <0.75% of wall-time at N=500.

If dispatch is not saturated and the mutex is not contended, the remaining
per-connection costs (stack, waker) are bounded by Tokio's implementation
and are not reducible by changing the noq architecture. The only scenario
where item 7 becomes relevant is N=10,000+ connections on a single
endpoint, which is outside Aster's deployment profile.

#### Recommendation

Do not pursue. If items 1 + 2 are ever resurrected due to a new
workload showing dispatch saturation, item 7 should be revisited as
part of the same coordinated rework — it is not independently
justifiable.

---

## Measurement Infrastructure Needed

To properly evaluate candidates 1–7, the project would benefit from:

- **High-packet-rate small-message benchmark:** Many small datagrams (100–200
  bytes) at maximum packet rate. The existing benchmarks lean toward bulk
  transfer. This is the scenario where per-datagram overhead dominates.
- **Connection-scaling benchmark:** 100, 1000, 10000 concurrent connections
  with modest traffic each. Tests dispatch and scheduling overhead.
- **Higher-percentile latency capture:** Extend `perf/src/stats.rs` to report
  p99.9 and p99.99, not just p90.

---

## References

- [Aeron Design Overview](https://github.com/aeron-io/aeron/wiki/Design-Overview)
- [Aeron Media Driver Operation](https://github.com/aeron-io/aeron/wiki/Media-Driver-Operation)
- [Aeron Flow and Congestion Control](https://github.com/aeron-io/aeron/wiki/Flow-and-Congestion-Control)
- [aeron-rs (Rust implementation)](https://github.com/UnitedTraders/aeron-rs)
