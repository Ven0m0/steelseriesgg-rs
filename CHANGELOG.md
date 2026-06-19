# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases and this changelog are generated automatically from
[Conventional Commits](https://www.conventionalcommits.org/) by release-please.

## [0.1.1](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.0...v0.1.1) (2026-06-19)


### Added

* **release:** adopt Keep a Changelog and automate releases with release-please ([1e2c81a](https://github.com/Ven0m0/steelseriesgg-rs/commit/1e2c81a2ee7cbd083bd9c4a7bbbbc42e73328aba))
* **release:** keep PKGBUILD pkgver in sync via release-please ([95774b7](https://github.com/Ven0m0/steelseriesgg-rs/commit/95774b71e19d6a776d3708d87770568659746c71))


### Fixed

* **ci:** address PR review on release automation ([efdb349](https://github.com/Ven0m0/steelseriesgg-rs/commit/efdb34983bdfd1aa49bd4fe01390daee3f52c31f))
* **ci:** push release-please tag with a PAT so release.yml triggers ([bdb29a4](https://github.com/Ven0m0/steelseriesgg-rs/commit/bdb29a4caae176cbee478127389a0d83878f880c))
* **deps:** update all dependencies ([aafd6dc](https://github.com/Ven0m0/steelseriesgg-rs/commit/aafd6dc6f143a7ddfb75f4b08906a97e5323dc5e))
* **pkgbuild:** reference $srcdir directly in build functions ([c27c030](https://github.com/Ven0m0/steelseriesgg-rs/commit/c27c030ecd357ff20df1d4eafe31a9e715cf66dd))
* **pkgbuild:** resolve cargo paths at build time, not parse time ([3f4fa96](https://github.com/Ven0m0/steelseriesgg-rs/commit/3f4fa96b2e5e0daa0e6b9ffe9966b41bcef0ff4a))
* remove needless borrow in write_core_props (clippy::needless_borrow) ([df80739](https://github.com/Ven0m0/steelseriesgg-rs/commit/df807390937f1c4141cc1a6a03981fdbad655c82))


### Documentation

* add database-schemas.md (merged engine + prism + sonar schemas) ([9054ce6](https://github.com/Ven0m0/steelseriesgg-rs/commit/9054ce6af104b2aa7d93fdb227882b47c78017bb))
* clarify unverified protocol status and zone mapping for Apex 3 TKL ([07d0139](https://github.com/Ven0m0/steelseriesgg-rs/commit/07d013966ed76f7254de07471f3f425edc49ef0f))
* consolidate database schemas into single reference ([32eb2c2](https://github.com/Ven0m0/steelseriesgg-rs/commit/32eb2c22b192033bddf700178d5b19e993fec7be))
* consolidate development docs from 10 files to 4 ([2d1f3e1](https://github.com/Ven0m0/steelseriesgg-rs/commit/2d1f3e10ebcbc991c68a8e4c1362366b40783fe7))
* escape lone backtick in zone map table cell ([5f33498](https://github.com/Ven0m0/steelseriesgg-rs/commit/5f33498b2b55696b9df8263162a50ed6444fb6f3))
* fix agnix errors and reduce warnings to 1 ([9ddf684](https://github.com/Ven0m0/steelseriesgg-rs/commit/9ddf6849a2f84ae6d863fa421519e1383df78518))
* fix agnix errors and reduce warnings to 1 ([58b8bd9](https://github.com/Ven0m0/steelseriesgg-rs/commit/58b8bd9301ad6d4fdb4f236de148a695e4451388))
* fix corrupted table separator rows in database-schemas.md ([cb17b17](https://github.com/Ven0m0/steelseriesgg-rs/commit/cb17b173beac9e93bf535d5d04ef2a8f758562f9))
* fix trailing pipe in table separators, clarify PID encoding; add git allowlist to project settings ([e309216](https://github.com/Ven0m0/steelseriesgg-rs/commit/e309216e73a7eb1b0a7cdbcc2b4f7a10e54c6165))
* prune stale tasks from TODO/PLAN, refocus on Apex 3 TKL RGB ([83f7704](https://github.com/Ven0m0/steelseriesgg-rs/commit/83f7704f5d4c5cd61e91611fb09f690e8befdadf))
* remove device-pid-registry.md (merged into devices.md) ([017a800](https://github.com/Ven0m0/steelseriesgg-rs/commit/017a80076f9c13d595a8d6b5cde28c55b309af52))
* remove device-registry.md (merged into devices.md) ([625458a](https://github.com/Ven0m0/steelseriesgg-rs/commit/625458af55ec1296ec98b2fcf0f0c7c3ef5c5789))
* remove engine-db-schema.md (merged into database-schemas.md) ([bcc439f](https://github.com/Ven0m0/steelseriesgg-rs/commit/bcc439f2bb28dfdeef1b1feb343ca74fe55b660b))
* remove gg-internal-api.md (GG internals ssgg doesn't use) ([2007f33](https://github.com/Ven0m0/steelseriesgg-rs/commit/2007f33397bdc8315d36d4d38834b05859ca1182))
* remove gg-reflection.txt (raw dump, superseded) ([4d8e2e9](https://github.com/Ven0m0/steelseriesgg-rs/commit/4d8e2e937ce71327a48df86ad17ec00d488836d3))
* remove preflight-findings.md (findings captured in other docs) ([9abd786](https://github.com/Ven0m0/steelseriesgg-rs/commit/9abd7864fe778dfc7391debf97f51f9e0d229adb))
* remove prism-schema.md (merged into database-schemas.md) ([167560a](https://github.com/Ven0m0/steelseriesgg-rs/commit/167560aa2dbb9c41fa2a407c6404be2d85e4fbb3))
* remove sonar-schema.md (merged into database-schemas.md) ([1aeac24](https://github.com/Ven0m0/steelseriesgg-rs/commit/1aeac24fdd09cddd157313c3fbf36c694c36d881))
* reorganize and expand AGENTS.md with structured module layout ([f504d5e](https://github.com/Ven0m0/steelseriesgg-rs/commit/f504d5ed0beb1b8ec9a52815e4f82e6d92c0daa9))
* update AGENTS.md with accurate file map and fix ctxlint errors ([67fb20a](https://github.com/Ven0m0/steelseriesgg-rs/commit/67fb20a834a37379fe132db4a9e74c82b1245e04))

## [0.1.0](https://github.com/Ven0m0/steelseriesgg-rs/releases/tag/v0.1.0) (2026-05-31)

Initial release of the open-source SteelSeries GG replacement for Linux.

### Added

- Type-safe HID report builder for talking to SteelSeries devices instead of
  hand-written byte arrays.
- RGB lighting control: colors, effects, per-key effects, and zone-to-HID
  mapping.
- Apex keyboard protocol implementation and headset protocol support.
- Device discovery with hot-plug detection and device fingerprinting.
- GameSense-compatible HTTP server on port 27301 with a localhost-only CORS
  policy.
- Profile save/load as TOML and `~/.config/ssgg/config.toml` configuration.
- Runtime diagnostics, RGB validation, and performance management.
- Optional `audio` feature: PulseAudio/PipeWire mixer.
- Optional `sonar` feature: SteelSeries Sonar HTTP integration.
- Experimental `experimental-apex-2023` feature for Apex Pro TKL 2023 direct
  per-key RGB (reverse-engineered, unverified on hardware).
- udev rules and a systemd user unit for installation.
