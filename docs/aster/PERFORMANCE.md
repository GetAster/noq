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

### 1. SPSC Ring Buffer for Endpoint ↔ Connection Communication

**Aeron concept:** `OneToOneRingBuffer` — pre-allocated power-of-2 buffer,
single-writer tail advance (no CAS), cached head position to reduce
cross-core cache line bouncing, padding records at wrap boundaries.

**Current noq:** `mpsc::UnboundedSender<ConnectionEvent>` per connection
(`endpoint.rs:680`). Tokio's unbounded channel is a linked list — each
`send()` allocates a node. No backpressure.

**Expected impact:** Eliminates per-datagram channel node allocation, provides
natural backpressure, reduces atomic operations on the dispatch hot path.
Most impactful at high connection counts (100+) and high packet rates.

**How to prove:** Add a connection-scaling benchmark (100, 1000, 10000
concurrent connections with modest traffic each). Profile channel allocation
overhead with dhat. Microbenchmark Tokio unbounded mpsc vs custom SPSC ring
for the specific message types used.

**Effort:** 2–3 days. Medium-high complexity — touches the endpoint/connection
communication contract.

**Prerequisite:** PR #583 accepted (establishes allocation profiling pattern).

---

### 2. Split Endpoint Mutex (Driver-Owned vs Shared State)

**Aeron concept:** Single-writer principle. The conductor owns mutable state
exclusively; publications/subscriptions communicate through lock-free
structures. No mutex on data paths.

**Current noq:** Single `Mutex<State>` on `EndpointInner` (`endpoint.rs:457`),
held across the entire `drive_recv` + `handle_events` cycle. Every
`accept()`, `local_addr()`, `open_connections()`, `stats()` call from
application code contends on this same lock.

**Expected impact:** Removes the mutex from the recv hot path entirely.
Matters under concurrent access patterns (accepting connections while
processing traffic). Less impactful for single-connection benchmarks.

**How to prove:** Add a benchmark with concurrent accept + high-rate traffic.
Use the existing lock duration tracking (`noq/src/mutex.rs`) to measure
contention before/after. Profile with `mutrace` on Linux.

**Effort:** 1–2 days. Requires splitting `State` into `DriverState`
(owned exclusively by the driver task, no mutex) and `SharedState`
(behind a lightweight lock or atomics for stats/ref_count/waker).

---

### 3. Send Pacing

**Aeron concept:** `ControlledMessageDrain` — spread transmissions evenly
across RTT intervals instead of bursting.

**Current noq:** `poll_transmit` writes as fast as the congestion window allows
within a single call. BBR and Cubic control the window size, but there is no
inter-packet pacing. This can cause buffer bloat at intermediate switches.

**Expected impact:** Reduced packet loss under congestion, improved throughput
in lossy/WAN conditions. BBR explicitly models a pacing rate that noq doesn't
currently enforce.

**How to prove:** Run the existing netsim benchmarks (WAN: 50ms/100Mbps/0.1%
loss, Lossy: 20ms/100Mbps/5% loss) before/after. Measure retransmission
rates and goodput.

**Effort:** 1–2 days. Add a token-bucket or leaky-bucket pacer between
`poll_transmit` and the socket send path.

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

### 5. Slab Allocator for Stream State

**Aeron concept:** Arrays indexed by stream/registration ID for O(1) lookup
with cache-friendly iteration.

**Current noq:** `FxHashMap<StreamId, Send/Recv>` in the proto layer. FxHashMap
is fast but each entry is a separate heap allocation, and iteration order is
random (cache-unfriendly).

**Expected impact:** O(1) access, better cache locality, no hashing overhead.
Matters for workloads with many concurrent streams (100+).

**How to prove:** Add a high-stream-count benchmark. Profile with `perf stat`
for cache miss rates before/after.

**Effort:** 1–2 days. Replace `FxHashMap` with `slab` crate, indexed by
stream ID.

---

### 6. Adaptive Idle Strategy (BackoffIdleStrategy)

**Aeron concept:** Progressive backoff state machine:
`NOT_IDLE → SPINNING (10 spins) → YIELDING (5 yields) → PARKING (1µs→1ms)`.
Reset on work detection.

**Current noq:** The endpoint driver returns `Poll::Pending` and relies on
Tokio's waker, which parks the task immediately. First packet after idle pays
full task wake-up cost.

**Expected impact:** Reduced p99/p99.9 latency for request-response workloads
with variable inter-request gaps.

**How to prove:** Extend the `perf` binary to capture p99.9 and p99.99
latency. Run request-response benchmark with varying inter-request gaps.

**Effort:** 0.5 days. Config knob on the endpoint driver: spin for N
iterations before yielding to the runtime.

---

### 7. Shared Connection Driver (Architectural)

**Aeron concept:** Single conductor thread processes all connections in a tight
loop, avoiding per-connection task scheduling overhead.

**Current noq:** Each connection spawns its own async task
(`ConnectionDriver`). At scale (thousands of connections), each task has its
own stack allocation, scheduling overhead, and waker management.

**Expected impact:** Significant at high connection counts (1000+). Reduces
Tokio runtime scheduling overhead.

**How to prove:** Connection-scaling benchmark measuring CPU per connection.

**Effort:** Very high. Major architectural change to the async wrapper layer.
Would not change the proto layer.

**Note:** This is a long-term consideration, not a near-term PR.

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
