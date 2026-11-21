//! Inline arithmetic for amount fields: [`eval_amount`] and its typed errors.

use std::str::FromStr;

use ledgerly_core::{Decimal, MoneyError, Paise, RoundingRule, round_rupees_to_paise};
use thiserror::Error;

/// Deepest nesting of parentheses and unary minus signs an amount may have.
const MAX_AMOUNT_DEPTH: usize = 64;
/// Longest amount text, in characters, that is evaluated.
const MAX_AMOUNT_CHARS: usize = 256;

/// Why an amount expression could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AmountError {
    /// Nothing was typed.
    #[error("type an amount")]
    Empty,
    /// A character that cannot be part of an amount.
    #[error("'{ch}' is not allowed in an amount; use digits, + - * / ( ) and %")]
    BadChar {
        /// The offending character.
        ch: char,
    },
    /// A run of digits and dots that is not a number.
    #[error("'{text}' is not a number")]
    BadNumber {
        /// The text.
        text: String,
    },
    /// The expression is incomplete or has a symbol in the wrong place.
    #[error("the amount is incomplete or has a stray symbol at character {at}")]
    Syntax {
        /// 1-based character position.
        at: usize,
    },
    /// `a+b%` or `a-b%`: it is unclear whether b% means b/100 or b percent of a.
    #[error(
        "'+ x%' and '- x%' are ambiguous (character {at}); write the base explicitly, for example 200+200*10%"
    )]
    AmbiguousPercent {
        /// 1-based character position.
        at: usize,
    },
    /// A percentage on its own (`18%`) or at the start of a sum (`10%+5`): it is unclear what it
    /// is a percentage of.
    #[error(
        "a percentage needs something to be a percentage of (character {at}); write for example 1200*18%"
    )]
    LonePercent {
        /// 1-based character position.
        at: usize,
    },
    /// Parentheses or minus signs nested too deeply.
    #[error("the amount nests parentheses or minus signs too deeply (character {at})")]
    TooDeep {
        /// 1-based character position.
        at: usize,
    },
    /// More text than any amount needs.
    #[error("the amount is longer than {max} characters")]
    TooLong {
        /// The limit.
        max: usize,
    },
    /// Division by zero.
    #[error("division by zero")]
    DivideByZero,
    /// An intermediate or final value is too large.
    #[error("the amount is too large to calculate")]
    Overflow,
    /// The result is below zero.
    #[error("the amount is negative; enter it on the other side (Dr or Cr) instead")]
    Negative,
    /// The result does not fit in paise.
    #[error(transparent)]
    Money(#[from] MoneyError),
}

/// One side of an operator while parsing: a value, and whether it is just `x%` (see
/// [`AmountError::AmbiguousPercent`]).
struct Operand {
    value: Decimal,
    bare_percent: bool,
}

impl Operand {
    fn plain(value: Decimal) -> Self {
        Operand {
            value,
            bare_percent: false,
        }
    }
}

