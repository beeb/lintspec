# Changelog

All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

## [0.5.0] - 2025-04-19

### Changed

- **(definitions)** create own text range type ([#70](https://github.com/beeb/lintspec/pull/70)) - ([e50f927](https://github.com/beeb/lintspec/commit/e50f927681ba6d838e391b84d079a83ba2e4bf25))

- **(deps)** bump actions/create-github-app-token from 1 to 2 ([#68](https://github.com/beeb/lintspec/pull/68)) - ([e109dc6](https://github.com/beeb/lintspec/commit/e109dc603b1a6054b63d0ee38bf1d59ea3f07b6a))

- **(deps)** update dependencies ([#71](https://github.com/beeb/lintspec/pull/71)) - ([619c3d9](https://github.com/beeb/lintspec/commit/619c3d92e8e73efcb4aa873ff347e2f6cc783e8e))


-  [**breaking**] allow to skip solidity version detection ([#67](https://github.com/beeb/lintspec/pull/67)) - ([d087faa](https://github.com/beeb/lintspec/commit/d087faaebfb485b8abc18d9fd83258f1d55e904f))
-  update runner image for dist ([#72](https://github.com/beeb/lintspec/pull/72)) - ([b4ddd0f](https://github.com/beeb/lintspec/commit/b4ddd0fd6e336aaa7da72544e221af55caba83f9))

### Documentation


-  pin action version in readme ([#63](https://github.com/beeb/lintspec/pull/63)) - ([dc97ea4](https://github.com/beeb/lintspec/commit/dc97ea456b503e8e258aa7c3ed723b66528491a3))
-  fix example for action extra args ([#65](https://github.com/beeb/lintspec/pull/65)) - ([7970bc2](https://github.com/beeb/lintspec/commit/7970bc2dc27882f5752e3d493c3f63d4f894972f))

### Fixed

- **(slang)** normalize the span for definitions ([#66](https://github.com/beeb/lintspec/pull/66)) - ([314ba71](https://github.com/beeb/lintspec/commit/314ba711a06db426a0395daf56e2c8eb1b22ef13))






**Full Changelog**: [0.4.1...0.5.0](https://github.com/beeb/lintspec/compare/v0.4.1...v0.5.0)

## [0.4.1] - 2025-03-19

### Changed


-  update readme benchmark ([#59](https://github.com/beeb/lintspec/pull/59)) - ([a27384a](https://github.com/beeb/lintspec/commit/a27384a9dc280940a6987b6266f03a117747cee1))
-  contributing guide ([#60](https://github.com/beeb/lintspec/pull/60)) - ([93e52d7](https://github.com/beeb/lintspec/commit/93e52d7379c7aa7990af478b45facca6b2bfc863))

### Documentation


-  add missing documentation ([#57](https://github.com/beeb/lintspec/pull/57)) - ([013d524](https://github.com/beeb/lintspec/commit/013d5244c3719be81330393f1d4ec6dc3e8a02ab))

### Fixed


-  undesirable print-out of config ([#62](https://github.com/beeb/lintspec/pull/62)) - ([d09bc4e](https://github.com/beeb/lintspec/commit/d09bc4e9841cc1bbf56596303de72174bc023e85))




**Full Changelog**: [0.4.0...0.4.1](https://github.com/beeb/lintspec/compare/v0.4.0...v0.4.1)

## [0.4.0] - 2025-03-13

### Added


-  add cargo deny ([#49](https://github.com/beeb/lintspec/pull/49)) - ([d144a9c](https://github.com/beeb/lintspec/commit/d144a9c4b56417123313c7d530b4895c74639e69))

### Changed


-  update readme ([#51](https://github.com/beeb/lintspec/pull/51)) - ([79a7638](https://github.com/beeb/lintspec/commit/79a763877f7b5750322b93861b7ab5381e162e22))
-  [**breaking**] separate parsing from validation ([#54](https://github.com/beeb/lintspec/pull/54)) - ([9cdd90d](https://github.com/beeb/lintspec/commit/9cdd90d69d469c15ac539ec499e82ee96dc29c6e))
-  [**breaking**] granular configuration ([#56](https://github.com/beeb/lintspec/pull/56)) - ([e286013](https://github.com/beeb/lintspec/commit/e286013090873fda3e14347b3cdc6198b9761e18))

### Removed


-  remove deprecated feature ([#53](https://github.com/beeb/lintspec/pull/53)) - ([8cfc11f](https://github.com/beeb/lintspec/commit/8cfc11f4bfe077967365c517c7c0eb3daded8984))




**Full Changelog**: [0.3.0...0.4.0](https://github.com/beeb/lintspec/compare/v0.3.0...v0.4.0)

## [0.3.0] - 2025-02-25

### Added

- **(config)** add `enforce-all` flag (#44) - ([7373f00](https://github.com/beeb/lintspec/commit/7373f00a2efecc4dd5a1528b6c817e77c8c993ca))


-  add option to sort results by file path (#41) - ([3ca29cc](https://github.com/beeb/lintspec/commit/3ca29cc3d7a5c2a4d0f5dbb522962c7c75686bc9))

### Changed

- **(definitions)** [**breaking**] conversion functions (#47) - ([7a5742d](https://github.com/beeb/lintspec/commit/7a5742d811fd38f2fbf8270d6f09d42a29f17f75))

- **(natspec)** parser improvements to support weird edge-cases (#39) - ([97b30c9](https://github.com/beeb/lintspec/commit/97b30c93b29262b2308441856624bc00cfa7f31a))


-  allow enforcing natspec on specific items (#42) - ([7d9a5b3](https://github.com/beeb/lintspec/commit/7d9a5b33364981a3478b47b4789f64d1740e0b2d))
-  [**breaking**] apply more lints from clippy (#45) - ([9c7dad9](https://github.com/beeb/lintspec/commit/9c7dad9cef3a1a26a3b6e4c825c88bc65e535564))
-  update changelog format (#48) - ([be7ab2b](https://github.com/beeb/lintspec/commit/be7ab2b84b9f35ad8a7544bb67a1f796a75baf85))

### Documentation


-  update readme (#43) - ([05a147a](https://github.com/beeb/lintspec/commit/05a147a5cb9207a51a619d1a05a7c909d6ad3fcd))
-  add documentation (#46) - ([353d659](https://github.com/beeb/lintspec/commit/353d659b42bedc35cae9ca08a727a05a167c20e7))

### Fixed

- **(config)** parsing of the struct-params arg (#40) - ([861f472](https://github.com/beeb/lintspec/commit/861f4724cf7d99f13d1ac1b72ecd0901e20544d3))

- **(definitions)** comments filtering (#35) - ([dc25919](https://github.com/beeb/lintspec/commit/dc25919c4186285ba46d0036ac8ada07eb0036e5))


-  natspec parser and function returns validation (#37) - ([f1c0d5f](https://github.com/beeb/lintspec/commit/f1c0d5fc7ac0488a815b6d5374cf2fb49102ef78))




**Full Changelog**: [0.2.0...0.3.0](https://github.com/beeb/lintspec/compare/v0.2.0...v0.3.0)

## [0.2.0] - 2025-02-21

### Changed

- **(cli)** [**breaking**] make documenting struct members optional (#32) - ([ee02f55](https://github.com/beeb/lintspec/commit/ee02f55410e5ea779f68e7aaf55b2274bcc5f9df))

- **(definitions)** replace macro with function (#24) - ([7b6ec13](https://github.com/beeb/lintspec/commit/7b6ec1308b66db00905ca094caaa6f3ce03e3a04))


-  [**breaking**] make most structs non exhaustive and add builders (#25) - ([f491e0d](https://github.com/beeb/lintspec/commit/f491e0d01dc83b0bb56db29eda675653d43cb421))
-  modify changelog format (#26) - ([f945663](https://github.com/beeb/lintspec/commit/f945663293ddff9f49fba413e45e06d83921a5aa))
-  typo in template (#29) - ([303bd7b](https://github.com/beeb/lintspec/commit/303bd7b55fa4b0d4077212dbf758d6c73951c384))

### Fixed

- **(definitions)** parse modifier without params (#22) - ([4a335de](https://github.com/beeb/lintspec/commit/4a335deff8057a4a80908135f921808880c17d00))

- **(definitions)** duplicate results for queries with quantifiers (#34) - ([c68040e](https://github.com/beeb/lintspec/commit/c68040e0f0c333203e631ebe5cfdebdd226cd62d))


-  fix git-cliff template (#27) - ([f2fa658](https://github.com/beeb/lintspec/commit/f2fa65827c44fa02d45476b0bba7a82458a15ec0))
-  fix template (#28) - ([ea014bc](https://github.com/beeb/lintspec/commit/ea014bcd05e47edbe5a61e4d4301c4a4c27d7d07))
-  fix tags in template (#30) - ([99a5961](https://github.com/beeb/lintspec/commit/99a5961a8c2efa6fd36e04071a778ae890e22a08))
-  various validation logic fixes and add a bunch of tests (#33) - ([2ff8f1f](https://github.com/beeb/lintspec/commit/2ff8f1fe510ff2428e2ba17769f76b78dfcced0b))




**Full Changelog**: [0.1.6...0.2.0](https://github.com/beeb/lintspec/compare/v0.1.6...v0.2.0)

## [0.1.6] - 2025-02-20

### Added

- Add github action (#15) - ([05431f6](https://github.com/beeb/lintspec/commit/05431f6ca93fa59333a927a7ccb0dc6aeb5ccb4a))
- Add tests (#21) - ([bf7e4fc](https://github.com/beeb/lintspec/commit/bf7e4fc15eff6ca32ea007fde5a0e85351b38d9f))

### Fixed

- Parent ignore files must be enabled (#17) - ([6d9663e](https://github.com/beeb/lintspec/commit/6d9663e68d00a4cbe3a734e71e87a8f140613e90))
- Return comment should not include return name (#19) - ([8be4150](https://github.com/beeb/lintspec/commit/8be4150d45039a7eda95f135ee4d34eb73bbe0a9))
- Process functions without returns (#20) - ([7fdfbda](https://github.com/beeb/lintspec/commit/7fdfbda4856eeb94ac29cca9cb4cd6b2332842d5))

## [0.1.5] - 2025-02-20

### Fixed

- Output a line return after json output (#12) - ([89f616f](https://github.com/beeb/lintspec/commit/89f616ff029229ecc49a89ea443e0dfe9bdebcb7))
- Merging of config from CLI and config file (#14) - ([bbef59a](https://github.com/beeb/lintspec/commit/bbef59ad3d68031d0258dca8ad47d11edc9c229d))

## [0.1.4] - 2025-02-20

### Documentation

- Fix help text (#9) - ([6f3025e](https://github.com/beeb/lintspec/commit/6f3025e95fc2f377ac5a7d66f157584ac9040125))
- Update readme (#11) - ([a123795](https://github.com/beeb/lintspec/commit/a12379584155c2822168962d7f64ed7af8f03048))

## [0.1.3] - 2025-02-20

### Added

- Add icon to title (#6) - ([0c0f300](https://github.com/beeb/lintspec/commit/0c0f300b717771832e6d41ae0c9a1149e70b54cb))
- Add cargo-dist (#7) - ([fe9a65e](https://github.com/beeb/lintspec/commit/fe9a65e81b37a5b50ec8c2db2cc1dbac4b03d5e3))

### Changed

- Change phrasing - ([71aa36e](https://github.com/beeb/lintspec/commit/71aa36e0421c5d83380ca41d1fe99857244e8c91))

### Fixed

- Highlighted span (#8) - ([e53840f](https://github.com/beeb/lintspec/commit/e53840f7614e018569dfc8be18701b9000709d82))

## [0.1.2] - 2025-02-19

### Added

- Add badges - ([ea6b8a6](https://github.com/beeb/lintspec/commit/ea6b8a61514466ab34ec0343d97d3b20da35da45))

### Changed

- Update benchmark with comparable output format - ([1b0bc92](https://github.com/beeb/lintspec/commit/1b0bc9291b866921bba35778fd8466459146cef8))

### Fixed

- Parsing of the `--out` CLI arg (#3) - ([b0de154](https://github.com/beeb/lintspec/commit/b0de154ed7fca881c737f54d20c80617fff5a9cd))
- Fix links - ([95d8585](https://github.com/beeb/lintspec/commit/95d85857f729c11ce68740754d6144823ed152e7))

## [0.1.1] - 2025-02-19

### Added

- Add workflows (#1) - ([e06b444](https://github.com/beeb/lintspec/commit/e06b4449649758565bc6cc5a064fb2117cf96dc1))

### Changed

- Update readme - ([2369db0](https://github.com/beeb/lintspec/commit/2369db044f00813db0685f4e1b38253d87df6c3d))

