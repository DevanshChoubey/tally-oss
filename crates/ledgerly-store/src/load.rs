//! Loading [`Books`] from the store (**card S4**, spec 6.1).
//!
//! The append-only event log is the source of truth (spec 6.1, 6.6), so [`load_books`] replays it
//! in sequence order into the company's masters and vouchers and derives the postings from the
//! vouchers. The `masters`, `vouchers` and `postings` projection tables of `0001_init.sql` are not
//! read: nothing in this wave fills them (`commit` appends events only), and a loader that read an
//! unfilled projection would return empty books, which is a wrong number. Card S6 compares a
//! rebuild against those tables once something maintains them.
//!
//! Replay rules, all derived from the doc comments of [`Event`] and of the model types:
//! - `MasterCreated`/`VoucherPosted` insert; `MasterAltered`/`VoucherAltered` replace the stored
//!   item with `after`; `MasterDeleted`/`VoucherDeleted` remove it. The ids named by an event must
//!   agree with the stored state (create of an existing id, alter or delete of a missing one, an
//!   alter that changes the id) or the load fails with [`StoreError::Encoding`] naming the event
//!   sequence number. The log is never "repaired".
//! - `VoucherCancelled` keeps the stored voucher (number, date, party, narration) with status
//!   [`VoucherStatus::Cancelled`] and removes its amounts: ledger lines, inventory lines and
//!   stored GST are cleared ("number kept, amounts removed", [`Event::VoucherCancelled`]).
//! - `MatchConfirmed` sets the bank date of every bank line of each matched voucher (spec 6.5:
//!   only user-confirmed matches set the bank date; [`BankMatch::bank_date`]).
//! - Settings, period locks, statement imports and assistant records do not change [`Books`].
//! - A voucher with status other than `Cancelled` gives one [`Posting`] per ledger line, carrying
//!   the voucher's status so scenarios (card E6) can filter. A cancelled voucher stays in
//!   `Books::vouchers` and has no postings. `postings` are sorted by `(date, voucher)`; lines of
//!   one voucher keep their line order.
//! - `period` keeps only vouchers (and their postings) dated inside it; masters are always all
//!   loaded.
//!
//! `Books::stock_moves` is filled from the same vouchers by card S5's projection
//! ([`crate::stock_load::moves_from_vouchers`]). A voucher that cannot be projected to stock
//! movements fails the load with [`StoreError::Encoding`] naming it: its stock is never left out,
//! because an empty or partial list would understate stock and skip the stock guards in reports.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt::Debug;

use ledgerly_core::{EventSeq, Period, VoucherId};
use ledgerly_model::{BankMatch, Books, MasterSnapshot, Masters, Posting, Voucher, VoucherStatus};

use crate::commit::read_events;
use crate::{AuditRecord, Event, SqliteStore, StoreError};

/// Records read from the log per query, so memory stays bounded by the books, not the log.
const PAGE: usize = 1000;

/// See [`crate::Store::load_books`].
///
/// # Errors
/// [`StoreError`] on a read failure; [`StoreError::Encoding`] when an event cannot be decoded or
/// does not fit the state built from the events before it (see the module documentation).
pub fn load_books(store: &SqliteStore, period: Option<Period>) -> Result<Books, StoreError> {
    let company = store.company()?;
    let mut state = Replay::default();
    let mut from = EventSeq(1);
    loop {
        let page = read_events(store, from, PAGE)?;
        let count = page.len();
        let Some(last_seq) = page.last().map(|r| r.seq) else {
            break;
        };
        let next = last_seq
            .0
            .checked_add(1)
            .ok_or_else(|| StoreError::Encoding("event sequence number overflow".to_owned()))?;
        for record in page {
            state.apply_record(record)?;
        }
        if count < PAGE {
            break;
        }
        from = EventSeq(next);
    }
    state.into_books(company, period)
}

/// Masters and vouchers rebuilt from the events read so far.
#[derive(Default)]
struct Replay {
    masters: Masters,
    vouchers: BTreeMap<VoucherId, Voucher>,
}

