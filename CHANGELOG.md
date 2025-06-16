# Changelog

## [0.6.5](https://github.com/matter-labs/anvil-zksync/compare/v0.6.4...v0.6.5) (2025-06-16)


### Bug Fixes

* linter warnings ([#733](https://github.com/matter-labs/anvil-zksync/issues/733)) ([aca4969](https://github.com/matter-labs/anvil-zksync/commit/aca496929564bea49f0d424e7b8846bedf323e7a))
* revert reasons not communicated to hardhat ([#731](https://github.com/matter-labs/anvil-zksync/issues/731)) ([74fe1fb](https://github.com/matter-labs/anvil-zksync/commit/74fe1fbe35e03c309e41632e081270d75fdbe81b))

## [0.6.4](https://github.com/matter-labs/anvil-zksync/compare/v0.6.3...v0.6.4) (2025-06-12)


### Features

* increase max transaction size ([#720](https://github.com/matter-labs/anvil-zksync/issues/720)) ([2b15674](https://github.com/matter-labs/anvil-zksync/commit/2b156749c5c94694ecba6ec26a1db8f7ccc3dff8))


### Bug Fixes

* adjusts system contract deps to split out non-kernal contracts ([#721](https://github.com/matter-labs/anvil-zksync/issues/721)) ([fe38245](https://github.com/matter-labs/anvil-zksync/commit/fe38245e8f7e67732f11b30f86521ef9d1dd64fd))
* correctly report halts and reverts during transaction cost estimation ([#716](https://github.com/matter-labs/anvil-zksync/issues/716)) ([e5bfb14](https://github.com/matter-labs/anvil-zksync/commit/e5bfb147f2db87fab1e0edfe9e4ae12661e788ea))
* respect quiet param ([#728](https://github.com/matter-labs/anvil-zksync/issues/728)) ([3f19ed6](https://github.com/matter-labs/anvil-zksync/commit/3f19ed65669a97b9f1182d0890a4c7edf8255918))
* return deployment nonce for contracts ([a88e17c](https://github.com/matter-labs/anvil-zksync/commit/a88e17c9324c94bbff7e602831dc2cd07e99e2c4))
* return deployment nonce for contracts in `eth_getTransactionCount` ([#710](https://github.com/matter-labs/anvil-zksync/issues/710)) ([a88e17c](https://github.com/matter-labs/anvil-zksync/commit/a88e17c9324c94bbff7e602831dc2cd07e99e2c4))

## [0.6.3](https://github.com/matter-labs/anvil-zksync/compare/v0.6.2...v0.6.3) (2025-05-21)


### Bug Fixes

* account for pubdata cost in estimation bounds ([#713](https://github.com/matter-labs/anvil-zksync/issues/713)) ([ab3cd44](https://github.com/matter-labs/anvil-zksync/commit/ab3cd44b6cc8fdcbe1a5281b211239dbbeb58fe1))
* preserve human-readable revert message ([#715](https://github.com/matter-labs/anvil-zksync/issues/715)) ([1a06611](https://github.com/matter-labs/anvil-zksync/commit/1a06611f32e6b43cefce98fe5d7517956c213e82))

## [0.6.2](https://github.com/matter-labs/anvil-zksync/compare/v0.6.1...v0.6.2) (2025-05-20)


### Features

* **boojum-os:** Updated boojum os support ([#702](https://github.com/matter-labs/anvil-zksync/issues/702)) ([eca0fec](https://github.com/matter-labs/anvil-zksync/commit/eca0fececf69a9d1bdb30440245807ec2b8c7860))
* Support setting EVM bytecode ([#707](https://github.com/matter-labs/anvil-zksync/issues/707)) ([7195c0f](https://github.com/matter-labs/anvil-zksync/commit/7195c0f7bcedd283e5b8832c271624d4e0730968))
* Support state overrides in eth_call ([#706](https://github.com/matter-labs/anvil-zksync/issues/706)) ([590335f](https://github.com/matter-labs/anvil-zksync/commit/590335f5f8ee0ce2028f41de7230ac6858702c01))


### Bug Fixes

* address timeout issue in CI ([#705](https://github.com/matter-labs/anvil-zksync/issues/705)) ([543096e](https://github.com/matter-labs/anvil-zksync/commit/543096e082de9f316a9d0b8c19b45326c8c1175d))
* set call request's nonce during estimation ([#711](https://github.com/matter-labs/anvil-zksync/issues/711)) ([923a708](https://github.com/matter-labs/anvil-zksync/commit/923a70821df91ca5505288c8a671990bdbf62344))
* Update nonces for accounts during impersonation ([#708](https://github.com/matter-labs/anvil-zksync/issues/708)) ([b6b0d69](https://github.com/matter-labs/anvil-zksync/commit/b6b0d69394f03de270323fdc11a4339c14d5a4ae))

## [0.6.1](https://github.com/matter-labs/anvil-zksync/compare/v0.6.0...v0.6.1) (2025-05-05)


### Features

* use V28 multivm ([#645](https://github.com/matter-labs/anvil-zksync/issues/645)) ([4789878](https://github.com/matter-labs/anvil-zksync/commit/478987815fa207a336399024809274ebf3b75ea9))


### Bug Fixes

* addresses issue with computing sha for binaries for homebrew installation ([#694](https://github.com/matter-labs/anvil-zksync/issues/694)) ([88e8402](https://github.com/matter-labs/anvil-zksync/commit/88e8402d2fd2c2ce27557147e0b8817225a268ca))
* fix sha mismatch from workflow ([#689](https://github.com/matter-labs/anvil-zksync/issues/689)) ([bea090e](https://github.com/matter-labs/anvil-zksync/commit/bea090ef000ebeaf24ca60223e715e3ef4468e36))
* update docker buildx for proper containers caching ([#691](https://github.com/matter-labs/anvil-zksync/issues/691)) ([0f6a1ab](https://github.com/matter-labs/anvil-zksync/commit/0f6a1abb028a404f77c5913e1bfb134bed4c5063))

## [0.6.0](https://github.com/matter-labs/anvil-zksync/compare/v0.5.1...v0.6.0) (2025-04-30)


### ⚠ BREAKING CHANGES

* drop unused formatting CLI options ([#666](https://github.com/matter-labs/anvil-zksync/issues/666))

### Features

* add homebrew formula for improved installation ([#672](https://github.com/matter-labs/anvil-zksync/issues/672)) ([32eec61](https://github.com/matter-labs/anvil-zksync/commit/32eec6170c3c838d13c88474fb7d68bbe5666876))
* adds anvil-zksync docs site ([#671](https://github.com/matter-labs/anvil-zksync/issues/671)) ([b1288b8](https://github.com/matter-labs/anvil-zksync/commit/b1288b80e19a14bcb0e46d4fd47a5e268ecf2576))
* adds pre-deployed contracts for evm mode ([#682](https://github.com/matter-labs/anvil-zksync/issues/682)) ([887d6c2](https://github.com/matter-labs/anvil-zksync/commit/887d6c2af46d51d74c5dfda9d952ba2a36b9973f))
* drop unused formatting CLI options ([#666](https://github.com/matter-labs/anvil-zksync/issues/666)) ([c13f3f0](https://github.com/matter-labs/anvil-zksync/commit/c13f3f06bee78e2e7b9c094877c159fb691a61ec))
* introduces additional ZK named chains ([#684](https://github.com/matter-labs/anvil-zksync/issues/684)) ([e3722ce](https://github.com/matter-labs/anvil-zksync/commit/e3722ce9fcfba161e2e27cc9a4457d45d3d186b7))
* print traces for unexecutable transactions during gas estimation ([#687](https://github.com/matter-labs/anvil-zksync/issues/687)) ([9bdf5ad](https://github.com/matter-labs/anvil-zksync/commit/9bdf5ad027d88eb573113fcc85841c873ff31d3f))
* show tx effect on ETH balances ([#656](https://github.com/matter-labs/anvil-zksync/issues/656)) ([6c6ae1e](https://github.com/matter-labs/anvil-zksync/commit/6c6ae1e87722fcf8755fbb11e8b1df397a4e806f))
* support custom base token symbol/ratio ([#669](https://github.com/matter-labs/anvil-zksync/issues/669)) ([0cde9c1](https://github.com/matter-labs/anvil-zksync/commit/0cde9c1ca8d6955f3abce59bc1a26b0157dcc43b))
* update zksync-error version to support overrides ([#679](https://github.com/matter-labs/anvil-zksync/issues/679)) ([a940e8c](https://github.com/matter-labs/anvil-zksync/commit/a940e8c68266628204e9296817dec7bd2e21ece8))


### Bug Fixes

* respect zero value in local storage ([#685](https://github.com/matter-labs/anvil-zksync/issues/685)) ([fd892f3](https://github.com/matter-labs/anvil-zksync/commit/fd892f356e391ff06fa7a4e5129486b8c4837053))
* show balance changes only with verbose outputs ([#683](https://github.com/matter-labs/anvil-zksync/issues/683)) ([a1c076e](https://github.com/matter-labs/anvil-zksync/commit/a1c076eb07a48f847b37d30c4920027e09e58ccb))
* sophon testnet url typo ([#668](https://github.com/matter-labs/anvil-zksync/issues/668)) ([8bfe731](https://github.com/matter-labs/anvil-zksync/commit/8bfe7312180c86953318439f5dc9719870a53769))

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
