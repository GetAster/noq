
#[cfg(test)]
mod qualification_gap_limit {
    use super::*;
    use alloc::vec;
    #[test]
    fn one_missing_packet_then_contiguous_full_buffers_must_not_exhaust_gap_limit() {
        let mut asm = Assembler::new();
        let payload = Bytes::from(vec![0x55; 1200]);
        // Exactly one missing packet, then one contiguous received span.
        for i in 1..=2049u64 {
            asm.insert(i * 1200, payload.clone(), 1200)
                .unwrap_or_else(|err| panic!("one gap rejected after {i} received frames: {err:?}"));
        }
    }
}
