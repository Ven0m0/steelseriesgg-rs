use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use steelseries_gg::config::Config;
use steelseries_gg::devices::diagnostics::HidDiagnostics;

#[test]
fn test_diagnostic_log_permissions() {
    let mut diag = HidDiagnostics::new(true);

    diag.enable_file_logging().expect("Failed to enable file logging");

    // Determine log directory (should match logic in diagnostics.rs)
    let log_dir = if let Some(config_dir) = Config::config_dir() {
        config_dir.join("logs")
    } else {
        PathBuf::from("logs")
    };

    // Find the created log file
    let mut log_file: Option<PathBuf> = None;
    let entries = fs::read_dir(&log_dir).expect("Failed to read log directory");

    for entry in entries {
        let entry = entry.expect("Failed to get entry");
        let path = entry.path();
        if let Some(filename) = path.file_name().and_then(|s| s.to_str()) {
            if filename.starts_with("ssgg_hid_diagnostics_") && filename.ends_with(".log") {
                log_file = Some(path);
                // We could break here, but we might want the most recent one if multiple exist
            }
        }
    }

    let path = log_file.expect("Log file was not created");
    #[allow(unused_variables)]
    let metadata = fs::metadata(&path).expect("Failed to get metadata");

    #[cfg(unix)]
    {
        let mode = metadata.permissions().mode() & 0o777;
        println!("File mode: {:o}", mode);
        // Check directory permissions too
        let dir_metadata = fs::metadata(&log_dir).expect("Failed to get log dir metadata");
        let dir_mode = dir_metadata.permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "Log directory should have 0o700 permissions");

        assert_eq!(mode, 0o600, "Log file should have 0o600 permissions");
    }

    // Clean up (optional, but good practice)
    fs::remove_file(&path).expect("Failed to remove log file");
}
