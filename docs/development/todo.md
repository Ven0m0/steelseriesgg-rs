# Development Enhancement Proposals

> **Status**: Reference document for potential future improvements
> **Current Implementation**: Using hidapi for device discovery (working well)

## Enhancement Idea: Alternative Device Discovery Backends

### Current Implementation

The project currently uses `hidapi` for device discovery and communication, which provides:
- ✅ Cross-platform support (Linux, Windows, macOS)
- ✅ Simple, stable API
- ✅ Direct HID device access
- ✅ Well-maintained and widely used

### Proposed Enhancement: Multi-Backend Support

Add optional backends for specialized use cases:

#### Option 1: udev (Linux-specific)

**Use Case**: More detailed device information on Linux

```rust
use udev::{Enumerator, Context};

fn discover_via_udev() -> anyhow::Result<Vec<DeviceInfo>> {
    let ctx = Context::new()?;
    let mut en = Enumerator::new(&ctx)?;
    en.match_subsystem("usb")?;

    for device in en.scan_devices()? {
        if let (Some(vid_s), Some(pid_s)) = (
            device.property_value("ID_VENDOR_ID"),
            device.property_value("ID_MODEL_ID"),
        ) {
            let vid = u16::from_str_radix(&vid_s.to_string_lossy(), 16)?;
            let pid = u16::from_str_radix(&pid_s.to_string_lossy(), 16)?;
            println!("{:04x}:{:04x}", vid, pid);
        }
    }
    Ok(())
}
```

**Pros**:
- Access to additional device properties (serial, manufacturer, etc.)
- Better integration with Linux udev rules
- Can monitor device hotplug events

**Cons**:
- Linux-only (not cross-platform)
- Additional dependency
- More complex implementation

#### Option 2: rusb (Cross-platform USB)

**Use Case**: Direct USB access for advanced features

```rust
use rusb;

fn discover_via_rusb() -> rusb::Result<Vec<DeviceInfo>> {
    let mut devices = Vec::new();
    for device in rusb::devices()?.iter() {
        let desc = device.device_descriptor()?;
        devices.push((desc.vendor_id(), desc.product_id()));
    }
    Ok(devices)
}
```

**Pros**:
- Cross-platform (Linux, Windows, macOS)
- Lower-level USB access
- Can access USB descriptors directly

**Cons**:
- More complex than hidapi
- Requires libusb on all platforms
- May need elevated permissions

### Recommendation

**Priority**: LOW (Enhancement, not critical)

**Current hidapi implementation is sufficient** for the project's needs. Consider this enhancement only if:
1. Need platform-specific optimizations
2. Require advanced USB features not available in hidapi
3. Want to provide users with backend choice

**Implementation Approach** (if pursued):
```rust
// Feature-gated backends
#[cfg(feature = "udev-backend")]
mod udev_discovery;

#[cfg(feature = "rusb-backend")]
mod rusb_discovery;

// Default to hidapi
#[cfg(not(any(feature = "udev-backend", feature = "rusb-backend")))]
mod hidapi_discovery; // Current implementation
```

---

**Document Status**: Reference for future consideration
**Action Required**: None (current implementation working well)
**Last Updated**: 2026-02-10
