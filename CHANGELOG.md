# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

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
