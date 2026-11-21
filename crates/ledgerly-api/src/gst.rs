//! GST returns over the open company file: GSTR-1 with its exception buckets, the GSTR-1 JSON and
//! the GSTR-3B working. **Card W4** implements this module.
//!
//! # Rate pack
//!
//! The rate pack is `crates/ledgerly-gst/packs/rates-slabs.toml`, read at build time. It holds the
//! GST 2.0 slabs, and the pre-September-2025 slabs are pending. No pack in the repository has the
//! `itc.blocked.*` entries of CGST Act s.17(5) yet, so [`Api::gstr3b`] refuses any period with a
//! purchase (see [`Api::gstr3b_with_pack`]). The bundled pack is therefore the only input this
//! module reads; callers that hold another pack use the `_with_pack` methods.
//!
//! # What is refused
//!
//! * [`Api::gstr1_json`] writes JSON only when every in-period voucher is `Ready` or `NotRelevant`.
//!   A voucher in any other bucket is left out of the tables, so the JSON would be a wrong return:
//!   the answer is `Unsupported`, naming the vouchers (rule A1).
//! * [`Api::gstr3b_with_pack`] refuses a period with a purchase dated in it when the pack has no
//!   usable `itc.blocked.*` entry on that purchase's time of supply. Without them, credit under
//!   CGST Act s.17(5) cannot be ruled out, and the working would claim it.

use ledgerly_core::{Gstin, Outcome, Period, Unsupported};
use ledgerly_gst::gstr1::build_gstr1;
use ledgerly_gst::gstr3b::build_gstr3b;
use ledgerly_gst::itc::BLOCKED_RULE_IDS;
use ledgerly_model::{BaseVoucherKind, Books, VoucherStatus};

use crate::error::ApiError;
use crate::facade::Api;

pub use ledgerly_gst::gstr1::{Bucket, BucketedVoucher};
pub use ledgerly_gst::gstr3b::Gstr3bWorking;
pub use ledgerly_gst::pack::{RulePack, load_pack};
pub use ledgerly_gst_formats::gstr1::Gstr1;

/// The GST 2.0 slab pack bundled with the application.
pub(crate) const BUNDLED_RATE_PACK: &str = include_str!("../../ledgerly-gst/packs/rates-slabs.toml");

/// A GSTR-1 return with the bucket of every in-period voucher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gstr1Return {
    /// The tables. A voucher that is not `Ready` or `NotRelevant` is left out of them.
    pub gstr1: Gstr1,
    /// Every in-period voucher and its bucket, in the order the builder gives.
    pub buckets: Vec<BucketedVoucher>,
}

impl Gstr1Return {
    /// `true` when every in-period voucher is `Ready` or `NotRelevant`, so the tables hold the
    /// whole return.
    pub fn is_complete(&self) -> bool {
        self.buckets.iter().all(|b| is_settled(b.bucket))
    }
}

/// `Ready` and `NotRelevant` vouchers are settled: nothing about them is left to fix.
fn is_settled(bucket: Bucket) -> bool {
    match bucket {
        Bucket::Ready | Bucket::NotRelevant => true,
        Bucket::Mismatch | Bucket::NeedsCorrection => false,
    }
}

/// The rate pack bundled with the application.
///
/// # Errors
/// [`ApiError::Gst`] when the bundled pack does not load (a build defect).
fn bundled_pack() -> Result<RulePack, ApiError> {
    Ok(load_pack(BUNDLED_RATE_PACK)?)
}

/// Why a return is not complete, as an `Unsupported` that names the vouchers left out.
fn incomplete(ret: &Gstr1Return) -> Option<Unsupported> {
    if ret.is_complete() {
        return None;
    }
    let listed: Vec<String> = ret
        .buckets
        .iter()
        .filter(|b| !is_settled(b.bucket))
        .map(|b| {
            format!(
                "voucher {}: {}",
                b.voucher.0,
                b.reason.as_deref().unwrap_or("no reason was given")
            )
        })
        .collect();
    Some(Unsupported {
        what: "GSTR-1 JSON: the return is incomplete".to_owned(),
        reason: format!(
            "{} voucher(s) are not in the tables and must be fixed first; {}",
            listed.len(),
            listed.join("; ")
        ),
    })
}

