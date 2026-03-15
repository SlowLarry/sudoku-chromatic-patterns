/// Iterate set bit positions in a u128 bitmask.
#[inline]
pub fn iter_bits(mut mask: u128) -> impl Iterator<Item = u8> {
    std::iter::from_fn(move || {
        if mask == 0 {
            None
        } else {
            let bit = mask.trailing_zeros() as u8;
            mask &= mask - 1; // clear lowest set bit
            Some(bit)
        }
    })
}

/// Count set bits.
#[inline(always)]
pub fn popcount(mask: u128) -> u32 {
    mask.count_ones()
}

/// Count set bits in u16.
#[inline(always)]
pub fn popcount16(mask: u16) -> u32 {
    mask.count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iter_bits_empty() {
        assert_eq!(iter_bits(0).collect::<Vec<_>>(), vec![]);
    }

    #[test]
    fn test_iter_bits() {
        let mask: u128 = (1 << 0) | (1 << 5) | (1 << 80);
        assert_eq!(iter_bits(mask).collect::<Vec<_>>(), vec![0, 5, 80]);
    }

    #[test]
    fn test_popcount() {
        assert_eq!(popcount(0), 0);
        assert_eq!(popcount(0b1011), 3);
        assert_eq!(popcount((1u128 << 81) - 1), 81);
    }
}
