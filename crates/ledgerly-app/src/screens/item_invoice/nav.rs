//! Moving between fields: Enter order, validation on leaving a field, and Escape.

use super::draft::{InvoiceRow, typed_amount};
use super::picker::{ledger_hits, search_items};
use super::{Field, ItemInvoiceScreen, Step, parse_decimal, parse_place, place_label};
use crate::screens::voucher_entry::{LedgerHit, format_entry_date, parse_entry_date};
use ledgerly_model::Masters;

impl ItemInvoiceScreen {
    /// The fields in Enter order for the invoice as it stands.
    fn order(&self) -> Vec<Field> {
        let mut order = vec![Field::Date];
        if self.draft.number_required {
            order.push(Field::Number);
        }
        order.extend([Field::Party, Field::Place, Field::Account, Field::Reference]);
        for i in 0..self.draft.rows.len() {
            order.extend([
                Field::Item(i),
                Field::Qty(i),
                Field::Rate(i),
                Field::Discount(i),
            ]);
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
            | Field::Narration => field,
        };
        self.order().iter().position(|f| *f == key)
    }

    fn move_to(&mut self, field: Field) {
        self.focus = field;
        self.focus_pending = true;
        self.pick_index = 0;
    }

    fn advance(&mut self) {
        let last_row = self.draft.rows.len().saturating_sub(1);
        if let Field::Discount(i) | Field::Amount(i) = self.focus
            && i == last_row
        {
            self.draft.rows.push(InvoiceRow::default());
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
            Field::Number | Field::Reference => Step::Next,
            Field::Narration => {
                self.save(masters);
                Step::Stay
            }
        };
        match step {
            Step::Next => self.advance(),
            Step::Goto(field) => self.move_to(field),
            Step::Stay => {}
        }
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
            None => self.refuse("No ledger matches; create it on the masters screen.".to_owned()),
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
            None => self.refuse("No ledger matches; create it on the masters screen.".to_owned()),
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
            | Field::Narration => Ok(()),
        };
        match check {
            Ok(()) => Step::Next,
            Err(e) => self.refuse(e),
        }
    }
}