impl Replay {
    fn apply_record(&mut self, record: AuditRecord) -> Result<(), StoreError> {
        let seq = record.seq.0;
        self.apply(record.event).map_err(|message| {
            StoreError::Encoding(format!("event log replay failed at event {seq}: {message}"))
        })
    }

    fn apply(&mut self, event: Event) -> Result<(), String> {
        match event {
            Event::MasterCreated(snapshot) => apply_master(&mut self.masters, snapshot, Op::Insert),
            Event::MasterAltered { before, after } => {
                if master_key(&before) != master_key(&after) {
                    return Err(format!(
                        "master alter changes identity from {:?} to {:?}",
                        master_key(&before),
                        master_key(&after)
                    ));
                }
                apply_master(&mut self.masters, after, Op::Replace)
            }
            Event::MasterDeleted(snapshot) => apply_master(&mut self.masters, snapshot, Op::Remove),
            Event::VoucherPosted(voucher) => {
                let id = voucher.id;
                Op::Insert.apply(&mut self.vouchers, id, voucher, "voucher")
            }
            Event::VoucherAltered { before, after } => {
                if before.id != after.id {
                    return Err(format!(
                        "voucher alter changes id from {:?} to {:?}",
                        before.id, after.id
                    ));
                }
                let id = after.id;
                Op::Replace.apply(&mut self.vouchers, id, after, "voucher")
            }
            Event::VoucherCancelled { before } => self.cancel(before.id),
            Event::VoucherDeleted { before } => {
                let id = before.id;
                Op::Remove.apply(&mut self.vouchers, id, before, "voucher")
            }
            Event::MatchConfirmed(matched) => self.confirm_match(&matched),
            Event::SettingChanged { .. }
            | Event::PeriodLocked { .. }
            | Event::StatementImported(_)
            | Event::AiProposalRecorded(_) => Ok(()),
        }
    }

    fn cancel(&mut self, id: VoucherId) -> Result<(), String> {
        let voucher = self
            .vouchers
            .get_mut(&id)
            .ok_or_else(|| format!("cancel of voucher {id:?} which does not exist"))?;
        if voucher.status == VoucherStatus::Cancelled {
            return Err(format!("voucher {id:?} is already cancelled"));
        }
        voucher.status = VoucherStatus::Cancelled;
        voucher.ledger_lines.clear();
        voucher.inventory_lines.clear();
        voucher.gst = None;
        Ok(())
    }

    fn confirm_match(&mut self, matched: &BankMatch) -> Result<(), String> {
        for id in &matched.vouchers {
            let voucher = self
                .vouchers
                .get_mut(id)
                .ok_or_else(|| format!("bank match names voucher {id:?} which does not exist"))?;
            let mut bank_lines = 0_usize;
            for allocation in voucher
                .ledger_lines
                .iter_mut()
                .filter_map(|line| line.bank.as_mut())
            {
                allocation.bank_date = Some(matched.bank_date);
                bank_lines = bank_lines.saturating_add(1);
            }
            if bank_lines == 0 {
                return Err(format!(
                    "bank match names voucher {id:?} which has no bank line"
                ));
            }
        }
        Ok(())
    }

    fn into_books(
        self,
        company: ledgerly_model::Company,
        period: Option<Period>,
    ) -> Result<Books, StoreError> {
        let mut vouchers = self.vouchers;
        if let Some(p) = period {
            vouchers.retain(|_, v| p.contains(v.date));
        }
        let mut postings = Vec::new();
        for voucher in vouchers.values() {
            postings_of(voucher, &mut postings).map_err(StoreError::Encoding)?;
        }
        // Stable sort: the lines of one voucher keep their line order.
        postings.sort_by_key(|p| (p.date, p.voucher));
        // The same vouchers as the postings, so `period` limits both alike.
        let stock_moves = crate::stock_load::moves_from_vouchers(&self.masters, &vouchers)?;
        Ok(Books {
            company,
            masters: self.masters,
            vouchers,
            postings,
            stock_moves,
        })
    }
}

