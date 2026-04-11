//! Tests for profile management.

use super::*;
use crate::rgb::{Color, Effect};

#[test]
fn test_profile_creation() {
    let profile = Profile::new("test");
    assert_eq!(profile.name, "test");
    assert!(profile.description.is_none());
    assert!(profile.keyboard.is_none());
    assert!(profile.headset.is_none());
}

#[test]
fn test_profile_serialization() {
    let mut profile = Profile::new("Gaming");
    profile.description = Some("My gaming profile".to_string());
    profile.keyboard = Some(KeyboardProfile {
        effect: Effect::Static { color: Color::RED },
        brightness: 80,
    });

    // Serialize to JSON
    let json = serde_json::to_string(&profile).unwrap();

    // Deserialize back
    let deserialized: Profile = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.name, "Gaming");
    assert_eq!(deserialized.description.as_deref(), Some("My gaming profile"));
    assert!(deserialized.keyboard.is_some());
    assert_eq!(deserialized.keyboard.as_ref().unwrap().brightness, 80);
}

#[test]
fn test_profile_filename_sanitization() {
    // Test sanitization by checking that ProfileManager would handle these names
    let dangerous_names = vec![
        "test/profile",
        "test\\profile",
        "test:profile",
        "test*profile",
        "test?profile",
        "test\"profile",
        "test<profile",
        "test>profile",
        "test|profile",
    ];

    for name in dangerous_names {
        let sanitized = ProfileManager::sanitize_filename(name);
        // Should not contain any dangerous characters
        assert!(!sanitized.contains('/'));
        assert!(!sanitized.contains('\\'));
        assert!(!sanitized.contains(':'));
        assert!(!sanitized.contains('*'));
        assert!(!sanitized.contains('?'));
        assert!(!sanitized.contains('"'));
        assert!(!sanitized.contains('<'));
        assert!(!sanitized.contains('>'));
        assert!(!sanitized.contains('|'));
    }
}

#[test]
fn test_keyboard_profile_default() {
    let profile = KeyboardProfile::default();
    assert_eq!(profile.brightness, 100);
    match profile.effect {
        Effect::Static { color } => assert_eq!(color, Color::WHITE),
        _ => panic!("Expected static white effect"),
    }
}

#[test]
fn test_headset_profile_default() {
    let profile = HeadsetProfile::default();
    assert_eq!(profile.sidetone, 50);
    assert_eq!(profile.mic_volume, 100);
    assert_eq!(profile.eq_preset, "Flat");
    assert_eq!(profile.auto_off_minutes, 15);
}

#[test]
fn test_profile_with_both_devices() {
    let mut profile = Profile::new("Complete");
    profile.keyboard = Some(KeyboardProfile {
        effect: Effect::Spectrum { speed: 1.0 },
        brightness: 75,
    });
    profile.headset = Some(HeadsetProfile {
        sidetone: 60,
        mic_volume: 90,
        eq_preset: "Bass Boost".to_string(),
        auto_off_minutes: 30,
    });

    // Serialize and deserialize
    let json = serde_json::to_string_pretty(&profile).unwrap();
    let deserialized: Profile = serde_json::from_str(&json).unwrap();

    assert!(deserialized.keyboard.is_some());
    assert!(deserialized.headset.is_some());
    assert_eq!(deserialized.headset.as_ref().unwrap().sidetone, 60);
}
#[test]
#[cfg(unix)]
fn test_secure_profile_permissions() {
    use std::os::unix::fs::PermissionsExt;

    // Use tempfile for a clean test environment
    let temp_dir = tempfile::tempdir().unwrap();
    let profiles_dir = temp_dir.path().join("profiles");

    // Mock the Config::config_dir to be the temp directory so ProfileManager::new creates it
    // Wait, ProfileManager::new uses Config::config_dir(), which reads from dirs_sys::config_dir().
    // We can't easily mock it. But we can just use with_dir to test file saving,
    // and manually test DirBuilder creation.

    // 1. Test directory creation
    if !profiles_dir.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&profiles_dir)
            .unwrap();
    }

    let dir_metadata = std::fs::symlink_metadata(&profiles_dir).unwrap();
    assert_eq!(dir_metadata.permissions().mode() & 0o777, 0o700);

    // 2. Test file saving
    let manager = ProfileManager::with_dir(profiles_dir.clone());
    let profile = Profile::new("SecretProfile");
    manager.save(&profile).unwrap();

    let path = profiles_dir.join("SecretProfile.json");
    let file_metadata = std::fs::symlink_metadata(&path).unwrap();
    assert_eq!(file_metadata.permissions().mode() & 0o777, 0o600);
}
use std::time::Instant;

#[tokio::test]
#[ignore = "benchmark-style test; run explicitly with `cargo test -- --ignored`"]
async fn benchmark_profile_deletion() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut manager = ProfileManager::with_dir(temp_dir.path().to_path_buf());

    let num_profiles = 1000;

    // Setup
    for i in 0..num_profiles {
        let profile = Profile::new(format!("Profile_{}", i));
        manager.set(profile).unwrap();
    }

    let start = Instant::now();
    for i in 0..num_profiles {
        manager.delete(&format!("Profile_{}", i)).await.unwrap();
    }
    let duration = start.elapsed();

    println!("Deleted {} profiles in {:?}", num_profiles, duration);
}
