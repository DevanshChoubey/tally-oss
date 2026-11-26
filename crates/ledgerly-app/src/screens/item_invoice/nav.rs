//! Moving between fields: Enter order, validation on leaving a field, and Escape.

use super::draft::{BillRow, InvoiceRow, ParticularRow, typed_amount};
use super::picker::{ledger_hits, search_items};
use super::{
    Field, InvoiceKind, InvoiceMode, ItemInvoiceScreen, Step, parse_decimal, parse_place,
    place_label,
};
use crate::screens::voucher_entry::{LedgerHit, format_entry_date, parse_entry_date};
use ledgerly_api::is_gst_tax_ledger;
use ledgerly_model::Masters;

impl ItemInvoiceScreen {
    /// The fields in Enter order for the invoice as it stands. An accounting invoice has no
    /// sales or purchase ledger field (the sales FAQ: "The Sales ledger field will not appear"
    /// in accounting invoice mode) and has particulars where an item invoice has stock lines.
    ///
    /// The order of the head follows the accounting product's help pages (read 2026-11-25): a
    /// purchase asks the supplier's invoice number and date first, then the party A/c name, then
    /// the purchase ledger (help.tallysolutions.com/purchase-goods-services/); a credit or debit
    /// note asks the party first, then the original invoice details, then the sales ledger
    /// (help.tallysolutions.com/tally-prime/sales-process/sales-return-tally/ and
    /// help.tallysolutions.com/tally-prime/purchase-process/purchase-returns-tally/). Those pages
    /// show no date in the voucher body (the date is the screen's date, F2), so a purchase or a
    /// note has no Date step: the Date box stays on the sheet for a click. A sale keeps its order.
    fn order(&self) -> Vec<Field> {
        let item = self.draft.mode == InvoiceMode::Item;
        let dated = self.draft.kind.reference_date_label().is_some();
        let mut head = match self.draft.kind {
            InvoiceKind::Sales => vec![Field::Date],
            InvoiceKind::Purchase | InvoiceKind::CreditNote | InvoiceKind::DebitNote => Vec::new(),
        };
        if self.draft.number_required {
            head.push(Field::Number);
        }
        let reference = |out: &mut Vec<Field>| {
            out.push(Field::Reference);
            if dated {
                out.push(Field::ReferenceDate);
            }
        };
        match self.draft.kind {
            InvoiceKind::Sales => {
                head.extend([Field::Party, Field::Place]);
                if item {
                    head.push(Field::Account);
                }
                reference(&mut head);
            }
            InvoiceKind::Purchase => {
                reference(&mut head);
                head.push(Field::Party);
                if item {
                    head.push(Field::Account);
                }
                head.push(Field::Place);
            }
            InvoiceKind::CreditNote | InvoiceKind::DebitNote => {
                head.push(Field::Party);
                reference(&mut head);
                if item {
                    head.push(Field::Account);
                }
                head.push(Field::Place);
            }
        }
        let mut order = head;
        order.extend(self.bill_fields());
        match self.draft.mode {
            InvoiceMode::Item => {
                let discount = self.discount_columns();
                for i in 0..self.draft.rows.len() {
                    order.extend([Field::Item(i), Field::Qty(i), Field::Rate(i)]);
                    if discount {
                        order.push(Field::Discount(i));
                    }
                }
            }
            InvoiceMode::Accounting | InvoiceMode::AsVoucher => {
                for i in 0..self.draft.particulars.len() {
                    order.extend([Field::Ledger(i), Field::LedgerAmount(i)]);
                }
            }
        }
        order.push(Field::Narration);
        order
    }

    /// The field the keyboard starts on: the first step of the Enter order, the party on a sale.
    pub(super) fn first_field(&self) -> Field {
        match self.draft.kind {
            InvoiceKind::Sales | InvoiceKind::CreditNote | InvoiceKind::DebitNote => Field::Party,
            InvoiceKind::Purchase => Field::Reference,
        }
    }

