//! Loading [`Books`] from the store. **Card S4** implements this module (replacing the body
//! below; it returns [`StoreError::NotBuilt`] until then). Stock movements are filled by
//! [`crate::stock_load`] (card S5).

use ledgerly_core::Period;
use ledgerly_model::Books;

use crate::{SqliteStore, StoreError};

/// See [`crate::Store::load_books`].
///
/// # Errors
/// [`StoreError`] on a read failure.
pub fn load_books(store: &SqliteStore, period: Option<Period>) -> Result<Books, StoreError> {
    let _ = (store, period);
    Err(StoreError::NotBuilt("S4"))
}
