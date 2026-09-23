//! Rule-based suggestions: a secret's type, a conventional name for it, and
//! structured search filters.
//!
//! All three are plain string matching in `envryn_core` -- instant, offline,
//! and they never send a value anywhere. When the rules do not recognise
//! something the commands return `None`, and the UI says "Unknown" rather
//! than guessing.

use envryn_core::classify::{self, Classification};
use envryn_core::search::{self, SearchFilter};

/// Recognise a value by its prefix or shape, falling back to the variable
/// name. `None` means the type is unknown.
#[tauri::command]
pub fn classify_value(value: String, name: Option<String>) -> Option<Classification> {
    classify::classify_value_or_name(&value, name.as_deref())
}

/// A conventional variable name for a pasted value (`sk_live_...` becomes
/// `STRIPE_SECRET_KEY`). `None` means the name is unknown.
#[tauri::command]
pub fn suggest_secret_name(value: String) -> Option<String> {
    classify::suggest_name(&value)
}

/// Split a typed search into environment, kind, and free-text filters.
#[tauri::command]
pub fn search_parse_query(query: String) -> SearchFilter {
    search::parse_query(&query)
}
