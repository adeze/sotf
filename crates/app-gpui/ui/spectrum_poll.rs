//! Snapshot requests and display updates have different spectrum cadences.

/// Keep visible spectrum data in every asynchronous snapshot request.
///
/// A snapshot can arrive one tick later. Alternating requests with no analyzer
/// would otherwise erase the data just before a throttled display update.
pub(crate) fn spectrum_poll_schedule(frame: u64, visible: bool) -> (bool, bool) {
    (visible, visible && frame.is_multiple_of(2))
}
