//! Rolling log of gameplay messages ("You hit the rat.").
//!
//! Written by Game, read by Renderer::draw_status.

/// Bounded list of recent messages
pub struct MessageLog {
    messages: Vec<String>, // A dynamically sized list of gameplay messages
    capacity: usize,       // The maximum number of messages the message log should retain
}

/// Defines methods associated with MessageLog struct
impl MessageLog {
    pub fn new(capacity: usize) -> Self {
        // Allows game to create a MessageLog
        Self {
            messages: Vec::new(), // Initilize an empty vector
            capacity,
        }
    }

    /// Adds a message in the session history
    /// SDD: Related to Game Contract 2 (ProcessAction), recording gameplay actions
    pub fn add_message(&mut self, message: String) {
        self.messages.push(message);
    }

    /// Returns most recent messages for normal gameplay
    /// SDD: Supports Renderer Contract 20 (RenderGameState)
    /// Protocol 3: draw_status(player, log)
    pub fn messages(&self) -> &[String] {
        let start = self.messages.len().saturating_sub(self.capacity);
        &self.messages[start..]
    }

    /// Returns every message from the current Game session (pressing button like "H")
    pub fn history(&self) -> &[String] {
        &self.messages
    }

    /// Removes all stored messages when starting a new session
    /// SDD: Supports Game Contract 1 (InitializeGame)
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
