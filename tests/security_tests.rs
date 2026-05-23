use std::fs;
use steelseries_gg::fs_utils::{secure_write, secure_write_async};
use tempfile::tempdir;

#[cfg(unix)]
#[test]
fn test_secure_write_refuses_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().unwrap();
    let target = dir.path().join("target.txt");
    let link = dir.path().join("link.txt");

    // Create a target file
    fs::write(&target, "target content").unwrap();

    // Create a symlink pointing to the target
    symlink(&target, &link).unwrap();

    // Attempt to write to the link using secure_write
    let result = secure_write(&link, "malicious content");

    // It should fail
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("symlink"));

    // Verify target content was NOT changed
    let content = fs::read_to_string(&target).unwrap();
    assert_eq!(content, "target content");
}

#[cfg(unix)]
#[tokio::test]
async fn test_secure_write_async_refuses_symlink() {
    use std::os::unix::fs::symlink;
    let dir = tempdir().unwrap();
    let target = dir.path().join("target_async.txt");
    let link = dir.path().join("link_async.txt");

    // Create a target file
    fs::write(&target, "target content").unwrap();

    // Create a symlink pointing to the target
    symlink(&target, &link).unwrap();

    // Attempt to write to the link using secure_write_async
    let result = secure_write_async(link.to_string_lossy().to_string(), "malicious content".to_string()).await;

    // It should fail
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("symlink"));

    // Verify target content was NOT changed
    let content = fs::read_to_string(&target).unwrap();
    assert_eq!(content, "target content");
}

#[test]
fn test_secure_write_creates_new_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("new_file.txt");

    let result = secure_write(&path, "new content");
    assert!(result.is_ok());

    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "new content");

    // Check permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(&path).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
    }
}