/// Recursive-descent parser over the typed characters.
struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) {
        self.pos = self.pos.saturating_add(1);
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.bump();
        }
    }

    /// 1-based position of the next character, for messages.
    fn at(&self) -> usize {
        self.pos.saturating_add(1)
    }

    /// Enters one more nesting level, or fails when the text nests too deeply.
    fn descend(&mut self) -> Result<(), AmountError> {
        self.depth = self.depth.saturating_add(1);
        if self.depth > MAX_AMOUNT_DEPTH {
            return Err(AmountError::TooDeep { at: self.at() });
        }
        Ok(())
    }

    fn ascend(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// `expr := term (('+' | '-') term)*`
    fn expr(&mut self) -> Result<Operand, AmountError> {
        let mut acc = self.term()?;
        loop {
            self.skip_space();
            let op = self.peek();
            let adding = op == Some('+');
            if !adding && op != Some('-') {
                return Ok(acc);
            }
            if acc.bare_percent {
                return Err(AmountError::LonePercent { at: self.at() });
            }
            self.bump();
            let at = self.at();
            let rhs = self.term()?;
            if rhs.bare_percent {
                return Err(AmountError::AmbiguousPercent { at });
            }
            let next = if adding {
                acc.value.checked_add(rhs.value)
            } else {
                acc.value.checked_sub(rhs.value)
            };
            acc = Operand::plain(next.ok_or(AmountError::Overflow)?);
        }
    }

    /// `term := unary (('*' | '/') unary)*`
    fn term(&mut self) -> Result<Operand, AmountError> {
        let mut acc = self.unary()?;
        loop {
            self.skip_space();
            let op = self.peek();
            let multiplying = op == Some('*');
            if !multiplying && op != Some('/') {
                return Ok(acc);
            }
            self.bump();
            let rhs = self.unary()?;
            let next = if multiplying {
                acc.value.checked_mul(rhs.value)
            } else if rhs.value.is_zero() {
                return Err(AmountError::DivideByZero);
            } else {
                acc.value.checked_div(rhs.value)
            };
            acc = Operand::plain(next.ok_or(AmountError::Overflow)?);
        }
    }

    /// `unary := '-' unary | primary '%'*`
    fn unary(&mut self) -> Result<Operand, AmountError> {
        self.skip_space();
        if self.peek() == Some('-') {
            self.bump();
            self.descend()?;
            let inner = self.unary()?;
            self.ascend();
            return Ok(Operand {
                value: -inner.value,
                bare_percent: inner.bare_percent,
            });
        }
        let mut operand = self.primary()?;
        loop {
            self.skip_space();
            if self.peek() != Some('%') {
                return Ok(operand);
            }
            self.bump();
            let value = operand
                .value
                .checked_div(Decimal::ONE_HUNDRED)
                .ok_or(AmountError::Overflow)?;
            operand = Operand {
                value,
                bare_percent: true,
            };
        }
    }

    /// `primary := number | '(' expr ')'`
    fn primary(&mut self) -> Result<Operand, AmountError> {
        self.skip_space();
        let Some(c) = self.peek() else {
            return Err(AmountError::Syntax { at: self.at() });
        };
        if c.is_ascii_digit() || c == '.' {
            let mut text = String::new();
            while let Some(d) = self.peek().filter(|d| d.is_ascii_digit() || *d == '.') {
                text.push(d);
                self.bump();
            }
            return match Decimal::from_str(&text) {
                Ok(value) => Ok(Operand::plain(value)),
                Err(_) => Err(AmountError::BadNumber { text }),
            };
        }
        if c == '(' {
            self.bump();
            self.descend()?;
            let inner = self.expr()?;
            self.ascend();
            self.skip_space();
            if self.peek() != Some(')') {
                return Err(AmountError::Syntax { at: self.at() });
            }
            self.bump();
            return Ok(inner);
        }
        Err(AmountError::Syntax { at: self.at() })
    }
}

fn allowed_in_amount(c: char) -> bool {
    c.is_ascii_digit()
        || c.is_whitespace()
        || matches!(c, '.' | '+' | '-' | '*' | '/' | '(' | ')' | '%')
}

/// Evaluates what the user typed in an amount field to paise.
///
/// Grammar: numbers (digits with one optional `.`), `+ - * /`, parentheses, unary minus, and a
/// postfix `%` meaning "divided by 100", so `1200*18%` is 1200 x 0.18 = 216. `x%` directly after
/// `+` or `-` is refused as ambiguous ([`AmountError::AmbiguousPercent`]), and a percentage that
/// is not the operand of `*` or `/` (a lone `18%`) is refused ([`AmountError::LonePercent`]).
/// Text over 256 characters or nested more than 64 deep is refused. The result is rounded
/// to the nearest paisa, halves away from zero ([`RoundingRule::HalfUpToPaise`]). No floats are
/// involved. A negative result is an error: the side (Dr or Cr) carries the sign.
///
/// # Errors
/// An [`AmountError`] naming what is wrong with the text.
pub fn eval_amount(input: &str) -> Result<Paise, AmountError> {
    let text = input.trim();
    if text.is_empty() {
        return Err(AmountError::Empty);
    }
    if let Some(ch) = text.chars().find(|c| !allowed_in_amount(*c)) {
        return Err(AmountError::BadChar { ch });
    }
    if text
        .chars()
        .take(MAX_AMOUNT_CHARS.saturating_add(1))
        .count()
        > MAX_AMOUNT_CHARS
    {
        return Err(AmountError::TooLong {
            max: MAX_AMOUNT_CHARS,
        });
    }
    let mut parser = Parser {
        chars: text.chars().collect(),
        pos: 0,
        depth: 0,
    };
    let result = parser.expr()?;
    parser.skip_space();
    if parser.pos < parser.chars.len() {
        return Err(AmountError::Syntax { at: parser.at() });
    }
    if result.bare_percent {
        return Err(AmountError::LonePercent { at: 1 });
    }
    let value = result.value;
    if value.is_sign_negative() && !value.is_zero() {
        return Err(AmountError::Negative);
    }
    Ok(round_rupees_to_paise(value, RoundingRule::HalfUpToPaise)?)
}
