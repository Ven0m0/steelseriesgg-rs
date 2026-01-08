# Dependency Audit Report
**Project:** steelseries-gg-linux
**Date:** 2026-01-08
**Total Dependencies:** 274 crates

## Executive Summary

This audit identified 6 outdated root dependencies requiring major version updates, 1 security warning related to an optional dependency, and opportunities to reduce dependency bloat. The project is generally well-maintained with minimal critical security issues.

---

## 1. Outdated Packages

### Major Updates Available

| Package | Current | Latest | Impact | Priority |
|---------|---------|--------|--------|----------|
| **axum** | 0.7.9 | 0.8.8 | Breaking changes in web framework | High |
| **tower-http** | 0.5.2 | 0.6.8 | Must update with axum | High |
| **directories** | 5.0.1 | 6.0.0 | Path handling improvements | Medium |
| **thiserror** | 1.0.69 | 2.0.17 | Error handling macros | Medium |
| **toml** | 0.8.23 | 0.9.10 | Config parsing updates | Medium |
| **hidapi** | 2.6.3 | 2.6.4 | Intentionally pinned (see note) | Low |

**Note on hidapi:** Version 2.6.3 is intentionally pinned to avoid build issues with 2.6.4 requiring libudev-dev. This is acceptable for now.

### Minor Updates (Maintained)

The following packages have minor updates available and are well-maintained:
- tokio 1.38.0 → 1.49.0
- clap 4.5.54 (latest 4.5.x)
- serde 1.0.228 (latest 1.0.x)

---

## 2. Security Vulnerabilities

### ⚠️ Warning: Unmaintained Dependency

**Issue:** `rustls-pemfile 1.0.4` is unmaintained
**Advisory:** RUSTSEC-2025-0134
**Severity:** Low
**Impact:** Minimal - only affects optional `gamesense` dependency

**Dependency Chain:**
```
rustls-pemfile 1.0.4
└── reqwest 0.11.27
    └── gamesense 0.1.2 (optional)
        └── steelseries-gg-linux 0.1.0
```

**Recommendation:** Since `gamesense` is an optional compatibility feature, this is low risk. However, the `gamesense` crate itself may need updates or could be considered for removal if not actively used.

### No Critical Vulnerabilities
✅ No critical security vulnerabilities found in core dependencies

---

## 3. Dependency Bloat Analysis

### Total Dependency Count: 274 crates

This is moderate for a Rust project with web server and hardware interaction capabilities.

### Potential Bloat Areas

#### a) Optional Dependencies (Feature Gated)
- **gamesense** (0.1.2) - Compatibility crate for existing gamesense integration
- **steelseries-sonar** (0.1.0) - Compatibility crate for sonar integration
- **libpulse-binding** (2.28) - Audio support (feature gated)

**Analysis:** These optional dependencies are properly feature-gated and won't impact build times unless explicitly enabled. This is good design.

#### b) Core Web Stack
The axum + tower-http + tokio stack adds ~150+ transitive dependencies. This is typical for async Rust web frameworks and unavoidable.

#### c) Redundant Error Handling?
Both `anyhow` and `thiserror` are included:
- **thiserror:** For library error types (derive macros)
- **anyhow:** For application-level error handling

**Verdict:** Not redundant - these serve different purposes and complement each other well.

### Unnecessary Dependencies: None Found

All direct dependencies appear necessary for the stated functionality:
- ✅ **hidapi:** Hardware USB/HID communication
- ✅ **tokio:** Async runtime (required)
- ✅ **axum + tower-http:** HTTP server for GameSense API
- ✅ **serde + serde_json:** Serialization (required for API)
- ✅ **tracing:** Logging infrastructure
- ✅ **directories:** Cross-platform path resolution
- ✅ **toml:** Configuration parsing
- ✅ **clap:** CLI argument parsing
- ✅ **futures:** Async utilities

---

## 4. Recommendations

### Priority 1: Update Core Web Framework (Breaking Changes)
```toml
axum = "0.8"
tower-http = { version = "0.6", features = ["cors"] }
```

**Effort:** Medium
**Risk:** Medium - Breaking API changes
**Benefits:**
- Latest bug fixes and performance improvements
- Security updates
- Better long-term maintainability

