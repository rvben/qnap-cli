# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).







## [0.1.7](https://github.com/rvben/qnap-cli/compare/v0.1.6...v0.1.7) - 2026-03-30

### Added

- shell completions, files ls --recursive, typed exit codes ([c2bdab4](https://github.com/rvben/qnap-cli/commit/c2bdab4c56a9302644ab817b17734d5d269e6de6))

## [0.1.6](https://github.com/rvben/qnap-cli/compare/v0.1.5...v0.1.6) - 2026-03-30

### Added

- add network command, recursive upload/download ([af3626d](https://github.com/rvben/qnap-cli/commit/af3626df896e1b8ae6a8fe80d9ec2a8eb635dee2))

## [0.1.5](https://github.com/rvben/qnap-cli/compare/v0.1.4...v0.1.5) - 2026-03-30

### Added

- add config show and files find commands ([b87ae54](https://github.com/rvben/qnap-cli/commit/b87ae5421c42956eb67afbcab7470341c46c27d8))

## [0.1.4](https://github.com/rvben/qnap-cli/compare/v0.1.3...v0.1.4) - 2026-03-30

## [0.1.3](https://github.com/rvben/qnap-cli/compare/v0.1.2...v0.1.3) - 2026-03-30

### Added

- **files**: batch rm, update README with all file commands ([24b1b17](https://github.com/rvben/qnap-cli/commit/24b1b17b194db959e56458e914a85e04c16abde1))

## [0.1.2](https://github.com/rvben/qnap-cli/compare/v0.1.1...v0.1.2) - 2026-03-30

### Added

- **files**: add mkdir, rm, mv, cp, upload, and download commands ([5225352](https://github.com/rvben/qnap-cli/commit/5225352ae3fd005407a1acef189f6b7d32debc70))

### Fixed

- **config**: rename config directory from qnap-cli to qnap ([cec4dfd](https://github.com/rvben/qnap-cli/commit/cec4dfd63e00ed1d1b12920090ef24a0ba98fc7e))
- **files**: fix upload, download, rm, and cp against live QNAP API ([8788607](https://github.com/rvben/qnap-cli/commit/878860795716aadcd7d6972050e1845c6a064019))

## [0.1.1] - 2026-03-30

### Added

- **dump**: anonymize sensitive data before saving fixtures ([bd2cc35](https://github.com/rvben/qnap-cli/commit/bd2cc35ef0ea2b20feb9e21750f31c4802f36dc7))
- add PyPI distribution via maturin ([550e1e0](https://github.com/rvben/qnap-cli/commit/550e1e039a2e80133c4bf9b9db531d6ec54de977))
- add dump command for compatibility fixture collection ([9247d4b](https://github.com/rvben/qnap-cli/commit/9247d4bf5fbd1aa40e6b1f53cc14a2e03f510987))
- **files**: add --all flag for paginated listing beyond 200 items ([4907454](https://github.com/rvben/qnap-cli/commit/4907454bfdffe595e60169fd5d08dba6f59b54ea))
- **config**: store password in OS keychain with env var fallback ([ec21974](https://github.com/rvben/qnap-cli/commit/ec21974c1e400aaf4aecdeecb4e47a2d1f1c5f4c))
- initial QNAP NAS management CLI ([10e66b6](https://github.com/rvben/qnap-cli/commit/10e66b6d904b07b0693fb8387beb8a05b61a45ac))

### Fixed

- volumes, info, and output formatting ([b06abfd](https://github.com/rvben/qnap-cli/commit/b06abfdcf1bb48b4bc68fac2539005468e2e6667))
- correct QNAP auth, API endpoints, and JSON field names ([e696b75](https://github.com/rvben/qnap-cli/commit/e696b75be8e968bac22ee6592273b9b817bda9dd))