/// Appends the postings of `voucher`: none when it is cancelled, otherwise one per ledger line.
fn postings_of(voucher: &Voucher, out: &mut Vec<Posting>) -> Result<(), String> {
    match voucher.status {
        VoucherStatus::Cancelled => return Ok(()),
        VoucherStatus::Regular | VoucherStatus::Optional | VoucherStatus::PostDated => {}
    }
    for line in &voucher.ledger_lines {
        if line.amount.amount.is_negative() {
            return Err(format!(
                "voucher {:?} has a ledger line with a negative amount",
                voucher.id
            ));
        }
        out.push(Posting {
            voucher: voucher.id,
            date: voucher.date,
            ledger: line.ledger,
            amount: line.amount,
            status: voucher.status,
            type_id: voucher.type_id,
        });
    }
    Ok(())
}

/// What an event does to one stored item.
#[derive(Clone, Copy)]
enum Op {
    /// The item must not exist yet.
    Insert,
    /// The item must exist; it is replaced.
    Replace,
    /// The item must exist; it is removed.
    Remove,
}

impl Op {
    fn apply<K: Ord + Copy + Debug, V>(
        self,
        map: &mut BTreeMap<K, V>,
        id: K,
        value: V,
        what: &str,
    ) -> Result<(), String> {
        match self {
            Op::Insert => match map.entry(id) {
                Entry::Occupied(_) => Err(format!("{what} {id:?} is created twice")),
                Entry::Vacant(slot) => {
                    slot.insert(value);
                    Ok(())
                }
            },
            Op::Replace => match map.get_mut(&id) {
                Some(slot) => {
                    *slot = value;
                    Ok(())
                }
                None => Err(format!("alter of {what} {id:?} which does not exist")),
            },
            Op::Remove => map
                .remove(&id)
                .map(|_| ())
                .ok_or_else(|| format!("delete of {what} {id:?} which does not exist")),
        }
    }
}

fn apply_master(masters: &mut Masters, snapshot: MasterSnapshot, op: Op) -> Result<(), String> {
    match snapshot {
        MasterSnapshot::Group(v) => op.apply(&mut masters.groups, v.id, v, "group"),
        MasterSnapshot::Ledger(v) => op.apply(&mut masters.ledgers, v.id, v, "ledger"),
        MasterSnapshot::VoucherType(v) => {
            op.apply(&mut masters.voucher_types, v.id, v, "voucher type")
        }
        MasterSnapshot::Unit(v) => op.apply(&mut masters.units, v.id, v, "unit"),
        MasterSnapshot::StockGroup(v) => {
            op.apply(&mut masters.stock_groups, v.id, v, "stock group")
        }
        MasterSnapshot::StockItem(v) => op.apply(&mut masters.stock_items, v.id, v, "stock item"),
        MasterSnapshot::Godown(v) => op.apply(&mut masters.godowns, v.id, v, "godown"),
        MasterSnapshot::Batch(v) => op.apply(&mut masters.batches, v.id, v, "batch"),
        MasterSnapshot::CostCategory(v) => {
            op.apply(&mut masters.cost_categories, v.id, v, "cost category")
        }
        MasterSnapshot::CostCentre(v) => {
            op.apply(&mut masters.cost_centres, v.id, v, "cost centre")
        }
    }
}

/// Kind and raw id of a master, to check that an alter keeps the identity.
fn master_key(snapshot: &MasterSnapshot) -> (&'static str, u64) {
    match snapshot {
        MasterSnapshot::Group(v) => ("group", v.id.0),
        MasterSnapshot::Ledger(v) => ("ledger", v.id.0),
        MasterSnapshot::VoucherType(v) => ("voucher type", v.id.0),
        MasterSnapshot::Unit(v) => ("unit", v.id.0),
        MasterSnapshot::StockGroup(v) => ("stock group", v.id.0),
        MasterSnapshot::StockItem(v) => ("stock item", v.id.0),
        MasterSnapshot::Godown(v) => ("godown", v.id.0),
        MasterSnapshot::Batch(v) => ("batch", v.id.0),
        MasterSnapshot::CostCategory(v) => ("cost category", v.id.0),
        MasterSnapshot::CostCentre(v) => ("cost centre", v.id.0),
    }
}
