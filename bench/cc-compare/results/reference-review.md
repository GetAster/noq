# Patched Noq BBRv3 reference review

Candidate: `5a20cb4` on `investigate/bbr3-recovery`, relative to Aster's Noq
pin `535bd88409193ae577f445a3207351bb83525f4e`. This evaluation changes no
production defaults. Benchmark instrumentation is separate from the controller.

References:

- [BBR draft 06](https://www.ietf.org/archive/id/draft-ietf-ccwg-bbr-06.html).
- [Google TCP BBRv3, pinned revision 90210de4](https://github.com/google/bbr/blob/90210de4b779d40496dee0b89081780eeddf2a60/net/ipv4/tcp_bbr.c).
- [Google QUIC byte-unit probe growth](https://github.com/google/quiche/blob/main/quiche/quic/core/congestion_control/bbr2_probe_bw.cc).

This is a focused implementation review for a default-controller decision,
not a claim that Noq implements every draft-06 or Linux TCP detail. The
comparison is CUBIC versus this Rust Noq BBRv3 implementation, not versus
Google's TCP binary.

| Area | Review and evidence |
|---|---|
| End of probe feedback | The candidate clears probe-sample state and moves ACK phase to Init at the end of the feedback round, including application-limited samples and transitions outside ProbeBW. Bandwidth-filter advancement remains conditional. This matches the reference ordering; three regression tests cover it. |
| Probe-up growth units | Noq's ACK budget is bytes; each earned increment adds one SMSS. The budget floor is SMSS and the divisor is `2^rounds`. This matches the byte-based QUIC formulation and the packet-based TCP formulation after unit conversion. A non-packet-aligned-window test exercises the rounding and retained remainder. |
| Growth bounds | The exponent is capped at 30. The retained ACK budget is reduced by earned increments. The existing tenfold-capacity-increase test passes without lowering its target or increasing its discovery-round limit. |
| Startup and drain | Bandwidth plateau and high-loss exits, recovery duration, discontiguous loss-event counting, inflight-based drain and the drain-time escape have behavioral tests. |
| Bandwidth model | Application-limited samples cannot lower the maximum-bandwidth estimate; the maximum filter ages by probe cycles; congestion lower bounds adapt and are reset on refill. Actual callback timing is exercised by encrypted endpoint benchmarks, in addition to controller tests. |
| ACK aggregation | Excess-ACK allowance, packet-to-byte conversion of the ACK-frequency threshold and the requested max-ACK-delay term are present. Appendix tests cover aggregation in startup and cruise. Native tests use the default ACK policy. |
| Pacing and batching | BBR exports its explicit byte rate and send quantum. The connection uses those metrics for pacing and aggregate sizing. Dedicated pacing tests exercise rate enforcement, burst budget and rate changes; native GSO-on/off runs exercise the socket path. |
| MTU and initial window | Initial window is bounded using the starting datagram size. MTU updates adjust SMSS, minimum pipe window and quantum. Controlled tests fix MTU at 1200; physical-path tests retain MTU discovery. |
| ProbeRTT and idle | Five-second probe interval, minimum probe duration, minimum-pipe floor, round completion and idle-restart logic are present. Existing tests cover startup/steady-state entry, exit and idle suppression. |
| Recovery and undo | Persistent-congestion floor, loss during probing, policer behavior, spurious fast-recovery/RTO undo and tenfold bandwidth increases/decreases have tests. Native recovery measurements provide a separate end-to-end check. |

All 22 existing Appendix-A example scenarios are represented by named tests
in `noq-proto/src/congestion/bbr3/mod.rs`; the full protocol library suite was
rerun during this evaluation: **460 passed**. These are implementation tests,
not an independent conformance test suite.

Differences and limits to retain in the adoption decision:

- Noq does not carry the reference's `prev_probe_too_high` /
  `prev_probe_precautionary` state for precautionary probing. The two fixes do
  not add that mechanism. Therefore this is not a complete draft-06 port.
- ECN notifications enter the controller through its packet-loss bookkeeping,
  with zero lost bytes supplied by the connection. This deserves direct CE
  testing; normal successful UDP transfers alone do not validate it. The
  supplementary fq_codel/ECN case records marking and drop counters, but does
  not certify the exact CE response in every controller phase.
- The Controller trait supplies no packet-number-space argument; the BBR
  adapter treats callbacks as Data-space callbacks. The source already flags
  this limitation. End-to-end handshakes are exercised, but no new exhaustive
  handshake/0-RTT packet-space audit is claimed.
- Model/control updates run during per-packet ACK callbacks, while rate-sample
  finalization occurs in `on_end_acks`. This differs from a literal
  once-per-ACK pseudocode transcription. The real-endpoint tests are important
  evidence for this integration; matching helper functions is insufficient.
- BBR's private probe RNG remains entropy-seeded. Repetitions characterize
  that variability; packet-loss seeds do not make the whole run deterministic.

The two corrections themselves agree with the references and have regression
evidence. A recommendation to enable this candidate should rely on the
controlled and native results alongside these explicitly retained limits.
