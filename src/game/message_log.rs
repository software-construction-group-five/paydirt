//! Session message history with a bounded recent window for gameplay rendering.
//!
//! Game writes messages; presentation borrows either recent messages or history.

/// Retains full session history and exposes the newest configured number of messages.
pub struct MessageLog {
    messages: Vec<String>,
    recent_capacity: usize,
    history_lines: usize,
}

impl MessageLog {
    /// Creates an empty history with a recent-message window of `capacity` entries.
    pub fn new(capacity: usize) -> Self {
        Self {
            messages: Vec::new(),
            recent_capacity: capacity,
            history_lines: 0,
        }
    }

    /// Appends a message to the current session history.
    pub fn add_message(&mut self, message: String) {
        self.history_lines = self.history_lines.saturating_add(message.lines().count());
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

    /// Number of logical history lines, including interior blank lines.
    pub fn history_lines(&self) -> usize {
        self.history_lines
    }

    /// Clears both full history and the recent window when the session is reset.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.history_lines = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::MessageLog;

    #[test]
    fn history_line_count_tracks_multiline_messages_and_session_reset() {
        let mut log = MessageLog::new(1);
        for message in ["oldest\n\nsecond\n", "", "newest"] {
            log.add_message(message.to_owned());
        }
        assert_eq!(log.history_lines(), 4);
        assert_eq!(log.messages(), &["newest"]);
        log.clear();
        assert_eq!(log.history_lines(), 0);
        log.add_message("new session".to_owned());
        assert_eq!(log.history_lines(), 1);
    }

    #[test]
    fn new_log_is_empty() {
        let log = MessageLog::new(3);

        assert!(log.messages().is_empty());
    }

    #[test]
    fn added_messages_keep_their_order() {
        let mut log = MessageLog::new(3);

        log.add_message("Entered the mine.".to_string());
        log.add_message("Found a pickaxe.".to_string());

        assert_eq!(log.messages().len(), 2);
        assert_eq!(log.messages()[0], "Entered the mine.");
        assert_eq!(log.messages()[1], "Found a pickaxe.")
    }

    #[test]
    fn recent_window_excludes_oldest_message() {
        let mut log = MessageLog::new(2);

        log.add_message("First".to_string());
        log.add_message("Second".to_string());
        log.add_message("Third".to_string());

        assert_eq!(log.messages().len(), 2);
        assert_eq!(log.messages()[0], "Second");
        assert_eq!(log.messages()[1], "Third");
    }

    #[test]
    fn zero_capacity_hides_recent_messages_but_retains_history() {
        let mut log = MessageLog::new(0);

        log.add_message("Retained".to_string());

        assert!(log.messages().is_empty());
        assert_eq!(log.history(), &["Retained"]);
        assert_eq!(log.history_lines(), 1);
    }

    #[test]
    fn clear_removes_messages_and_log_can_be_reused() {
        let mut log = MessageLog::new(2);

        log.add_message("First".to_string());
        log.clear();

        assert!(log.messages().is_empty());

        log.add_message("After clear".to_string());

        assert_eq!(log.messages().len(), 1);
        assert_eq!(log.messages()[0], "After clear");
    }

    #[test]
    fn capacity_one_keeps_only_latest_message() {
        let mut log = MessageLog::new(1);

        log.add_message("Old".to_string());
        log.add_message("New".to_string());

        assert_eq!(log.messages().len(), 1);
        assert_eq!(log.messages()[0], "New");
    }

    #[test]
    fn history_retains_all_session_messages() {
        let mut log = MessageLog::new(2);

        log.add_message("First".to_string());
        log.add_message("Second".to_string());
        log.add_message("Third".to_string());

        assert_eq!(log.messages().len(), 2);
        assert_eq!(log.history().len(), 3);

        assert_eq!(log.history()[0], "First");
        assert_eq!(log.history()[1], "Second");
        assert_eq!(log.history()[2], "Third");
    }
}
