import re

with open("src/validation.rs", "r") as f:
    content = f.read()

# Replace the specific import line for rgb
# From: use crate::rgb::{Color, PerKeyEffect};
# To: use crate::rgb::{Color, Effect, EffectEngine, PerKeyEffect};
content = re.sub(
    r"use crate::rgb::\{Color, PerKeyEffect\};",
    "use crate::rgb::{Color, Effect, EffectEngine, PerKeyEffect};",
    content
)

with open("src/validation.rs", "w") as f:
    f.write(content)
