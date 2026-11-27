//! Card M4: store-free voucher invariants (spec section 5, invariants 1, 2 and 6).
//!
//! Provenance for every expected value below is `derived`: spec section 5 "Invariants" 1, 2 and 6
//! (docs/specs/2026-11-20-ledgerly-design.md), with the paise arithmetic written at each case.
//! Nothing here is copied from the engine's output. The `MissingParty` rule for kinds other than
//! Sales is a judgment recorded in the card report, not a spec statement, so only Sales is
//! asserted to fail and Journal is asserted to pass (spec: a Journal has no party field).

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ledgerly_core::{
        CompanyId, DrCr, GroupId, LedgerId, Paise, SignedAmount, StateCode, VoucherId,
        VoucherTypeId,
    };
    use ledgerly_model::validate::validate_voucher;
    use ledgerly_model::{
        Address, BaseVoucherKind, BillAllocation, BillRefKind, BillWiseConfig, Company, EntityKind,
        Features, Ledger, LedgerLine, Masters, NegativeStockPolicy, NumberingConfig,
        NumberingMethod, RestartPeriod, Voucher, VoucherError, VoucherMode, VoucherStatus,
        VoucherType,
    };
    use proptest::prelude::*;
    use time::macros::date;

    const SALES: VoucherTypeId = VoucherTypeId(1);
    const JOURNAL: VoucherTypeId = VoucherTypeId(2);
    const DEBTORS: LedgerId = LedgerId(10); // bill-wise
    const DEBTORS_2: LedgerId = LedgerId(11); // bill-wise
    const SALES_AC: LedgerId = LedgerId(20);
    const GST_AC: LedgerId = LedgerId(21);

    fn company() -> Company {
        Company {
            ineligible_itc_default: None,
            id: CompanyId(1),
            name: "Test Traders".to_string(),
            mailing_name: None,
            address: Address::default(),
            state: StateCode::new(27).unwrap(),
            pan: None,
            registrations: vec![],
            fy_start: date!(2026 - 04 - 01),
            books_begin: date!(2026 - 04 - 01),
            entity_kind: EntityKind::Proprietorship,
            features: Features::default(),
            negative_stock: NegativeStockPolicy::Block,
        }
    }

    fn ledger(id: LedgerId, bill_wise: bool) -> Ledger {
        Ledger {
            ineligible_itc: None,
            id,
            name: format!("L{}", id.0),
            aliases: vec![],
            group: Some(GroupId(1)),
            opening: SignedAmount::dr(Paise::ZERO),
            bill_wise: bill_wise.then_some(BillWiseConfig {
                default_credit_days: None,
            }),
            cost_centres_applicable: false,
            party: None,
            bank: None,
            tax: None,
            gst: None,
            reserved: None,
            tally: None,
        }
    }

    fn vtype(id: VoucherTypeId, base: BaseVoucherKind) -> VoucherType {
        VoucherType {
            id,
            name: format!("{base:?}"),
            base,
            abbreviation: String::new(),
            numbering: NumberingConfig {
                method: NumberingMethod::Automatic,
                prevent_renumbering: false,
                prefix: String::new(),
                suffix: String::new(),
                width: 0,
                restart: RestartPeriod::Never,
                series: vec![],
            },
            active: true,
            tally: None,
        }
    }

    fn masters() -> Masters {
        let mut m = Masters::default();
        for (id, bw) in [
            (DEBTORS, true),
            (DEBTORS_2, true),
            (SALES_AC, false),
            (GST_AC, false),
        ] {
            m.ledgers.insert(id, ledger(id, bw));
        }
        m.voucher_types
            .insert(SALES, vtype(SALES, BaseVoucherKind::Sales));
        m.voucher_types
            .insert(JOURNAL, vtype(JOURNAL, BaseVoucherKind::Journal));
        m
    }

    fn line(ledger: LedgerId, amount: SignedAmount) -> LedgerLine {
        LedgerLine {
            ineligible_itc: None,
            ledger,
            amount,
            bills: vec![],
            cost: vec![],
            bank: None,
        }
    }

    fn bill(kind: BillRefKind, name: &str, amount: SignedAmount) -> BillAllocation {
        BillAllocation {
            kind,
            name: name.to_string(),
            due_date: None,
            credit_days: None,
            amount,
        }
    }

    fn voucher(type_id: VoucherTypeId, lines: Vec<LedgerLine>) -> Voucher {
        Voucher {
            id: VoucherId(1),
            type_id,
            number: None,
            series: None,
            date: date!(2026 - 06 - 15),
            supply_date: None,
            payment_date: None,
            status: VoucherStatus::Regular,
            mode: VoucherMode::AccountingInvoice,
            party: Some(DEBTORS),
            place_of_supply: None,
            reverse_charge: false,
            reference: None,
            reference_date: None,
            narration: String::new(),
            ledger_lines: lines,
            inventory_lines: vec![],
            gst: None,
            tally: None,
            extra: BTreeMap::new(),
        }
    }

    fn p(paise: i64) -> Paise {
        Paise::new(paise)
    }

    /// Sales: Dr Debtors 1,180.00 (new bill INV-1, as Debtors is bill-wise); Cr Sales 1,000.00;
    /// Cr GST 180.00.
    fn good_sales() -> Voucher {
        let mut debtors = line(DEBTORS, SignedAmount::dr(p(118_000)));
        debtors.bills = vec![bill(
            BillRefKind::New,
            "INV-1",
            SignedAmount::dr(p(118_000)),
        )];
        voucher(
            SALES,
            vec![
                debtors,
                line(SALES_AC, SignedAmount::cr(p(100_000))),
                line(GST_AC, SignedAmount::cr(p(18_000))),
            ],
        )
    }

    #[test]
    fn balanced_voucher_passes() {
        // provenance: derived, spec section 5 invariant 1 (sum Dr = sum Cr exactly in paise);
        // Dr 118000 = Cr 100000 + Cr 18000 = 118000.
        assert_eq!(
            validate_voucher(&good_sales(), &masters(), &company()),
            Ok(())
        );
    }

    #[test]
    fn unbalanced_voucher_reports_both_totals() {
        // provenance: derived, spec section 5 invariant 1; Dr 118000 paise, Cr 100000 + 17999
        // = 117999 paise, so the error carries dr=118000 and cr=117999.
        let mut v = good_sales();
        v.ledger_lines[2].amount = SignedAmount::cr(p(17_999));
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::Unbalanced {
                dr: p(118_000),
                cr: p(117_999)
            })
        );
    }

    #[test]
    fn one_paise_difference_is_not_rounded_away() {
        // provenance: derived, invariant 1 says "exactly in paise"; Dr 100001 vs Cr 100000.
        let v = voucher(
            JOURNAL,
            vec![
                line(SALES_AC, SignedAmount::dr(p(100_001))),
                line(GST_AC, SignedAmount::cr(p(100_000))),
            ],
        );
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::Unbalanced {
                dr: p(100_001),
                cr: p(100_000)
            })
        );
    }

    #[test]
    fn bill_allocations_not_summing_to_line_are_rejected() {
        // provenance: derived, invariant 2 (bill allocations on a bill-wise line sum to the line
        // amount); line is Dr 118000, allocations 60000 + 50000 = 110000.
        let mut v = good_sales();
        v.ledger_lines[0].bills = vec![
            bill(BillRefKind::New, "INV-1", SignedAmount::dr(p(60_000))),
            bill(BillRefKind::New, "INV-2", SignedAmount::dr(p(50_000))),
        ];
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::BillAllocationMismatch {
                line: 0,
                expected: p(118_000),
                allocated: p(110_000)
            })
        );
    }

    #[test]
    fn bill_allocations_summing_to_line_pass() {
        // provenance: derived, invariant 2; 60000 + 58000 = 118000 = the line.
        let mut v = good_sales();
        v.ledger_lines[0].bills = vec![
            bill(BillRefKind::New, "INV-1", SignedAmount::dr(p(60_000))),
            bill(BillRefKind::New, "INV-2", SignedAmount::dr(p(58_000))),
        ];
        assert_eq!(validate_voucher(&v, &masters(), &company()), Ok(()));
    }

    #[test]
    fn bill_wise_line_with_no_allocations_is_a_mismatch() {
        // provenance: derived, invariant 2 read literally: a bill-wise line's allocations sum to
        // the line; no allocations sum to 0, the line is 118000. The reading is recorded in the
        // card report as a judgment.
        let mut v = good_sales();
        v.ledger_lines[0].bills = vec![];
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::BillAllocationMismatch {
                line: 0,
                expected: p(118_000),
                allocated: Paise::ZERO
            })
        );
    }

    #[test]
    fn bills_on_the_second_line_without_bill_wise_are_named_line_2() {
        // provenance: derived (S22): people count a voucher's lines from 1, so the message for
        // the line at index 1 names line 2.
        let mut v = good_sales();
        v.ledger_lines[1].bills = vec![bill(
            BillRefKind::New,
            "INV-1",
            SignedAmount::cr(p(100_000)),
        )];
        let e = validate_voucher(&v, &masters(), &company()).unwrap_err();
        assert!(e.to_string().starts_with("line 2: "), "{e}");
    }

    #[test]
    fn bills_on_a_ledger_without_bill_wise_are_rejected() {
        // provenance: derived, invariant 2 speaks of bill-wise lines; allocations on a ledger
        // that is not bill-wise have nothing to settle against, so they are an error rather than
        // silently dropped (rule A1).
        let mut v = good_sales();
        v.ledger_lines[0].bills = vec![bill(
            BillRefKind::New,
            "INV-1",
            SignedAmount::dr(p(118_000)),
        )];
        v.ledger_lines[1].bills = vec![bill(
            BillRefKind::OnAccount,
            "X",
            SignedAmount::cr(p(100_000)),
        )];
        assert!(matches!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::Invalid { .. })
        ));
    }

    #[test]
    fn voucher_before_books_begin_is_rejected() {
        // provenance: derived, invariant 6 (voucher date >= books_begin); books begin
        // 2026-04-01, voucher 2026-03-31 is one day earlier.
        let mut v = good_sales();
        v.date = date!(2026 - 03 - 31);
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::BeforeBooksBegin {
                date: date!(2026 - 03 - 31),
                books_begin: date!(2026 - 04 - 01)
            })
        );
    }

    #[test]
    fn voucher_on_books_begin_passes() {
        // provenance: derived, invariant 6 uses >=, so 2026-04-01 itself is allowed.
        let mut v = good_sales();
        v.date = date!(2026 - 04 - 01);
        assert_eq!(validate_voucher(&v, &masters(), &company()), Ok(()));
    }

    #[test]
    fn against_reference_on_another_ledger_is_rejected() {
        // provenance: derived, invariant 2 (Against refs must be on the same ledger). Within the
        // voucher: bill "INV-9" is opened (New) on DEBTORS and settled (Against) on DEBTORS_2.
        // The store-dependent half (existing bills of other vouchers) belongs to the engine.
        let v = voucher(
            JOURNAL,
            vec![
                {
                    let mut l = line(DEBTORS, SignedAmount::dr(p(50_000)));
                    l.bills = vec![bill(BillRefKind::New, "INV-9", SignedAmount::dr(p(50_000)))];
                    l
                },
                {
                    let mut l = line(DEBTORS_2, SignedAmount::cr(p(50_000)));
                    l.bills = vec![bill(
                        BillRefKind::Against,
                        "INV-9",
                        SignedAmount::cr(p(50_000)),
                    )];
                    l
                },
            ],
        );
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::BillOnWrongLedger {
                bill: "INV-9".to_string(),
                ledger: DEBTORS_2
            })
        );
    }

    #[test]
    fn against_reference_on_the_same_ledger_passes() {
        // provenance: derived, invariant 2; New and Against of "INV-9" both on DEBTORS, each line
        // sums to its bill (50000 / 50000).
        let v = voucher(
            JOURNAL,
            vec![
                {
                    let mut l = line(DEBTORS, SignedAmount::dr(p(50_000)));
                    l.bills = vec![bill(BillRefKind::New, "INV-9", SignedAmount::dr(p(50_000)))];
                    l
                },
                {
                    let mut l = line(DEBTORS, SignedAmount::cr(p(50_000)));
                    l.bills = vec![bill(
                        BillRefKind::Against,
                        "INV-9",
                        SignedAmount::cr(p(50_000)),
                    )];
                    l
                },
            ],
        );
        assert_eq!(validate_voucher(&v, &masters(), &company()), Ok(()));
    }

    #[test]
    fn sales_voucher_without_party_is_rejected() {
        // provenance: derived, card M4 and the error's own text ("this voucher kind needs a
        // party"); a Sales invoice is addressed to a party.
        let mut v = good_sales();
        v.party = None;
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::MissingParty)
        );
    }

    #[test]
    fn journal_without_party_passes() {
        // provenance: derived, a Journal posts Dr/Cr lines between ledgers and has no party.
        let mut v = voucher(
            JOURNAL,
            vec![
                line(SALES_AC, SignedAmount::dr(p(500))),
                line(GST_AC, SignedAmount::cr(p(500))),
            ],
        );
        v.party = None;
        assert_eq!(validate_voucher(&v, &masters(), &company()), Ok(()));
    }

    #[test]
    fn unknown_ledger_is_rejected() {
        // provenance: derived, ledger 999 is not in the masters.
        let mut v = good_sales();
        v.ledger_lines[1].ledger = LedgerId(999);
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::UnknownLedger(LedgerId(999)))
        );
    }

    #[test]
    fn unknown_party_is_rejected() {
        // provenance: derived, party ledger 998 is not in the masters.
        let mut v = good_sales();
        v.party = Some(LedgerId(998));
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::UnknownLedger(LedgerId(998)))
        );
    }

    #[test]
    fn unknown_voucher_type_is_rejected() {
        // provenance: derived, voucher type 77 is not in the masters.
        let v = voucher(VoucherTypeId(77), vec![]);
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::UnknownVoucherType(VoucherTypeId(77)))
        );
    }

    #[test]
    fn voucher_with_no_lines_is_rejected() {
        // provenance: derived, a regular voucher with neither ledger nor inventory lines carries
        // nothing to post.
        let v = voucher(JOURNAL, vec![]);
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::Empty)
        );
    }

    #[test]
    fn negative_line_amount_is_rejected() {
        // provenance: derived, SignedAmount is "a non-negative amount with its side"
        // (ledgerly-core); line 1 is -5 paise.
        let mut v = good_sales();
        v.ledger_lines[1].amount = SignedAmount {
            amount: p(-5),
            side: DrCr::Cr,
        };
        assert_eq!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::NegativeAmount { line: 1 })
        );
    }

    #[test]
    fn total_overflow_is_an_error_not_a_wrap() {
        // provenance: derived, i64::MAX + 1 does not fit in i64 (review focus 1).
        let v = voucher(
            JOURNAL,
            vec![
                line(SALES_AC, SignedAmount::dr(p(i64::MAX))),
                line(GST_AC, SignedAmount::dr(p(1))),
                line(GST_AC, SignedAmount::cr(p(i64::MAX))),
            ],
        );
        assert!(matches!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::Money(_))
        ));
    }

    #[test]
    fn cancelled_voucher_with_no_lines_passes() {
        // provenance: derived, spec section 5: Cancelled is "number kept, no amounts".
        let mut v = voucher(JOURNAL, vec![]);
        v.status = VoucherStatus::Cancelled;
        assert_eq!(validate_voucher(&v, &masters(), &company()), Ok(()));
    }

    #[test]
    fn cancelled_voucher_with_amounts_is_rejected() {
        // provenance: derived, spec section 5: Cancelled carries no amounts.
        let mut v = good_sales();
        v.status = VoucherStatus::Cancelled;
        assert!(matches!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::Invalid { .. })
        ));
    }

    #[test]
    fn cancelled_voucher_before_books_begin_is_still_rejected() {
        // provenance: derived, invariant 6 applies to every voucher date.
        let mut v = voucher(JOURNAL, vec![]);
        v.status = VoucherStatus::Cancelled;
        v.date = date!(2026 - 03 - 01);
        assert!(matches!(
            validate_voucher(&v, &masters(), &company()),
            Err(VoucherError::BeforeBooksBegin { .. })
        ));
    }

    proptest! {
        #[test]
        fn constructed_balanced_vouchers_pass(debits in proptest::collection::vec(0i64..1_000_000_000, 1..8)) {
            // provenance: derived by construction: the single credit equals the sum of the
            // debits, so sum Dr = sum Cr (invariant 1).
            let total: i64 = debits.iter().sum();
            let mut lines: Vec<LedgerLine> = debits
                .iter()
                .map(|d| line(SALES_AC, SignedAmount::dr(p(*d))))
                .collect();
            lines.push(line(GST_AC, SignedAmount::cr(p(total))));
            let v = voucher(JOURNAL, lines);
            prop_assert_eq!(validate_voucher(&v, &masters(), &company()), Ok(()));
        }

        #[test]
        fn constructed_unbalanced_vouchers_fail_with_exact_totals(
            debits in proptest::collection::vec(0i64..1_000_000_000, 1..8),
            skew in 1i64..1_000,
        ) {
            // provenance: derived by construction: credit = sum Dr + skew, skew >= 1, so the
            // totals differ and the error carries exactly dr = sum, cr = sum + skew.
            let total: i64 = debits.iter().sum();
            let mut lines: Vec<LedgerLine> = debits
                .iter()
                .map(|d| line(SALES_AC, SignedAmount::dr(p(*d))))
                .collect();
            lines.push(line(GST_AC, SignedAmount::cr(p(total + skew))));
            let v = voucher(JOURNAL, lines);
            prop_assert_eq!(
                validate_voucher(&v, &masters(), &company()),
                Err(VoucherError::Unbalanced { dr: p(total), cr: p(total + skew) })
            );
        }
    }
}
