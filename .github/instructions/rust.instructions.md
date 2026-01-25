---
description: "Rust coding conventions and idiomatic patterns"
applyTo: "**/*.rs"
---

# Rust Best Practices

**Goal:** Memory-safe, performant, idiomatic Rust with zero-cost abstractions.

## Core Rules

- **Safety**: Ownership+borrowing > GC; no `.unwrap()` in prod; `Result<T,E>` for errors
- **Performance**: Zero-cost abstractions; iterators > loops; stack > heap
- **Idioms**: Traits for polymorphism; lifetimes for safety; `?` operator
- **Format**: `cargo fmt`; Clippy strict (`-D warnings`); lines <100; 2-space indent
- **Quality**: No warnings; comprehensive tests; rustdoc for public APIs

## Project Structure

```rust
// lib.rs or main.rs
mod error;    // thiserror for custom errors
mod model;    // Domain types
mod service;  // Business logic
mod storage;  // Persistence

pub use error::{Error, Result};
pub use model::*;
```

**Cargo.toml:**

- Semantic versioning
- Metadata: `description`, `license`, `repository`, `keywords`, `categories`
- Feature flags for optional functionality

## Ownership & Borrowing

```rust
// ✅ Prefer borrowing
fn process(data: &str) -> String { data.to_uppercase() }

// ✅ Mutable borrowing when needed
fn modify(data: &mut Vec<i32>) { data.push(42); }

// ❌ Avoid unnecessary cloning
fn bad(data: String) -> String { data }
```

**Smart Pointers:**

- `Box<T>`: Heap allocation, single owner
- `Rc<T>`: Shared ownership, single-threaded
- `Arc<T>`: Shared ownership, multi-threaded
- `RefCell<T>`: Interior mutability, single-threaded
- `Mutex<T>`, `RwLock<T>`: Interior mutability, multi-threaded

