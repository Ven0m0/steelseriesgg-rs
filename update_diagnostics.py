import re

with open("src/diagnostics_export.rs", "r") as f:
    content = f.read()

replacement = """
/// Error log entry (placeholder for future error tracking).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorLog {
    /// Error timestamp
    pub timestamp: DateTime<Utc>,
    /// Error message
    pub message: String,
}

/// Global error log to keep track of recent errors.
static GLOBAL_ERROR_LOG: std::sync::OnceLock<parking_lot::Mutex<Vec<ErrorLog>>> = std::sync::OnceLock::new();

/// Maximum number of errors to keep in the global log.
const MAX_ERROR_LOGS: usize = 100;

/// Record a new error message in the global error log.
pub fn record_error(message: String) {
    let log_mutex = GLOBAL_ERROR_LOG.get_or_init(|| parking_lot::Mutex::new(Vec::new()));
    let mut logs = log_mutex.lock();

    logs.push(ErrorLog {
        timestamp: Utc::now(),
        message,
    });

    // Keep only the most recent errors
    if logs.len() > MAX_ERROR_LOGS {
        let excess = logs.len() - MAX_ERROR_LOGS;
        logs.drain(0..excess);
    }
}

/// Retrieve a copy of the recent error logs.
pub fn get_recent_errors() -> Vec<ErrorLog> {
    if let Some(log_mutex) = GLOBAL_ERROR_LOG.get() {
        log_mutex.lock().clone()
    } else {
        Vec::new()
    }
}
"""

content = re.sub(
    r"/// Error log entry \(placeholder for future error tracking\).\s*#\[derive\(Debug, Serialize, Deserialize\)\]\s*pub struct ErrorLog \{\s*/// Error timestamp\s*pub timestamp: DateTime<Utc>,\s*/// Error message\s*pub message: String,\s*\}",
    replacement.strip(),
    content,
    flags=re.MULTILINE
)

# Also update recent_errors assignment
content = re.sub(
    r"// Placeholder for error logs \(future enhancement\)\s*let recent_errors = Vec::new\(\);",
    "// Fetch recent error logs\n    let recent_errors = get_recent_errors();",
    content
)

# Fix tests
replacement_tests = """
#[cfg(test)]
mod tests {
    use super::*;

    // Since GLOBAL_ERROR_LOG is static, tests running in parallel can conflict.
    // We use a local static Mutex to serialize access to the global error log in tests.
    static TEST_MUTEX: parking_lot::Mutex<()> = parking_lot::const_mutex(());

    fn clear_global_logs() {
        if let Some(log_mutex) = GLOBAL_ERROR_LOG.get() {
            log_mutex.lock().clear();
        }
    }

    #[test]
    fn test_record_and_get_errors() {
        let _guard = TEST_MUTEX.lock();
        clear_global_logs();

        record_error("Test error 1".to_string());
        record_error("Test error 2".to_string());

        let errors = get_recent_errors();
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].message, "Test error 1");
        assert_eq!(errors[1].message, "Test error 2");
    }

    #[test]
    fn test_error_log_size_limit() {
        let _guard = TEST_MUTEX.lock();
        clear_global_logs();

        // Add more than the limit
        for i in 0..110 {
            record_error(format!("Error {}", i));
        }

        let errors = get_recent_errors();
        assert_eq!(errors.len(), MAX_ERROR_LOGS);

        // Ensure we kept the most recent ones
        // Since we insert sequentially, if we had 110 (0..109), the ones kept are 10..109
        assert_eq!(errors[0].message, "Error 10");
        assert_eq!(errors[errors.len() - 1].message, "Error 109");
    }
}
"""

if "#[cfg(test)]" not in content:
    content += "\n" + replacement_tests.strip() + "\n"

with open("src/diagnostics_export.rs", "w") as f:
    f.write(content)
