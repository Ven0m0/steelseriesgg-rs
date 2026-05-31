//! GameSense HTTP server for game integration.
//!
//! Implements a compatible GameSense API that games can connect to
//! for reactive RGB lighting and device feedback.

mod server;

pub use server::GameSenseServer;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Game registration request.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GameMetadata {
    /// Game identifier (uppercase, no spaces).
    pub game: String,

    /// Display name for the game.
    pub game_display_name: String,

    /// Developer/publisher name.
    pub developer: String,

    /// Optional deinitialization timeout in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deinitialize_timer_length_ms: Option<u32>,
}

/// Game event data.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GameEvent {
    /// Game identifier.
    pub game: String,

    /// Event name.
    pub event: String,

    /// Event data.
    pub data: EventData,
}

/// Event data payload.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventData {
    /// Numeric value (0-100 typically).
    pub value: i32,

    /// Optional frame data for complex events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame: Option<HashMap<String, serde_json::Value>>,
}

/// Event handler binding.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EventBinding {
    /// Game identifier.
    pub game: String,

    /// Event name.
    pub event: String,

    /// Minimum value.
    #[serde(default)]
    pub min_value: i32,

    /// Maximum value.
    #[serde(default = "default_max_value")]
    pub max_value: i32,

    /// Icon ID (0-40).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_id: Option<u8>,

    /// Handlers for this event.
    pub handlers: Vec<Handler>,
}

fn default_max_value() -> i32 {
    100
}

/// Event handler definition.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "device-type")]
pub enum Handler {
    /// RGB zone handler.
    #[serde(rename = "rgb-per-key-zones")]
    RgbPerKeyZones {
        zone: String,
        color: ColorHandler,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<String>,
    },

    /// Keyboard handler.
    #[serde(rename = "keyboard")]
    Keyboard {
        zone: String,
        color: ColorHandler,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<String>,
    },

    /// Screen handler (OLED).
    #[serde(rename = "screened")]
    Screen {
        zone: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        datas: Option<Vec<ScreenData>>,
    },

    /// Tactile handler (vibration).
    #[serde(rename = "tactile")]
    Tactile {
        zone: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mode: Option<String>,
    },
}

/// Color handler definition.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ColorHandler {
    /// Static color.
    Static { red: u8, green: u8, blue: u8 },

    /// Gradient between two colors.
    Gradient { gradient: GradientSpec },

    /// Range-based color selection.
    Range { color: Vec<RangeColor> },
}

/// Gradient specification.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GradientSpec {
    pub zero: ColorSpec,
    pub hundred: ColorSpec,
}

/// Color specification.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ColorSpec {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

/// Range-based color.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RangeColor {
    pub low: i32,
    pub high: i32,
    pub color: ColorSpec,
}

/// Screen data for OLED display.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ScreenData {
    /// Line number (0-indexed).
    pub line: u8,

    /// Text to display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Icon ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_id: Option<u8>,

    /// Whether to show progress bar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_progress_bar: Option<bool>,
}

/// Heartbeat request.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Heartbeat {
    /// Game identifier.
    pub game: String,
}

/// Remove game request.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemoveGame {
    /// Game identifier.
    pub game: String,
}

/// Remove event request.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemoveEvent {
    /// Game identifier.
    pub game: String,

    /// Event name.
    pub event: String,
}

/// API response.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiResponse {
    /// Whether the request succeeded.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<bool>,

    /// Error code if failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Error details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_text: Option<String>,
}

impl ApiResponse {
    /// Create a success response.
    pub fn success() -> Self {
        Self {
            success: Some(true),
            error: None,
            error_text: None,
        }
    }

    /// Create an error response.
    pub fn error(code: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            success: Some(false),
            error: Some(code.into()),
            error_text: Some(text.into()),
        }
    }
}

/// Create a simple static color handler.
pub fn static_color_handler(zone: &str, color: crate::rgb::Color) -> Handler {
    Handler::Keyboard {
        zone: zone.to_string(),
        color: ColorHandler::Static {
            red: color.r,
            green: color.g,
            blue: color.b,
        },
        mode: None,
    }
}

/// Create a health bar handler (red at low, green at high).
pub fn health_bar_handler(zone: &str) -> Handler {
    Handler::Keyboard {
        zone: zone.to_string(),
        color: ColorHandler::Gradient {
            gradient: GradientSpec {
                zero: ColorSpec {
                    red: 255,
                    green: 0,
                    blue: 0,
                },
                hundred: ColorSpec {
                    red: 0,
                    green: 255,
                    blue: 0,
                },
            },
        },
        mode: None,
    }
}