    /// The bill reference fields in Enter order: none unless More details is open on a party that
    /// tracks bills (it holds rows only then).
    fn bill_fields(&self) -> Vec<Field> {
        if !self.more_open {
            return Vec::new();
        }
        (0..self.draft.bills.len())
            .flat_map(|i| [Field::BillKind(i), Field::BillName(i), Field::BillAmount(i)])
            .collect()
    }

    /// Where `field` sits in the Enter order (a typed amount counts as its line's discount).
    fn position(&self, field: Field) -> Option<usize> {
        let key = match field {
            Field::Amount(i) => Field::Discount(i),
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Account
            | Field::Reference
            | Field::ReferenceDate
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Ledger(_)
            | Field::LedgerAmount(_)
            | Field::BillKind(_)
            | Field::BillName(_)
            | Field::BillAmount(_)
            | Field::Narration => field,
        };
        self.order().iter().position(|f| *f == key)
    }

    pub(super) fn move_to(&mut self, field: Field) {
        self.focus = field;
        self.focus_pending = true;
        self.pick_index = 0;
        self.list_open = false;
    }

    pub(super) fn advance(&mut self) {
        let last_row = self.draft.rows.len().saturating_sub(1);
        let row_end = |i: usize| i == last_row;
        match self.focus {
            Field::Discount(i) | Field::Amount(i) if row_end(i) => {
                self.draft.rows.push(InvoiceRow::default());
            }
            Field::Rate(i) if row_end(i) && !self.discount_columns() => {
                self.draft.rows.push(InvoiceRow::default());
            }
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Account
            | Field::Reference
            | Field::ReferenceDate
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Ledger(_)
            | Field::LedgerAmount(_)
            | Field::BillKind(_)
            | Field::BillName(_)
            | Field::BillAmount(_)
            | Field::Narration => {}
        }
        if let Field::LedgerAmount(i) = self.focus
            && i == self.draft.particulars.len().saturating_sub(1)
        {
            self.draft.particulars.push(ParticularRow::default());
        }
        let order = self.order();
        let next = self
            .position(self.focus)
            .and_then(|p| order.get(p.saturating_add(1)));
        if let Some(next) = next {
            self.move_to(*next);
        }
    }

    /// Shift+Enter: back one field.
    pub fn step_back(&mut self) {
        let order = self.order();
        let prev = self
            .position(self.focus)
            .and_then(|p| p.checked_sub(1))
            .and_then(|p| order.get(p));
        if let Some(prev) = prev {
            self.move_to(*prev);
        }
    }

    /// Enter: checks the field that has the keyboard and moves on.
    pub fn submit(&mut self, masters: &Masters) {
        self.notice = None;
        let step = match self.focus {
            Field::Date => self.submit_date(),
            Field::ReferenceDate => self.submit_reference_date(),
            Field::Party => self.submit_party(masters),
            Field::Account => self.submit_account(masters),
            Field::Place => self.submit_place(),
            Field::Item(i) => self.submit_item(masters, i),
            Field::Qty(i) | Field::Rate(i) | Field::Discount(i) | Field::Amount(i) => {
                self.submit_cell(i)
            }
            Field::Ledger(i) => self.submit_particular(masters, i),
            Field::LedgerAmount(i) => self.submit_particular_amount(masters, i),
            Field::Number | Field::Reference | Field::BillName(_) => Step::Next,
            Field::BillKind(i) => self.submit_bill_kind(i),
            Field::BillAmount(i) => self.submit_bill_amount(i),
            Field::Narration => {
                self.ask_save(masters);
                Step::Stay
            }
        };
        match step {
            Step::Next => self.advance(),
            Step::Goto(field) => self.move_to(field),
            Step::Stay => {}
        }
    }

    /// Enter on a party or ledger field whose text names no ledger: says the ledger is still being
    /// created when it was asked for, or how to create it.
    fn no_ledger_match(&mut self) -> Step {
        let text = match self.focus {
            Field::Account => self.draft.account_text.clone(),
            Field::Ledger(i) => self
                .draft
                .particulars
                .get(i)
                .map_or_else(String::new, |r| r.ledger_text.clone()),
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Reference
            | Field::ReferenceDate
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::LedgerAmount(_)
            | Field::BillKind(_)
            | Field::BillName(_)
            | Field::BillAmount(_)
            | Field::Narration => self.draft.party_text.clone(),
        };
        if self.is_waiting_for(&text) {
            return self.refuse(format!("Ledger '{}' is still being created.", text.trim()));
        }
        self.refuse("No ledger matches; press Alt+C to create one.".to_owned())
    }

