# Changelog

## [0.3.1](https://github.com/matter-labs/anvil-zksync/compare/v0.3.0...v0.3.1) (2025-02-20)


### Features

* add telemetry ([#589](https://github.com/matter-labs/anvil-zksync/issues/589)) ([323687d](https://github.com/matter-labs/anvil-zksync/commit/323687d006decd1bfc88eb9321ef1745b129f7ac))
* adds abstract and abstract-testnet to named fork options ([#587](https://github.com/matter-labs/anvil-zksync/issues/587)) ([9774a3d](https://github.com/matter-labs/anvil-zksync/commit/9774a3d5864435134017c66563d7f209846c8653))
* implement basic L1 support and `anvil_zks_commitBatch` ([#575](https://github.com/matter-labs/anvil-zksync/issues/575)) ([ee49bb9](https://github.com/matter-labs/anvil-zksync/commit/ee49bb9434de823d682a4ba4558ff68f2f095c71))
* use rustls instead of openssl ([#581](https://github.com/matter-labs/anvil-zksync/issues/581)) ([1aa2217](https://github.com/matter-labs/anvil-zksync/commit/1aa22177c8057f740bbc58bc14edb023ad64dc60))
* zksync_error integration ([#583](https://github.com/matter-labs/anvil-zksync/issues/583)) ([055cd43](https://github.com/matter-labs/anvil-zksync/commit/055cd432d07202edfc5550edf86841fe165bdab7))


### Bug Fixes

* refrain from starting server during tx replay ([#588](https://github.com/matter-labs/anvil-zksync/issues/588)) ([6cb0925](https://github.com/matter-labs/anvil-zksync/commit/6cb092567d4fcaf34f5da23e087657e4f4cae9ab))
* update replay mode to refrain from starting server during tx replaying ([6cb0925](https://github.com/matter-labs/anvil-zksync/commit/6cb092567d4fcaf34f5da23e087657e4f4cae9ab))

## [0.3.0](https://github.com/matter-labs/anvil-zksync/compare/v0.2.5...v0.3.0) (2025-02-04)


### ⚠ BREAKING CHANGES

* upgrade to protocol v26 (gateway) ([#567](https://github.com/matter-labs/anvil-zksync/issues/567))
* **cli:** replay transactions when forking at a tx hash ([#557](https://github.com/matter-labs/anvil-zksync/issues/557))

### Features

* **cli:** replay transactions when forking at a tx hash ([#557](https://github.com/matter-labs/anvil-zksync/issues/557)) ([a955a9b](https://github.com/matter-labs/anvil-zksync/commit/a955a9bad062046d17aac47c0c5d86738af3f538))
* upgrade to protocol v26 (gateway) ([#567](https://github.com/matter-labs/anvil-zksync/issues/567)) ([94da53c](https://github.com/matter-labs/anvil-zksync/commit/94da53c8fab17423d7f4280f1df3139ee0d4db95))


### Bug Fixes

* add protocol version 26 ([#580](https://github.com/matter-labs/anvil-zksync/issues/580)) ([7465bc7](https://github.com/matter-labs/anvil-zksync/commit/7465bc7f50de819caf909907d129f5f3e575a159))
* make `anvil_reset` follow the same logic as main ([#569](https://github.com/matter-labs/anvil-zksync/issues/569)) ([0000e7d](https://github.com/matter-labs/anvil-zksync/commit/0000e7ddf3585c395b3e68e57dd29e0e6c294713))