**Action Required:** Review axum 0.8 migration guide and test thoroughly.

---

### Priority 2: Update Error Handling
```toml
thiserror = "2.0"
```

**Effort:** Low
**Risk:** Low - Minimal API changes
**Benefits:** Latest improvements to error derive macros

---

### Priority 3: Update Configuration Parsing
```toml
toml = "0.9"
```

**Effort:** Low
**Risk:** Low - Mostly backward compatible
**Benefits:** TOML 1.1.0 spec compliance

---

### Priority 4: Update Directories
```toml
directories = "6.0"
```

**Effort:** Low
**Risk:** Low
**Benefits:** Improved path handling on newer platforms

---

### Priority 5: Evaluate Optional Dependencies

**gamesense** and **steelseries-sonar** compatibility crates:

**Questions to consider:**
1. Are these actively used by your users?
2. Are they maintained?
3. Could their functionality be integrated directly?

**If not actively used:** Consider deprecating the `compat` feature in a future release.

**Security note:** The `gamesense` dependency pulls in an unmaintained `rustls-pemfile`. If this feature is rarely used, consider documenting this security caveat or removing it.

---

### Priority 6: Minor Version Updates

Update minor versions that don't require code changes:
```toml
tokio = { version = "1.49", features = ["rt-multi-thread", "macros"] }
```

---

## 5. Recommended Action Plan

### Phase 1: Low-Risk Updates (Do First)
1. ✅ Update `thiserror` to 2.0
2. ✅ Update `toml` to 0.9
3. ✅ Update `directories` to 6.0
4. ✅ Update `tokio` to 1.49
5. ✅ Test all functionality

### Phase 2: Breaking Changes (Requires Testing)
1. ✅ Update `axum` to 0.8 and `tower-http` to 0.6
2. ✅ Follow axum migration guide for breaking changes
3. ✅ Comprehensive testing of HTTP endpoints
4. ✅ Test GameSense API compatibility

### Phase 3: Dependency Cleanup (Optional)
1. ⚠️ Evaluate usage of `gamesense` and `steelseries-sonar` features
2. ⚠️ Consider deprecating or documenting security caveats
3. ⚠️ If keeping, reach out to crate maintainers about the rustls-pemfile issue

---

## 6. Build & Compile Time Analysis

**Current dependency count:** 274 crates is moderate.

**If compile times become an issue, consider:**
- Using `cargo-chef` for Docker builds
- Enabling incremental compilation in dev
- Using `sccache` or `mold` linker
- Feature-gating more optional functionality

**Current verdict:** No immediate bloat concerns. The dependency count is justified by the features provided.

---

## 7. Maintenance Score: B+

**Strengths:**
- ✅ Well-structured feature gates
- ✅ Intentional dependency pinning (hidapi) with documentation
- ✅ Modern async stack
- ✅ No critical security vulnerabilities
- ✅ Clear separation of concerns

**Areas for Improvement:**
- ⚠️ Some dependencies are 1-2 major versions behind
- ⚠️ Optional gamesense dependency has unmaintained transitive deps
- ⚠️ Need to stay current with axum ecosystem updates

---

## 8. Summary of Changes Needed

### Cargo.toml Updates
```toml
[dependencies]
hidapi = "=2.6.3" # Keep pinned

# Update these:
tokio = { version = "1.49", features = ["rt-multi-thread", "macros"] }
axum = "0.8"
tower-http = { version = "0.6", features = ["cors"] }
thiserror = "2.0"
directories = "6.0"
toml = "0.9"

# Keep as-is (already latest in their major version):
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4.5", features = ["derive"] }
futures = "0.3"

# Optional - consider deprecating or documenting security caveat:
gamesense = { version = "0.1.2", optional = true }
steelseries-sonar = { version = "0.1.0", optional = true }
```

---

## Conclusion

The project has a healthy dependency structure with no critical issues. The main recommendations are to update to latest major versions (especially axum 0.8) and evaluate the necessity of optional compatibility crates that have unmaintained transitive dependencies.

**Estimated effort:** 4-8 hours for testing and migration
**Risk level:** Medium (due to axum breaking changes)
**Recommended timeline:** 1-2 weeks for thorough testing