    fn refuse(&mut self, message: String) -> Step {
        self.fail(message);
        Step::Stay
    }

    fn submit_date(&mut self) -> Step {
        match parse_entry_date(&self.draft.date_text, self.fy) {
            Ok(date) => {
                self.draft.date_text = format_entry_date(date);
                match self.draft.kind {
                    InvoiceKind::Sales => Step::Next,
                    // No Date step for these: Enter on the clicked box goes on to the head's start.
                    InvoiceKind::Purchase | InvoiceKind::CreditNote | InvoiceKind::DebitNote => {
                        Step::Goto(self.first_field())
                    }
                }
            }
            Err(e) => self.refuse(e.to_string()),
        }
    }

    fn submit_reference_date(&mut self) -> Step {
        match self.draft.reference_date(self.fy) {
            Ok(date) => {
                if let Some(date) = date {
                    self.draft.reference_date_text = format_entry_date(date);
                }
                Step::Next
            }
            Err(e) => self.refuse(e.to_string()),
        }
    }

    fn submit_place(&mut self) -> Step {
        match parse_place(&self.draft.place_text) {
            Ok(state) => {
                self.draft.place_text = place_label(state);
                Step::Next
            }
            Err(e) => self.refuse(e.to_string()),
        }
    }

    fn picked_ledger(&self, masters: &Masters, query: &str) -> Option<LedgerHit> {
        ledger_hits(masters, query, &self.recents, &self.balances)
            .into_iter()
            .nth(self.pick_index)
    }

    fn submit_party(&mut self, masters: &Masters) -> Step {
        if self.draft.party.is_some() {
            return Step::Next;
        }
        match self.picked_ledger(masters, &self.draft.party_text) {
            Some(hit) => {
                self.draft.party = Some(hit.id);
                self.draft.party_text = hit.name;
                Step::Next
            }
            None => self.no_ledger_match(),
        }
    }

    fn submit_account(&mut self, masters: &Masters) -> Step {
        if self.draft.account.is_some() {
            return Step::Next;
        }
        match self.picked_ledger(masters, &self.draft.account_text) {
            Some(hit) => {
                self.draft.account = Some(hit.id);
                self.draft.account_text = hit.name;
                Step::Next
            }
            None => self.no_ledger_match(),
        }
    }

    fn submit_item(&mut self, masters: &Masters, i: usize) -> Step {
        let Some(row) = self.draft.rows.get(i) else {
            return Step::Stay;
        };
        if row.item.is_some() {
            return Step::Next;
        }
        if row.is_blank() && !self.list_open {
            if i == 0 {
                return self.refuse("Enter at least one item.".to_owned());
            }
            // On a sales invoice or purchase bill the empty entry row stays at the end of the
            // list (the mock always draws one); `build_voucher` skips it, so what is saved does
            // not change. The notes' states draw only their lines, so a note drops it.
            match self.draft.kind {
                InvoiceKind::Sales | InvoiceKind::Purchase => {}
                InvoiceKind::CreditNote | InvoiceKind::DebitNote => {
                    if i.saturating_add(1) == self.draft.rows.len() {
                        self.draft.rows.pop();
                    }
                }
            }
            return Step::Goto(Field::Narration);
        }
        let hit = search_items(masters, &row.item_text)
            .into_iter()
            .nth(self.pick_index);
        let Some(hit) = hit else {
            return self.refuse("No item matches; create it on the masters screen.".to_owned());
        };
        if let Some(row) = self.draft.rows.get_mut(i) {
            row.item = Some(hit.id);
            row.item_text = hit.name;
        }
        Step::Next
    }