/// The purchases dated in `period` need the s.17(5) entries of the pack. Each purchase's time of
/// supply is checked, and every entry in [`BLOCKED_RULE_IDS`] must be usable on it.
fn blocked_credit_rules(books: &Books, period: Period, pack: &RulePack) -> Outcome<()> {
    for v in books.vouchers.values() {
        let live = matches!(v.status, VoucherStatus::Regular | VoucherStatus::PostDated);
        if !live || !period.contains(v.date) {
            continue;
        }
        let is_purchase = books
            .masters
            .voucher_types
            .get(&v.type_id)
            .is_some_and(|t| t.base == BaseVoucherKind::Purchase);
        if !is_purchase {
            continue;
        }
        let tos = v.supply_date.unwrap_or(v.date);
        let rules = match pack.for_time_of_supply(tos) {
            Outcome::Value(rules) => rules,
            Outcome::Unsupported(u) => return Outcome::Unsupported(u),
        };
        for id in BLOCKED_RULE_IDS {
            if let Outcome::Unsupported(u) = rules.get(id) {
                return Outcome::unsupported(
                    format!("GSTR-3B: input tax credit on the purchase dated {tos}"),
                    format!(
                        "the rule pack has no usable entry for `{id}` ({}); credit under CGST Act s.17(5) cannot be ruled out, so no working is shown",
                        u.reason
                    ),
                );
            }
        }
    }
    Outcome::Value(())
}

impl Api {
    /// GSTR-1 for `gstin` and `period`, with the exception buckets, using the bundled rate pack.
    ///
    /// # Errors
    /// [`ApiError::Gst`] on a malformed master or arithmetic overflow, [`ApiError::Stale`] after
    /// a saved change whose reload failed.
    pub fn gstr1(&self, gstin: &Gstin, period: Period) -> Result<Outcome<Gstr1Return>, ApiError> {
        self.gstr1_with_pack(gstin, period, &bundled_pack()?)
    }

    /// As [`Api::gstr1`], with `pack` as the rule pack.
    ///
    /// # Errors
    /// As [`Api::gstr1`].
    pub fn gstr1_with_pack(
        &self,
        gstin: &Gstin,
        period: Period,
        pack: &RulePack,
    ) -> Result<Outcome<Gstr1Return>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        Ok(build_gstr1(books, gstin, period, pack)?
            .map(|(gstr1, buckets)| Gstr1Return { gstr1, buckets }))
    }

    /// The GSTR-1 JSON for `gstin` and `period`, using the bundled rate pack.
    ///
    /// The JSON is written only when the return is complete ([`Gstr1Return::is_complete`]).
    /// Otherwise the answer is `Unsupported`, naming the vouchers that must be fixed first.
    ///
    /// # Errors
    /// As [`Api::gstr1`], and [`ApiError::Format`] when the writer refuses a value.
    pub fn gstr1_json(&self, gstin: &Gstin, period: Period) -> Result<Outcome<String>, ApiError> {
        match self.gstr1(gstin, period)? {
            Outcome::Unsupported(u) => Ok(Outcome::Unsupported(u)),
            Outcome::Value(ret) => match incomplete(&ret) {
                Some(u) => Ok(Outcome::Unsupported(u)),
                None => Ok(Outcome::Value(ledgerly_gst_formats::gstr1::gstr1_to_json(
                    &ret.gstr1,
                )?)),
            },
        }
    }

    /// The GSTR-3B working for `gstin` and `period`, using the bundled rate pack.
    ///
    /// # Errors
    /// As [`Api::gstr1`].
    pub fn gstr3b(
        &self,
        gstin: &Gstin,
        period: Period,
    ) -> Result<Outcome<Gstr3bWorking>, ApiError> {
        self.gstr3b_with_pack(gstin, period, &bundled_pack()?)
    }

    /// As [`Api::gstr3b`], with `pack` as the rule pack.
    ///
    /// The working is `Unsupported` when `pack` has no usable `itc.blocked.*` entry for a purchase
    /// dated in `period` (see the module docs), and when the engine refuses a voucher.
    ///
    /// # Errors
    /// As [`Api::gstr1`].
    pub fn gstr3b_with_pack(
        &self,
        gstin: &Gstin,
        period: Period,
        pack: &RulePack,
    ) -> Result<Outcome<Gstr3bWorking>, ApiError> {
        let books = match self.cached_books()? {
            Outcome::Value(books) => books,
            Outcome::Unsupported(u) => return Ok(Outcome::Unsupported(u)),
        };
        if let Outcome::Unsupported(u) = blocked_credit_rules(books, period, pack) {
            return Ok(Outcome::Unsupported(u));
        }
        Ok(build_gstr3b(books, gstin, period, pack, None)?)
    }
}
