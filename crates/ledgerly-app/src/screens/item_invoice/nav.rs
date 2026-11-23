//! Moving between fields: Enter order, validation on leaving a field, and Escape.

use super::draft::{InvoiceRow, ParticularRow, typed_amount};
use super::picker::{ledger_hits, search_items};
use super::{Field, InvoiceMode, ItemInvoiceScreen, Step, parse_decimal, parse_place, place_label};
use crate::screens::voucher_entry::{LedgerHit, format_entry_date, parse_entry_date};
use ledgerly_model::Masters;

impl ItemInvoiceScreen {
    /// The fields in Enter order for the invoice as it stands. An accounting invoice has no
    /// sales or purchase ledger field (the sales FAQ: "The Sales ledger field will not appear"
    /// in accounting invoice mode) and has particulars where an item invoice has stock lines.
    fn order(&self) -> Vec<Field> {
        let mut order = vec![Field::Date];
        if self.draft.number_required {
            order.push(Field::Number);
        }
        match self.draft.mode {
            InvoiceMode::Item => {
                order.extend([Field::Party, Field::Place, Field::Account, Field::Reference]);
                for i in 0..self.draft.rows.len() {
                    order.extend([
                        Field::Item(i),
                        Field::Qty(i),
                        Field::Rate(i),
                        Field::Discount(i),
                    ]);
                }
            }
            InvoiceMode::Accounting => {
                order.extend([Field::Party, Field::Place, Field::Reference]);
                for i in 0..self.draft.particulars.len() {
                    order.extend([Field::Ledger(i), Field::LedgerAmount(i)]);
                }
            }
        }
        order.push(Field::Narration);
        order
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
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Ledger(_)
            | Field::LedgerAmount(_)
            | Field::Narration => field,
        };
        self.order().iter().position(|f| *f == key)
    }

    fn move_to(&mut self, field: Field) {
        self.focus = field;
        self.focus_pending = true;
        self.pick_index = 0;
    }

    pub(super) fn advance(&mut self) {
        let last_row = self.draft.rows.len().saturating_sub(1);
        if let Field::Discount(i) | Field::Amount(i) = self.focus
            && i == last_row
        {
            self.draft.rows.push(InvoiceRow::default());
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
            Field::Party => self.submit_party(masters),
            Field::Account => self.submit_account(masters),
            Field::Place => self.submit_place(),
            Field::Item(i) => self.submit_item(masters, i),
            Field::Qty(i) | Field::Rate(i) | Field::Discount(i) | Field::Amount(i) => {
                self.submit_cell(i)
            }
            Field::Ledger(i) => self.submit_particular(masters, i),
            Field::LedgerAmount(i) => self.submit_particular_amount(i),
            Field::Number | Field::Reference => Step::Next,
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
            | Field::Item(_)
            | Field::Qty(_)
            | Field::Rate(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::LedgerAmount(_)
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
        if row.is_blank() {
            if i == 0 {
                return self.refuse("Enter at least one item.".to_owned());
            }
            if i.saturating_add(1) == self.draft.rows.len() {
                self.draft.rows.pop();
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
            | Field::Item(_)
            | Field::Discount(_)
            | Field::Amount(_)
            | Field::Ledger(_)
            | Field::LedgerAmount(_)
            | Field::Narration => Ok(()),
        };
        match check {
            Ok(()) => Step::Next,
            Err(e) => self.refuse(e),
        }
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
    fn submit_particular_amount(&mut self, i: usize) -> Step {
        let Some(row) = self.draft.particulars.get(i) else {
            return Step::Stay;
        };
        match typed_amount(&row.amount_text) {
            Ok(amount) if amount.is_zero() || amount.is_negative() => {
                self.refuse("The amount must be more than zero.".to_owned())
            }
            Ok(_) => Step::Next,
            Err(e) => self.refuse(format!("Amount: {e}")),
        }
    }
}
