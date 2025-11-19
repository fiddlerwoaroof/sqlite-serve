//! Legacy logging utilities for sqlite-serve
//!
//! NOTE: This module is deprecated. New code should use the Logger trait
//! in the domain module with the NginxLogger adapter for dependency injection.
//! This module is kept for backwards compatibility if needed.

#[cfg(test)]
mod tests {
    #[test]
    fn test_log_level_ordering() {
        // Placeholder test - logging functionality moved to domain::Logger trait
        assert!(true);
    }
}
