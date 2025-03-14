# Changelog

## [0.3.3](https://github.com/matter-labs/anvil-zksync/compare/v0.3.2...v0.3.3) (2025-03-14)


### Features

* introduces verbose actionable error messaging ([#592](https://github.com/matter-labs/anvil-zksync/issues/592)) ([690bf89](https://github.com/matter-labs/anvil-zksync/commit/690bf897fc3e22eace4b9d16e5b601bb3e35254a))
* make VM produce system logs ([#600](https://github.com/matter-labs/anvil-zksync/issues/600)) ([35e4a6c](https://github.com/matter-labs/anvil-zksync/commit/35e4a6c895155c46c4add7ae9f7facf29f0dd3ae))
* support L1 priority txs ([#606](https://github.com/matter-labs/anvil-zksync/issues/606)) ([c19092b](https://github.com/matter-labs/anvil-zksync/commit/c19092b55090279ca117e92a7855312cbfe07f23))
* support L2 to L1 logs ([#605](https://github.com/matter-labs/anvil-zksync/issues/605)) ([9903df9](https://github.com/matter-labs/anvil-zksync/commit/9903df988eae4299dd2749f128b8d5c5d4afcc11))


### Bug Fixes

* make `eth_sendTransaction` construct proper transactions ([#608](https://github.com/matter-labs/anvil-zksync/issues/608)) ([40723c9](https://github.com/matter-labs/anvil-zksync/commit/40723c93cc587bba060a14b8bba005f5fd9e4883))

## [0.3.2](https://github.com/matter-labs/anvil-zksync/compare/v0.3.1...v0.3.2) (2025-02-28)


### Features

* implement `anvil_zks_{prove,execute}Batch` ([#586](https://github.com/matter-labs/anvil-zksync/issues/586)) ([abbcf72](https://github.com/matter-labs/anvil-zksync/commit/abbcf72d0afbb662b5abbd621b4b959b6849e7ba))
* update zksync error setup ([#596](https://github.com/matter-labs/anvil-zksync/issues/596)) ([18cdc30](https://github.com/matter-labs/anvil-zksync/commit/18cdc3035fb00c9e47998c770331536d782315e3))


### Bug Fixes

* block `net_version` on a separate runtime ([#602](https://github.com/matter-labs/anvil-zksync/issues/602)) ([8ca721d](https://github.com/matter-labs/anvil-zksync/commit/8ca721daaaa6aa58caac9495f14d5db6aa0232ed))

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
