//! Card C2 property tests: money arithmetic invariants.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test code: rule B2 lets tests unwrap and panic"
)]

use ledgerly_core::money_ops::{split_evenly, sum};
use ledgerly_core::{MoneyError, Paise};
use proptest::prelude::*;

proptest! {
    // invariant (derived algebraically): the parts of a split add up to the total.
    #[test]
    fn split_sums_to_input(total in any::<i64>(), parts in 1u32..2000) {
        let split = split_evenly(Paise::new(total), parts).unwrap();
        prop_assert_eq!(split.len(), usize::try_from(parts).unwrap());
        prop_assert_eq!(sum(split).unwrap(), Paise::new(total));
    }

    // invariant (derived): parts differ by at most one paisa, and the larger magnitudes come
    // first (remainder to the first parts).
    #[test]
    fn split_parts_are_even_and_ordered(total in any::<i64>(), parts in 1u32..2000) {
        let split = split_evenly(Paise::new(total), parts).unwrap();
        let values: Vec<i64> = split.iter().map(|p| p.get()).collect();
        let max = values.iter().copied().max().unwrap();
        let min = values.iter().copied().min().unwrap();
        prop_assert!(max - min <= 1);
        for pair in values.windows(2) {
            if let [a, b] = pair {
                prop_assert!(a.unsigned_abs() >= b.unsigned_abs());
            }
        }
    }

    // invariant (derived): checked sum of small amounts equals the i128 sum, which cannot overflow.
    #[test]
    fn sum_matches_wide_sum(values in proptest::collection::vec(-1_000_000_000_000i64..1_000_000_000_000, 0..50)) {
        let wide: i128 = values.iter().map(|v| i128::from(*v)).sum();
        let got = sum(values.iter().map(|v| Paise::new(*v))).unwrap();
        prop_assert_eq!(i128::from(got.get()), wide);
    }

    // invariant (derived): two amounts overflow exactly when the i128 sum leaves the i64 range.
    #[test]
    fn sum_overflow_iff_out_of_range(a in any::<i64>(), b in any::<i64>()) {
        let wide = i128::from(a) + i128::from(b);
        let got = sum([Paise::new(a), Paise::new(b)]);
        if i64::try_from(wide).is_ok() {
            prop_assert_eq!(got.unwrap().get(), i64::try_from(wide).unwrap());
        } else {
            prop_assert_eq!(got, Err(MoneyError::Overflow));
        }
    }
}
