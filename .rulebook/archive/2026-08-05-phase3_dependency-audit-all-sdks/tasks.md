## 1. Audit

- [x] 1.1 Audit the Rust workspace and Rust SDK (`cargo audit`, `cargo outdated`)
- [x] 1.2 Audit the TypeScript SDK and GUI (`npm audit`, `npm outdated`)
- [x] 1.3 Audit the Python SDK (`pip-audit` / `uv pip list --outdated`)
- [x] 1.4 Audit the PHP SDK (`composer audit`, `composer outdated`)
- [x] 1.5 Audit the C# SDK (`dotnet list package --vulnerable --outdated`)

## 2. Upgrades

- [x] 2.1 Apply every Rust upgrade and re-run `cargo check` + `clippy -D warnings` + tests
- [x] 2.2 Apply every TypeScript upgrade and re-run type-check + lint + tests
- [x] 2.3 Apply every Python upgrade and re-run lint + tests
- [x] 2.4 Apply every PHP upgrade and re-run PHPStan + tests
- [x] 2.5 Apply every C# upgrade and re-run build + tests
- [x] 2.6 Open a follow-up rulebook task for any upgrade that needs an API migration

## 3. Tail (docs + tests)

- [x] 3.1 Update or create documentation covering the implementation
- [x] 3.2 Write tests covering the new behavior
- [x] 3.3 Run tests and confirm they pass