    fn submit_cell(&mut self, i: usize) -> Step {
        let Some(row) = self.draft.rows.get(i) else {
            return Step::Stay;
        };
        let check = match self.focus {
            Field::Qty(_) => parse_decimal(&row.qty_text).map(|_| ()),
            Field::Rate(_) => parse_decimal(&row.rate_text).map(|_| ()),
            Field::Discount(_) if !row.discount_text.trim().is_empty() => {
                parse_decimal(&row.discount_text).map(|_| ())
            }
            Field::Amount(_) if !row.amount_text.trim().is_empty() => {
                typed_amount(&row.amount_text).map(|_| ())
            }
            Field::Date
            | Field::Number
            | Field::Party
            | Field::Place
            | Field::Account
            | Field::Reference
            | Field::ReferenceDate
            | Field::Item(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Ledger(_)
            | Field::LedgerAmount(_)
            | Field::BillKind(_)
            | Field::BillName(_)
            | Field::BillAmount(_)
            | Field::Narration => Ok(()),
        };
        match check {
            Ok(()) => Step::Next,
            Err(e) => self.refuse(e),
        }
    }

    /// Enter on a bill reference's type: it must be empty or one of the four types, which is
    /// written out in full.
    fn submit_bill_kind(&mut self, i: usize) -> Step {
        let Some(row) = self.draft.bills.get_mut(i) else {
            return Step::Stay;
        };
        if row.kind_text.trim().is_empty() {
            return Step::Next;
        }
        match BillRow::parse_kind(&row.kind_text) {
            Some(kind) => {
                row.kind_text.clear();
                row.kind_text.push_str(BillRow::kind_label(kind));
                Step::Next
            }
            None => self.refuse("The type is New Ref, Agst Ref, Advance or On Account.".to_owned()),
        }
    }

    /// Enter on a bill reference's amount: it must be empty or a plain amount; on the last
    /// reference a typed one opens the next row.
    fn submit_bill_amount(&mut self, i: usize) -> Step {
        let Some(row) = self.draft.bills.get(i) else {
            return Step::Stay;
        };
        if !row.amount_text.trim().is_empty()
            && let Err(e) = typed_amount(&row.amount_text)
        {
            return self.refuse(format!("Amount: {e}"));
        }
        let typed = !row.is_blank();
        if typed && i.saturating_add(1) == self.draft.bills.len() {
            self.draft.bills.push(BillRow::default());
        }
        Step::Next
    }

    /// Enter on a particular's ledger: picks the ledger the list shows. A blank first particular
    /// is refused; a blank later one ends the particulars and goes to the narration.
    fn submit_particular(&mut self, masters: &Masters, i: usize) -> Step {
        let Some(row) = self.draft.particulars.get(i) else {
            return Step::Stay;
        };
        if row.ledger.is_some() {
            return Step::Next;
        }
        if row.is_blank() {
            if i == 0 {
                return self.refuse("Enter at least one ledger.".to_owned());
            }
            if i.saturating_add(1) == self.draft.particulars.len() {
                self.draft.particulars.pop();
            }
            return Step::Goto(Field::Narration);
        }
        let query = row.ledger_text.clone();
        match self.picked_ledger(masters, &query) {
            Some(hit) => {
                if let Some(row) = self.draft.particulars.get_mut(i) {
                    row.ledger = Some(hit.id);
                    row.ledger_text = hit.name;
                }
                Step::Next
            }
            None => self.no_ledger_match(),
        }
    }

    /// Enter on a particular's amount: it must be a plain amount in paise.
    fn submit_particular_amount(&mut self, masters: &Masters, i: usize) -> Step {
        let Some(row) = self.draft.particulars.get(i) else {
            return Step::Stay;
        };
        // As voucher: a GST ledger line may stay empty, the engine fills it.
        if self.draft.mode == InvoiceMode::AsVoucher
            && row.amount_text.trim().is_empty()
            && row.ledger.is_some_and(|l| is_gst_tax_ledger(masters, l))
        {
            return Step::Next;
        }
        match typed_amount(&row.amount_text) {
            Ok(amount) if amount.is_zero() || amount.is_negative() => {
                self.refuse("The amount must be more than zero.".to_owned())
            }
            Ok(_) => Step::Next,
            Err(e) => self.refuse(format!("Amount: {e}")),
        }
    }
}