## Error Handling

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
  #[error("IO error: {0}")]
  Io(#[from] std::io::Error),
  #[error("Parse error: {0}")]
  Parse(String),
  #[error("Not found: {0}")]
  NotFound(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

// Use ? operator
fn load_config() -> Result<Config> {
  let data = std::fs::read_to_string("config.toml")?;
  let config: Config = toml::from_str(&data)
    .map_err(|e| AppError::Parse(e.to_string()))?;
  Ok(config)
}
```

## Traits

```rust
// Define behavior
pub trait Processor {
  type Output;
  fn process(&self, input: &str) -> Self::Output;
}

// Implement for types
impl Processor for JsonProcessor {
  type Output = serde_json::Value;
  fn process(&self, input: &str) -> Self::Output {
    serde_json::from_str(input).unwrap_or_default()
  }
}

// Generic constraints
fn run<P: Processor>(p: &P, data: &str) -> P::Output {
  p.process(data)
}
```

## Patterns

**Builder Pattern:**

```rust
pub struct Config {
  host: String,
  port: u16,
  timeout: Duration,
}

impl Config {
  pub fn builder() -> ConfigBuilder {
    ConfigBuilder::default()
  }
}

#[derive(Default)]
pub struct ConfigBuilder {
  host: Option<String>,
  port: Option<u16>,
  timeout: Option<Duration>,
}

impl ConfigBuilder {
  pub fn host(mut self, h: impl Into<String>) -> Self {
    self.host = Some(h.into());
    self
  }

  pub fn build(self) -> Result<Config> {
    Ok(Config {
      host: self.host.ok_or(AppError::Parse("host required".into()))?,
      port: self.port.unwrap_or(8080),
      timeout: self.timeout.unwrap_or(Duration::from_secs(30)),
    })
  }
}
```

**Newtype Pattern:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(u64);

impl UserId {
  pub fn new(id: u64) -> Self { Self(id) }
  pub fn inner(&self) -> u64 { self.0 }
}
```

**Iterator Chains:**

```rust
// ✅ Idiomatic
let evens: Vec<_> = (0..100)
  .filter(|x| x % 2 == 0)
  .map(|x| x * 2)
  .collect();

// ❌ Not idiomatic
let mut evens = Vec::new();
for i in 0..100 {
  if i % 2 == 0 {
    evens.push(i * 2);
  }
}
```

## Performance

**Stack vs Heap:**

```rust
let small = [0u8; 64];              // Stack (fast)
let large = vec![0u8; 1024*1024];   // Heap (necessary)
```

**Zero-Copy:**

```rust
// ✅ Borrow instead of clone
fn process(data: &[u8]) -> usize { data.len() }

// ✅ Cow for conditional ownership
use std::borrow::Cow;
fn maybe_modify(data: &str, modify: bool) -> Cow<'_, str> {
  if modify {
    Cow::Owned(data.to_uppercase())
  } else {
    Cow::Borrowed(data)
  }
}
```

**Inline:**

```rust
#[inline]
pub fn hot_path(x: i32) -> i32 { x * 2 }

#[inline(always)]
pub fn critical(x: i32) -> i32 { x + 1 }
```

## Concurrency

**Channels:**

```rust
use std::sync::mpsc;
use std::thread;

let (tx, rx) = mpsc::channel();
thread::spawn(move || {
  tx.send(42).unwrap();
});
let value = rx.recv().unwrap();
```

**Async:**

```rust
use tokio;

#[tokio::main]
async fn main() -> Result<()> {
  let data = fetch_data().await?;
  process(data).await?;
  Ok(())
}

async fn fetch_data() -> Result<String> {
  let response = reqwest::get("https://api.example.com/data").await?;
  response.text().await.map_err(Into::into)
}
```

**Rayon (Data Parallelism):**

```rust
use rayon::prelude::*;

let sum: i32 = (0..1000000)
  .into_par_iter()
  .map(|x| x * 2)
  .sum();
```

## Type Safety

**Phantom Types:**

```rust
use std::marker::PhantomData;

struct Buffer<T, State> {
  data: Vec<u8>,
  _marker: PhantomData<(T, State)>,
}

struct Empty;
struct Full;

impl<T> Buffer<T, Empty> {
  fn new() -> Self {
    Buffer { data: Vec::new(), _marker: PhantomData }
  }

  fn fill(mut self, data: Vec<u8>) -> Buffer<T, Full> {
    self.data = data;
    Buffer { data: self.data, _marker: PhantomData }
  }
}

impl<T> Buffer<T, Full> {
  fn read(&self) -> &[u8] { &self.data }
}
```

## Unsafe Code

```rust
// Only when absolutely necessary
// ALWAYS document safety invariants
unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> &'static [u8] {
  // SAFETY: Caller must ensure:
  // 1. ptr is valid for reads of len bytes
  // 2. ptr is properly aligned
  // 3. Data at ptr outlives 'static
  std::slice::from_raw_parts(ptr, len)
}
```

**Minimize unsafe:**

- Use safe abstractions when possible
- Isolate unsafe code in small functions
- Document all safety invariants
- Test with Miri: `cargo miri test`

## Testing

```rust
#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_basic() {
    assert_eq!(process("test"), "TEST");
  }

  #[test]
  #[should_panic(expected = "not found")]
  fn test_error() {
    load_missing_file().unwrap();
  }

  #[test]
  fn test_result() -> Result<()> {
    let config = load_config()?;
    assert_eq!(config.port, 8080);
    Ok(())
  }
}
```

**Property Testing:**

```rust
use proptest::prelude::*;

proptest! {
  #[test]
  fn test_reversible(s in "\\PC*") {
    let encoded = encode(&s);
    let decoded = decode(&encoded)?;
    prop_assert_eq!(s, decoded);
  }
}
```

## Documentation

````rust
/// Processes input data and returns transformed output.
///
/// # Arguments
/// * `input` - The input string to process
///
/// # Returns
/// Transformed output string
///
/// # Errors
/// Returns `AppError::Parse` if input is invalid
///
/// # Examples
/// ```
/// use mylib::process;
/// assert_eq!(process("test"), "TEST");
/// ```
pub fn process(input: &str) -> Result<String> {
  Ok(input.to_uppercase())
}
````

## Toolchain

```bash
# Format
cargo fmt

# Lint (strict)
cargo clippy --all-targets --all-features -- -D warnings

# Test
cargo test --all-features

# Safety check
cargo miri test

# Audit dependencies
cargo audit

# Bench
cargo bench

# Flamegraph
cargo flamegraph --bin myapp
```

## Cargo.toml Best Practices

```toml
[package]
name = "myapp"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"
tokio = { version = "1.35", features = ["full"] }

[dev-dependencies]
proptest = "1.4"

[profile.release]
lto = true              # Link-time optimization
codegen-units = 1       # Better optimization
panic = "abort"         # Smaller binary
strip = true            # Remove debug symbols

[lints.clippy]
all = "deny"
pedantic = "warn"
nursery = "warn"
```

## Checklist

- [ ] No compiler warnings
- [ ] `cargo fmt` applied
- [ ] `cargo clippy` clean (no warnings)
- [ ] All tests pass
- [ ] Public APIs have rustdoc
- [ ] Error types use `thiserror`
- [ ] No `.unwrap()` in production code
- [ ] Unsafe code documented with safety invariants
- [ ] Feature flags for optional dependencies
- [ ] Semantic versioning
