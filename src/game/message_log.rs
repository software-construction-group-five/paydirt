//! Session message history with a bounded recent window for gameplay rendering.
//!
//! Game writes messages; presentation borrows either recent messages or history.

/// Retains full session history and exposes the newest configured number of messages.
pub struct MessageLog {
    messages: Vec<String>,
    recent_capacity: usize,
}

impl MessageLog {
    /// Creates an empty history with a recent-message window of `capacity` entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            messages: Vec::new(),
            recent_capacity: capacity,
        }
    }

    /// Appends a message to the current session history.
    pub fn add_message(&mut self, message: String) {
        self.messages.push(message);
    }

    /// Borrows the recent window in chronological order, preserving older history.
    pub fn messages(&self) -> &[String] {
        let start = self.messages.len().saturating_sub(self.recent_capacity);
        &self.messages[start..]
    }

    /// Borrows every message in the current session, in chronological order.
    pub fn history(&self) -> &[String] {
        &self.messages
    }

    /// Clears both full history and the recent window when the session is reset.
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

/// Tells Rust to compile test module only during testing
#[cfg(test)]
mod tests {
    use super::MessageLog;

    #[test] // Test function
    fn new_log_is_empty() {
        let log = MessageLog::new(3); // Create new MessageLog log with capacity 3

        assert!(log.messages().is_empty()); // Check to see if log is empty -> expected result is empty log
    }

    #[test] // Test function
    fn added_messages_keep_their_order() {
        let mut log = MessageLog::new(3); // Create new MessageLog log with capacity 3

        log.add_message("Entered the mine.".to_string()); // Add string message to log
        log.add_message("Found a pickaxe.".to_string()); // Add string message to log

        assert_eq!(log.messages().len(), 2); // Checks whether length of log matches expected length
        assert_eq!(log.messages()[0], "Entered the mine."); // Expected message at log index 0
        assert_eq!(log.messages()[1], "Found a pickaxe.") // Expected message at log index 1
    }

    #[test] // Test function
    fn full_log_discards_oldest_message() {
        let mut log = MessageLog::new(2); // Create new MessageLog log with capacity 2

        log.add_message("First".to_string()); // Add message to log
        log.add_message("Second".to_string()); // Add message to log
        log.add_message("Third".to_string()); // Add message to log

        assert_eq!(log.messages().len(), 2); // Checks whether length of log matches expected length
        assert_eq!(log.messages()[0], "Second"); // Expected message at log index 0
        assert_eq!(log.messages()[1], "Third"); // Expected message at log index 1
    }

    #[test] // Test function
    fn zero_capacity_does_not_store_messages() {
        let mut log = MessageLog::new(0); // Create new MessageLog log with capacity 0

        log.add_message("Ignored".to_string()); // Add message to log with capacity 0

        assert!(log.messages().is_empty()); // Expected to still have empty log
    }

    #[test] // Test function
    fn clear_removes_messages_and_log_can_be_reused() {
        let mut log = MessageLog::new(2); // Create new MessageLog log with capacity 2

        log.add_message("First".to_string()); // Add message to log
        log.clear(); // Clear log

        assert!(log.messages().is_empty()); // Check for empty log -> Log expected to be empty

        log.add_message("After clear".to_string()); // Add new message to log after log clear

        assert_eq!(log.messages().len(), 1); // Test for log length -> Expected to be length 1
        assert_eq!(log.messages()[0], "After clear"); // Test for expected string at log index 0
    }

    #[test] // Test function
    fn capacity_one_keeps_only_latest_message() {
        let mut log = MessageLog::new(1); // Create new MessageLog log with capacity 1

        log.add_message("Old".to_string()); // Add message to log
        log.add_message("New".to_string()); // Add message to log

        assert_eq!(log.messages().len(), 1); // Test for log length -> Expected to be length 1
        assert_eq!(log.messages()[0], "New"); // Test for expected string at log index 0
    }

    #[test] // Test function
    fn history_retains_all_session_messages() {
        let mut log = MessageLog::new(2); // Create new MessageLog log with capacity 2

        log.add_message("First".to_string()); // Add message to log
        log.add_message("Second".to_string()); // Add message to log
        log.add_message("Third".to_string()); // Add message to log

        assert_eq!(log.messages().len(), 2); // Test for log length -> Expected to be length 2
        assert_eq!(log.history().len(), 3); // Test for history length -> Expected to be length 3

        assert_eq!(log.history()[0], "First"); // Test for log history at index to match string
        assert_eq!(log.history()[1], "Second"); // Test for log history at index to match string
        assert_eq!(log.history()[2], "Third"); // Test for log history at index to match string
    }
}