/// Create an ammo counter handler (yellow at low, white at high).
pub fn ammo_handler(zone: &str) -> Handler {
    Handler::Keyboard {
        zone: zone.to_string(),
        color: ColorHandler::Gradient {
            gradient: GradientSpec {
                zero: ColorSpec {
                    red: 255,
                    green: 200,
                    blue: 0,
                },
                hundred: ColorSpec {
                    red: 255,
                    green: 255,
                    blue: 255,
                },
            },
        },
        mode: None,
    }
}

/// Create a cooldown handler with range colors.
pub fn cooldown_handler(zone: &str) -> Handler {
    Handler::Keyboard {
        zone: zone.to_string(),
        color: ColorHandler::Range {
            color: vec![
                RangeColor {
                    low: 0,
                    high: 25,
                    color: ColorSpec {
                        red: 255,
                        green: 0,
                        blue: 0,
                    },
                },
                RangeColor {
                    low: 26,
                    high: 50,
                    color: ColorSpec {
                        red: 255,
                        green: 128,
                        blue: 0,
                    },
                },
                RangeColor {
                    low: 51,
                    high: 75,
                    color: ColorSpec {
                        red: 255,
                        green: 255,
                        blue: 0,
                    },
                },
                RangeColor {
                    low: 76,
                    high: 100,
                    color: ColorSpec {
                        red: 0,
                        green: 255,
                        blue: 0,
                    },
                },
            ],
        },
        mode: None,
    }
}

/// Common zone names.
pub mod zones {
    pub const FUNCTION_KEYS: &str = "function-keys";
    pub const NUMBER_KEYS: &str = "number-keys";
    pub const MAIN_KEYBOARD: &str = "main-keyboard";
    pub const KEYPAD: &str = "keypad";
    pub const ARROW_KEYS: &str = "arrow-keys";
    pub const NAV_CLUSTER: &str = "nav-cluster";
    pub const ALL: &str = "all";
    pub const LOGO: &str = "logo";
    pub const WHEEL: &str = "wheel";
}

/// Preset event bindings for common games.
pub mod presets {
    use super::*;

    pub fn fps_preset(game_id: &str) -> Vec<EventBinding> {
        vec![
            EventBinding {
                game: game_id.to_string(),
                event: "HEALTH".to_string(),
                min_value: 0,
                max_value: 100,
                icon_id: Some(1),
                handlers: vec![health_bar_handler(zones::FUNCTION_KEYS)],
            },
            EventBinding {
                game: game_id.to_string(),
                event: "AMMO".to_string(),
                min_value: 0,
                max_value: 100,
                icon_id: Some(2),
                handlers: vec![ammo_handler(zones::NUMBER_KEYS)],
            },
        ]
    }

