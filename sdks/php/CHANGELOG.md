# Changelog

All notable changes to the Synap PHP SDK will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-10-23

### Added

**Core Features:**
- ✅ Complete Key-Value Store API with TTL support
- ✅ Message Queue API with ACK/NACK and priority
- ✅ Event Stream API with offset tracking
- ✅ Pub/Sub API with wildcard topic support
- ✅ StreamableHTTP protocol implementation

**Type Safety:**
- ✅ PHP 8.2+ strict types
- ✅ Full type hints for all parameters and returns
- ✅ Readonly classes for immutable data types
- ✅ Named arguments support

**Developer Experience:**
- ✅ PSR-4 autoloading
- ✅ PSR-12 code style compliance
- ✅ Comprehensive error handling
- ✅ Complete API documentation
- ✅ Working examples

**Quality Assurance:**
- ✅ PHPStan max level static analysis
- ✅ PHP-CS-Fixer for code style
- ✅ PHPUnit for testing
- ✅ Composer scripts for quality checks

### Dependencies

**Core:**
- php: ^8.2
- guzzlehttp/guzzle: ^7.8 (HTTP client)

**Development:**
- phpunit/phpunit: ^11.0 (testing)
- phpstan/phpstan: ^2.0 (static analysis)
- squizlabs/php_codesniffer: ^3.10 (code style)
- friendsofphp/php-cs-fixer: ^3.64 (formatting)

### Documentation

- Complete README with API reference
- Changelog (this file)
- Examples demonstrating all features
- Inline PHPDoc for all public methods

### Compatibility

- PHP: 8.2+
- Synap Server: v1.0.0+
- Platforms: Linux, macOS, Windows

[0.1.0]: https://github.com/hivellm/synap/releases/tag/php-sdk-v0.1.0

