//! strips pointer-authentication and top-byte tags so a tagged pointer still
//! matches the heap block it points at

/// masks to try in order: identity, top byte cleared, top 16 bits cleared
pub fn candidate_addresses(word: u64) -> [u64; 3] {
    const TBI_MASK: u64 = 0x00FF_FFFF_FFFF_FFFF;
    const PAC_MASK: u64 = 0x0000_FFFF_FFFF_FFFF;
    [word, word & TBI_MASK, word & PAC_MASK]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_yield_distinct_candidates_when_high_bits_set() {
        let w = 0xAAAA_0000_BBBB_CCCCu64;
        let cands = candidate_addresses(w);
        assert_eq!(cands[0], w);
        assert_eq!(cands[1], 0x00AA_0000_BBBB_CCCC);
        assert_eq!(cands[2], 0x0000_0000_BBBB_CCCC);
    }

    #[test]
    fn clean_pointer_dedupes_to_itself() {
        let w = 0x0000_AAAA_BBBB_CCCCu64;
        let cands = candidate_addresses(w);
        // no high bits set, so all three are equal.
        assert_eq!(cands[0], cands[2]);
    }
}
