# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases and this changelog are generated automatically from
[Conventional Commits](https://www.conventionalcommits.org/) by release-please.

## [0.1.13](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.12...v0.1.13) (2026-10-26)


### Changed

* **apex-2023:** keep last-sent colour and threshold frames in one SentFrames store ([c570060](https://github.com/Ven0m0/steelseriesgg-rs/commit/c5700605308b89b06c8e60f8844d4f615dc631e6))

## [0.1.12](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.11...v0.1.12) (2026-10-25)


### Fixed

* **apex-pro-tkl-2023:** use GG's 40-step actuation table ([#311](https://github.com/Ven0m0/steelseriesgg-rs/issues/311)) ([f070eea](https://github.com/Ven0m0/steelseriesgg-rs/commit/f070eea1e1e57d994192bc00df3aa97e145f34e1))

## [0.1.11](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.10...v0.1.11) (2026-10-25)


### Changed

* update rust crate parking_lot to 0.13.0 ([d38d069](https://github.com/Ven0m0/steelseriesgg-rs/commit/d38d069cc8b0cb5bf3df5fbca74b17f3567b35b1))

## [0.1.10](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.9...v0.1.10) (2026-10-21)


### Added

* **apex-2023:** improve desktop app, add generic x86-64 compiling ([#307](https://github.com/Ven0m0/steelseriesgg-rs/issues/307)) ([689322f](https://github.com/Ven0m0/steelseriesgg-rs/commit/689322fdbb7a00739f7a404f31ec8314d5827920))

## [0.1.9](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.8...v0.1.9) (2026-10-21)


### Added

* **apex-2023:** keep unlisted keys on per-key frames, correct 1.0mm … ([#305](https://github.com/Ven0m0/steelseriesgg-rs/issues/305)) ([5b3bcfd](https://github.com/Ven0m0/steelseriesgg-rs/commit/5b3bcfd72086262a5bb693d49efd83a938338ecc))

## [0.1.8](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.7...v0.1.8) (2026-10-21)


### Added

* **gui:** live lighting panel served by ssgg gui ([#303](https://github.com/Ven0m0/steelseriesgg-rs/issues/303)) ([a23bed4](https://github.com/Ven0m0/steelseriesgg-rs/commit/a23bed40be78223aa3df9c00fcf4e0734c058743))
* onboard profile support for Apex 7 (supersedes [#301](https://github.com/Ven0m0/steelseriesgg-rs/issues/301)) ([#302](https://github.com/Ven0m0/steelseriesgg-rs/issues/302)) ([3d2ae40](https://github.com/Ven0m0/steelseriesgg-rs/commit/3d2ae40b00a717df960bd58656edb031fff50404))

## [0.1.7](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.6...v0.1.7) (2026-10-17)


### Changed

* update dependency rust to v1.99.1 ([a5700a0](https://github.com/Ven0m0/steelseriesgg-rs/commit/a5700a0f67cb564a0d53a0205f98e37167b71674))

## [0.1.6](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.5...v0.1.6) (2026-10-15)


### Added

* add scripts/capture-device.ps1 USBPcap helper for new-device captures ([dadc47a](https://github.com/Ven0m0/steelseriesgg-rs/commit/dadc47a885e88a0c2ebad597537bffda39eba75c))
* support wired Apex Pro TKL 2023 (0x1628) RGB lighting ([dadc47a](https://github.com/Ven0m0/steelseriesgg-rs/commit/dadc47a885e88a0c2ebad597537bffda39eba75c))

## [0.1.5](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.4...v0.1.5) (2026-08-26)


### Fixed

* **keyboards:** make RGB work on Apex Pro TKL Gen 3 (2024) ([#290](https://github.com/Ven0m0/steelseriesgg-rs/issues/290)) ([360b6c6](https://github.com/Ven0m0/steelseriesgg-rs/commit/360b6c6d7fc7fb6c9636a44ead65c08a1c67cccd))

## [0.1.4](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.3...v0.1.4) (2026-08-18)


### Fixed

* **build:** disable global sccache wrapper, unavailable in CI ([6d51f38](https://github.com/Ven0m0/steelseriesgg-rs/commit/6d51f38aa00aa85fd1dbfb5b7efad2a7cf443b73))

## [0.1.3](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.2...v0.1.3) (2026-08-18)


### Fixed

* **ci:** create tag and release on manual dispatch when tag is missing ([434a9cd](https://github.com/Ven0m0/steelseriesgg-rs/commit/434a9cd39b640029653e81cc5fcadeb6b54abf44))

## [0.1.2](https://github.com/Ven0m0/steelseriesgg-rs/compare/v0.1.1...v0.1.2) (2026-07-03)


### Changed

* fix synchronous file open blocking async diagnostics ([3cc60ef](https://github.com/Ven0m0/steelseriesgg-rs/commit/3cc60ef78c1655689ab28c536a4fcab82663e23a))

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
