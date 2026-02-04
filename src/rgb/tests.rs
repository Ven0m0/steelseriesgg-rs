//! Tests for RGB color and effect engine.

use super::*;
use crate::devices::key_mapping::KeyMappingDatabase;
use crate::devices::product_ids;

#[test]
fn test_color_from_hex() {
    assert_eq!(Color::from_hex(0xFF0000), Color::new(255, 0, 0));
    assert_eq!(Color::from_hex(0x00FF00), Color::new(0, 255, 0));
    assert_eq!(Color::from_hex(0x0000FF), Color::new(0, 0, 255));
    assert_eq!(Color::from_hex(0xFFFFFF), Color::WHITE);
    assert_eq!(Color::from_hex(0x000000), Color::BLACK);
}

#[test]
fn test_color_to_hex() {
    assert_eq!(Color::RED.to_hex(), 0xFF0000);
    assert_eq!(Color::GREEN.to_hex(), 0x00FF00);
    assert_eq!(Color::BLUE.to_hex(), 0x0000FF);
    assert_eq!(Color::WHITE.to_hex(), 0xFFFFFF);
    assert_eq!(Color::BLACK.to_hex(), 0x000000);
}

#[test]
fn test_color_blend() {
    // Blend red and blue at 50% should give purple-ish
    let blended = Color::blend(Color::RED, Color::BLUE, 0.5);
    assert_eq!(blended.r, 127); // (255 + 0) / 2
    assert_eq!(blended.g, 0);
    assert_eq!(blended.b, 127); // (0 + 255) / 2

    // Blend at 0% should give first color
    let at_zero = Color::blend(Color::RED, Color::BLUE, 0.0);
    assert_eq!(at_zero, Color::RED);

    // Blend at 100% should give second color
    let at_one = Color::blend(Color::RED, Color::BLUE, 1.0);
    assert_eq!(at_one, Color::BLUE);
}

#[test]
fn test_color_scale() {
    let color = Color::new(200, 150, 100);

    // Scale by 0.5 should halve all values
    let scaled = color.scale(0.5);
    assert_eq!(scaled.r, 100);
    assert_eq!(scaled.g, 75);
    assert_eq!(scaled.b, 50);

    // Scale by 0.0 should give black
    assert_eq!(color.scale(0.0), Color::BLACK);

    // Scale by 1.0 should be unchanged
    assert_eq!(color.scale(1.0), color);
}

#[test]
fn test_color_from_hsv() {
    // Red at hue 0
    let red = Color::from_hsv(0.0, 1.0, 1.0);
    assert!(red.r > 250 && red.g < 5 && red.b < 5);

    // Green at hue 120
    let green = Color::from_hsv(120.0, 1.0, 1.0);
    assert!(green.r < 5 && green.g > 250 && green.b < 5);

    // Blue at hue 240
    let blue = Color::from_hsv(240.0, 1.0, 1.0);
    assert!(blue.r < 5 && blue.g < 5 && blue.b > 250);

    // Grayscale (saturation = 0)
    let gray = Color::from_hsv(0.0, 0.0, 0.5);
    assert_eq!(gray.r, gray.g);
    assert_eq!(gray.g, gray.b);
}

#[test]
fn test_effect_engine_static() {
    let effect = Effect::Static { color: Color::RED };
    let mut engine = EffectEngine::new(effect, 5);

    let colors = engine.compute();
    assert_eq!(colors.len(), 5);
    for color in colors {
        assert_eq!(*color, Color::RED);
    }
}

#[test]
fn test_effect_engine_gradient() {
    let effect = Effect::Gradient {
        start: Color::RED,
        end: Color::BLUE,
    };
    let mut engine = EffectEngine::new(effect, 3);

    let colors = engine.compute();
    assert_eq!(colors.len(), 3);

    // First should be red
    assert_eq!(colors[0], Color::RED);

    // Last should be blue
    assert_eq!(colors[2], Color::BLUE);

    // Middle should be blend
    assert!(colors[1].r > 0 && colors[1].b > 0);
}

#[test]
fn test_effect_engine_custom() {
    let custom_colors = vec![Color::RED, Color::GREEN, Color::BLUE];
    let effect = Effect::Custom {
        colors: custom_colors.clone(),
    };
    let mut engine = EffectEngine::new(effect, 5);

    let colors = engine.compute();
    assert_eq!(colors.len(), 5);

    // First three should match custom colors
    assert_eq!(colors[0], Color::RED);
    assert_eq!(colors[1], Color::GREEN);
    assert_eq!(colors[2], Color::BLUE);

    // Remaining should be black (padding)
    assert_eq!(colors[3], Color::BLACK);
    assert_eq!(colors[4], Color::BLACK);
}

#[test]
fn test_effect_engine_off() {
    let effect = Effect::Off;
    let mut engine = EffectEngine::new(effect, 5);

    let colors = engine.compute();
    assert_eq!(colors.len(), 5);
    for color in colors {
        assert_eq!(*color, Color::BLACK);
    }
}

#[test]
fn test_rgb_controller_brightness() {
    let mut controller = RgbController::new(3);
    controller.set_effect(Effect::Static {
        color: Color::new(200, 200, 200),
    });

    // Full brightness
    controller.set_brightness(1.0);
    let colors = controller.compute_colors();
    assert!(colors[0].r > 195 && colors[0].r <= 200);

    // Half brightness
    controller.set_brightness(0.5);
    let colors = controller.compute_colors();
    assert!(colors[0].r >= 95 && colors[0].r <= 105);

    // Zero brightness
    controller.set_brightness(0.0);
    let colors = controller.compute_colors();
    assert_eq!(colors[0], Color::BLACK);
}

#[test]
fn test_effect_engine_caching() {
    let effect = Effect::Static { color: Color::RED };
    let mut engine = EffectEngine::new(effect, 5);

    // First compute
    let colors = engine.compute();
    assert_eq!(colors.len(), 5);

    // Immediate second compute should return cached values
    let colors = engine.compute();
    assert_eq!(colors.len(), 5);
    for color in colors {
        assert_eq!(*color, Color::RED);
    }
}

#[test]
#[ignore]
fn test_per_key_performance_benchmark() {
    let db = KeyMappingDatabase::new();
    if let Some(mapping) = db.get_mapping(product_ids::APEX_PRO_TKL_2023) {
        let mut controller = PerKeyRgbController::new(mapping);

        // Set a complex effect
        controller.set_effect(PerKeyEffect::Wave {
            colors: vec![Color::RED, Color::BLUE],
            speed: 1.0,
            direction: KeyWaveDirection::LeftToRight,
        });

        let start = std::time::Instant::now();
        let iterations = 10000;

        for _ in 0..iterations {
            let _ = controller.compute_key_colors();
        }

        let elapsed = start.elapsed();
        println!("{} iterations took {:?}", iterations, elapsed);
        println!("Average time per iteration: {:?}", elapsed / iterations);
    }
}
