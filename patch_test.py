import re

with open("src/validation.rs", "r") as f:
    content = f.read()

replacement = """    /// Test zone-based RGB effects.
    async fn test_zone_rgb_effects(&self, keyboard: &mut dyn Keyboard) -> ValidationResult {
        let start = Instant::now();
        let test_name = "Zone RGB Effects".to_string();

        let zone_count = keyboard.zone_count();
        if zone_count == 0 {
            return ValidationResult::success(test_name, start.elapsed())
                .with_note("Skipped: No zones supported by this device");
        }

        let effects = vec![
            Effect::Breathing {
                color: Color::RED,
                speed: 1.0,
            },
            Effect::Spectrum { speed: 1.0 },
        ];

        let mut successful_effects = 0;

        for (i, effect) in effects.into_iter().enumerate() {
            let mut engine = EffectEngine::new(effect, zone_count);

            // First computation
            let colors = engine.compute();
            if let Err(e) = keyboard.set_zone_colors(colors).await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to set initial colors for effect {}: {}", i, e),
                );
            }
            if let Err(e) = keyboard.apply().await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to apply initial colors for effect {}: {}", i, e),
                );
            }

            // Simulate time passing
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Second computation (time has passed, colors should update)
            let colors = engine.compute();
            if let Err(e) = keyboard.set_zone_colors(colors).await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to set updated colors for effect {}: {}", i, e),
                );
            }
            if let Err(e) = keyboard.apply().await {
                return ValidationResult::failure(
                    test_name,
                    start.elapsed(),
                    format!("Failed to apply updated colors for effect {}: {}", i, e),
                );
            }

            successful_effects += 1;
        }

        ValidationResult::success(test_name, start.elapsed())
            .with_metric("tested_effects", successful_effects as f64)
            .with_note(&format!(
                "Successfully tested {} zone effects on {} zones",
                successful_effects, zone_count
            ))
    }"""

content = re.sub(
    r"    /// Test zone-based RGB effects\.\n    async fn test_zone_rgb_effects\(&self, _keyboard: &mut dyn Keyboard\) -> ValidationResult \{\n        let start = Instant::now\(\);\n        let test_name = \"Zone RGB Effects\"\.to_string\(\);\n\n        // For now, this is a placeholder since we don't have direct access to EffectEngine\n        // In a real implementation, this would test various effects\n        ValidationResult::success\(test_name, start\.elapsed\(\)\)\n            \.with_note\(\"Zone effects test placeholder - would test breathing, spectrum, wave effects\"\)\n    \}",
    replacement,
    content
)

with open("src/validation.rs", "w") as f:
    f.write(content)