    pub fn moba_preset(game_id: &str) -> Vec<EventBinding> {
        vec![
            EventBinding {
                game: game_id.to_string(),
                event: "HEALTH".to_string(),
                min_value: 0,
                max_value: 100,
                icon_id: Some(1),
                handlers: vec![health_bar_handler(zones::FUNCTION_KEYS)],
            },
            EventBinding {
                game: game_id.to_string(),
                event: "MANA".to_string(),
                min_value: 0,
                max_value: 100,
                icon_id: Some(3),
                handlers: vec![Handler::Keyboard {
                    zone: zones::NUMBER_KEYS.to_string(),
                    color: ColorHandler::Gradient {
                        gradient: GradientSpec {
                            zero: ColorSpec {
                                red: 0,
                                green: 0,
                                blue: 100,
                            },
                            hundred: ColorSpec {
                                red: 0,
                                green: 100,
                                blue: 255,
                            },
                        },
                    },
                    mode: None,
                }],
            },
            EventBinding {
                game: game_id.to_string(),
                event: "ABILITY_Q".to_string(),
                min_value: 0,
                max_value: 100,
                icon_id: None,
                handlers: vec![cooldown_handler("q")],
            },
            EventBinding {
                game: game_id.to_string(),
                event: "ABILITY_W".to_string(),
                min_value: 0,
                max_value: 100,
                icon_id: None,
                handlers: vec![cooldown_handler("w")],
            },
            EventBinding {
                game: game_id.to_string(),
                event: "ABILITY_E".to_string(),
                min_value: 0,
                max_value: 100,
                icon_id: None,
                handlers: vec![cooldown_handler("e")],
            },
            EventBinding {
                game: game_id.to_string(),
                event: "ABILITY_R".to_string(),
                min_value: 0,
                max_value: 100,
                icon_id: None,
                handlers: vec![cooldown_handler("r")],
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rgb::Color;

    #[test]
    fn test_static_color_handler() {
        let handler = static_color_handler(zones::FUNCTION_KEYS, Color::new(255, 128, 0));
        match handler {
            Handler::Keyboard { zone, color, mode } => {
                assert_eq!(zone, zones::FUNCTION_KEYS);
                assert!(mode.is_none());
                match color {
                    ColorHandler::Static { red, green, blue } => {
                        assert_eq!(red, 255);
                        assert_eq!(green, 128);
                        assert_eq!(blue, 0);
                    }
                    _ => panic!("Expected Static color handler"),
                }
            }
            _ => panic!("Expected Keyboard handler"),
        }
    }

    #[test]
    fn test_health_bar_handler() {
        let handler = health_bar_handler(zones::MAIN_KEYBOARD);
        match handler {
            Handler::Keyboard { zone, color, mode } => {
                assert_eq!(zone, zones::MAIN_KEYBOARD);
                assert!(mode.is_none());
                match color {
                    ColorHandler::Gradient { gradient } => {
                        assert_eq!(gradient.zero.red, 255);
                        assert_eq!(gradient.zero.green, 0);
                        assert_eq!(gradient.zero.blue, 0);
                        assert_eq!(gradient.hundred.red, 0);
                        assert_eq!(gradient.hundred.green, 255);
                        assert_eq!(gradient.hundred.blue, 0);
                    }
                    _ => panic!("Expected Gradient color handler"),
                }
            }
            _ => panic!("Expected Keyboard handler"),
        }
    }

    #[test]
    fn test_ammo_handler() {
        let handler = ammo_handler(zones::NUMBER_KEYS);
        match handler {
            Handler::Keyboard { zone, color, .. } => {
                assert_eq!(zone, zones::NUMBER_KEYS);
                match color {
                    ColorHandler::Gradient { gradient } => {
                        assert_eq!(gradient.zero.red, 255);
                        assert_eq!(gradient.zero.green, 200);
                        assert_eq!(gradient.hundred.red, 255);
                        assert_eq!(gradient.hundred.green, 255);
                        assert_eq!(gradient.hundred.blue, 255);
                    }
                    _ => panic!("Expected Gradient color handler"),
                }
            }
            _ => panic!("Expected Keyboard handler"),
        }
    }

    #[test]
    fn test_cooldown_handler() {
        let handler = cooldown_handler("q");
        match handler {
            Handler::Keyboard { zone, color, .. } => {
                assert_eq!(zone, "q");
                match color {
                    ColorHandler::Range { color: ranges } => {
                        assert_eq!(ranges.len(), 4);
                        assert_eq!(ranges[0].low, 0);
                        assert_eq!(ranges[0].high, 25);
                        assert_eq!(ranges[0].color.red, 255);
                        assert_eq!(ranges[3].low, 76);
                        assert_eq!(ranges[3].high, 100);
                        assert_eq!(ranges[3].color.green, 255);
                    }
                    _ => panic!("Expected Range color handler"),
                }
            }
            _ => panic!("Expected Keyboard handler"),
        }
    }

    #[test]
    fn test_fps_preset() {
        let bindings = presets::fps_preset("TEST_GAME");
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].game, "TEST_GAME");
        assert_eq!(bindings[0].event, "HEALTH");
        assert_eq!(bindings[0].min_value, 0);
        assert_eq!(bindings[0].max_value, 100);
        assert_eq!(bindings[0].icon_id, Some(1));
        assert_eq!(bindings[1].event, "AMMO");
        assert_eq!(bindings[1].icon_id, Some(2));
    }

    #[test]
    fn test_moba_preset() {
        let bindings = presets::moba_preset("TEST_MOBA");
        assert_eq!(bindings.len(), 6);
        assert_eq!(bindings[0].event, "HEALTH");
        assert_eq!(bindings[1].event, "MANA");
        assert_eq!(bindings[2].event, "ABILITY_Q");
        assert_eq!(bindings[3].event, "ABILITY_W");
        assert_eq!(bindings[4].event, "ABILITY_E");
        assert_eq!(bindings[5].event, "ABILITY_R");
        match &bindings[1].handlers[0] {
            Handler::Keyboard { zone, color, .. } => {
                assert_eq!(zone, zones::NUMBER_KEYS);
                match color {
                    ColorHandler::Gradient { gradient } => {
                        assert_eq!(gradient.zero.blue, 100);
                        assert_eq!(gradient.hundred.blue, 255);
                    }
                    _ => panic!("Expected Gradient handler for mana"),
                }
            }
            _ => panic!("Expected Keyboard handler"),
        }
    }
}
