// src/backend/storage/cursor.rs
use crate::storage::memory::{get_cursor_memory, Memory};
use ic_stable_structures::StableCell;
use std::cell::RefCell;

// This is a generic cursor example. You might have multiple cursors
// for different purposes (e.g., pagination offset, log sequence number).
// Consider using separate MemoryIds if needed.

thread_local! {
    /// Stable cell for storing a generic cursor position (e.g., for pagination).
    static CURSOR_POSITION: RefCell<StableCell<u64, Memory>> = RefCell::new(
        StableCell::init(get_cursor_memory(), 0)
            .expect("Failed to initialize cursor cell")
    );
}

/// Get the current cursor position.
pub fn get_cursor() -> u64 {
    CURSOR_POSITION.with(|cell| *cell.borrow().get())
}

/// Set the cursor position.
pub fn set_cursor(position: u64) -> Result<(), String> {
    CURSOR_POSITION.with(|cell| {
        cell.borrow_mut()
            .set(position)
            .map(|_old_value| ())
            .map_err(|e| format!("Failed to set cursor: {:?}", e))
    })
}

/// Increment the cursor position and return the new value.
pub fn increment_cursor() -> Result<u64, String> {
    let current = get_cursor();
    let next = current.saturating_add(1);
    set_cursor(next)?;
    Ok(next)
} 