# Changelog

## [0.5.1](https://github.com/matter-labs/anvil-zksync/compare/v0.5.0...v0.5.1) (2025-04-11)


### Bug Fixes

* crossbeam-channel dep from rust advisory ([#663](https://github.com/matter-labs/anvil-zksync/issues/663)) ([5c50303](https://github.com/matter-labs/anvil-zksync/commit/5c503037d8a07a64a89abf036d4d55344904b5d1))
* fix revert messages being appended to every call ([#665](https://github.com/matter-labs/anvil-zksync/issues/665)) ([94ba824](https://github.com/matter-labs/anvil-zksync/commit/94ba824f0b48e428925ae881b368052672d87abc))

## [0.5.0](https://github.com/matter-labs/anvil-zksync/compare/v0.4.0...v0.5.0) (2025-04-09)


### ⚠ BREAKING CHANGES

* support both V26 and V27 at the same time ([#636](https://github.com/matter-labs/anvil-zksync/issues/636))

### Features

* add cli option for local system contracts usage ([#659](https://github.com/matter-labs/anvil-zksync/issues/659)) ([d47a8eb](https://github.com/matter-labs/anvil-zksync/commit/d47a8ebce7c82c4301b8eefd5380f16d0b954c91))
* full support for EVM emulator for protocol V27 and higher ([#644](https://github.com/matter-labs/anvil-zksync/issues/644)) ([b99e7e2](https://github.com/matter-labs/anvil-zksync/commit/b99e7e2bcc5151e3648a885832102f2d275e7a37))
* pack built-in contract into .tar.gz archive ([#635](https://github.com/matter-labs/anvil-zksync/issues/635)) ([c3f5a5f](https://github.com/matter-labs/anvil-zksync/commit/c3f5a5ff7bad9e0f3ab76a039cff54a61180203a))
* progress reporting replaying of txs ([#642](https://github.com/matter-labs/anvil-zksync/issues/642)) ([50c1dc1](https://github.com/matter-labs/anvil-zksync/commit/50c1dc1dbbd05f6649e53849c68ff794c3764fc2))
* support automatic execution of L1 batches ([#647](https://github.com/matter-labs/anvil-zksync/issues/647)) ([da51724](https://github.com/matter-labs/anvil-zksync/commit/da51724279efa7aefe5bb7da78108489f135679c))
* support both V26 and V27 at the same time ([#636](https://github.com/matter-labs/anvil-zksync/issues/636)) ([8901610](https://github.com/matter-labs/anvil-zksync/commit/890161094098d3c7fe744723ea12d000a92d1b63))
* support protocol V28 ([#637](https://github.com/matter-labs/anvil-zksync/issues/637)) ([bdec93c](https://github.com/matter-labs/anvil-zksync/commit/bdec93c4e96c3b17d6666b547c495be6f42f661d))
* support replay of L1 and upgrade transactions ([#638](https://github.com/matter-labs/anvil-zksync/issues/638)) ([f1d7f21](https://github.com/matter-labs/anvil-zksync/commit/f1d7f215b47d440860af0fcb465fbe61bdd1af7a))
* upgrade to v27 protocol ([#627](https://github.com/matter-labs/anvil-zksync/issues/627)) ([0273215](https://github.com/matter-labs/anvil-zksync/commit/02732158b212a5cb408434b6aa445101de123f21))
* warn of transactions and calls to non-assigned addresses ([#652](https://github.com/matter-labs/anvil-zksync/issues/652)) ([0ab3915](https://github.com/matter-labs/anvil-zksync/commit/0ab3915c3b496c3504b19ae2f815ef28d36906a1))


### Bug Fixes

* allow wider `anvil` versions ([#639](https://github.com/matter-labs/anvil-zksync/issues/639)) ([7ffbd3d](https://github.com/matter-labs/anvil-zksync/commit/7ffbd3d4aa11a3a110430ed0dba3ca6d9082ef73))
* make sure `eth_getStorageAt` works with historic blocks ([#653](https://github.com/matter-labs/anvil-zksync/issues/653)) ([28316a3](https://github.com/matter-labs/anvil-zksync/commit/28316a31724d11d30af1ae9cf5a1ebe5118d0030))
* safe usize to u32 conversion for pubdata limit ([f1a0522](https://github.com/matter-labs/anvil-zksync/commit/f1a05225188026b55cef292e3fb240941cc4c308))
* usize to u32 conversion for pubdata limit ([#650](https://github.com/matter-labs/anvil-zksync/issues/650)) ([f1a0522](https://github.com/matter-labs/anvil-zksync/commit/f1a05225188026b55cef292e3fb240941cc4c308))

## [0.4.0](https://github.com/matter-labs/anvil-zksync/compare/v0.3.2...v0.4.0) (2025-03-24)


### ⚠ BREAKING CHANGES

* support L1-L2 communication ([#630](https://github.com/matter-labs/anvil-zksync/issues/630))

### Features

* add cli option for bytecode compression ([#622](https://github.com/matter-labs/anvil-zksync/issues/622)) ([db06412](https://github.com/matter-labs/anvil-zksync/commit/db0641261a076dee00fcf0468dd22a6e2cf1c073))
* introduces verbose actionable error messaging ([#592](https://github.com/matter-labs/anvil-zksync/issues/592)) ([690bf89](https://github.com/matter-labs/anvil-zksync/commit/690bf897fc3e22eace4b9d16e5b601bb3e35254a))
* introduces verbosity cli option with foundry-style traces ([#577](https://github.com/matter-labs/anvil-zksync/issues/577)) ([e7223ff](https://github.com/matter-labs/anvil-zksync/commit/e7223ffca2a3dd4e9034af13e30121045eaa8c90))
* make VM produce system logs ([#600](https://github.com/matter-labs/anvil-zksync/issues/600)) ([35e4a6c](https://github.com/matter-labs/anvil-zksync/commit/35e4a6c895155c46c4add7ae9f7facf29f0dd3ae))
* support both spawned and external L1s ([#616](https://github.com/matter-labs/anvil-zksync/issues/616)) ([593f579](https://github.com/matter-labs/anvil-zksync/commit/593f57904edecea1a056c34d578f269ba86e56a5))
* support L1 priority txs ([#606](https://github.com/matter-labs/anvil-zksync/issues/606)) ([c19092b](https://github.com/matter-labs/anvil-zksync/commit/c19092b55090279ca117e92a7855312cbfe07f23))
* support L1-L2 communication ([#630](https://github.com/matter-labs/anvil-zksync/issues/630)) ([7e779f7](https://github.com/matter-labs/anvil-zksync/commit/7e779f7cad623eb91b01f90456b1226e6afc1286))
* support L2 to L1 logs ([#605](https://github.com/matter-labs/anvil-zksync/issues/605)) ([9903df9](https://github.com/matter-labs/anvil-zksync/commit/9903df988eae4299dd2749f128b8d5c5d4afcc11))


### Bug Fixes

* make `eth_sendTransaction` construct proper transactions ([#608](https://github.com/matter-labs/anvil-zksync/issues/608)) ([40723c9](https://github.com/matter-labs/anvil-zksync/commit/40723c93cc587bba060a14b8bba005f5fd9e4883))
* update compression flag and rename to enforce_bytecode_compression ([#624](https://github.com/matter-labs/anvil-zksync/issues/624)) ([f623e72](https://github.com/matter-labs/anvil-zksync/commit/f623e722661b7ff7c986fe54b5ad5e45e0a083d5))

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
