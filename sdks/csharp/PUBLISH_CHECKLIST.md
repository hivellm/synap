# Publishing Checklist for Synap C# SDK

This checklist ensures the SDK is ready for publication to NuGet.

## Pre-Publish Checklist

### Code Quality

- [ ] All tests passing (100%)
  ```bash
  dotnet test
  ```

- [ ] Code formatted
  ```bash
  dotnet format --verify-no-changes
  ```

- [ ] No build warnings
  ```bash
  dotnet build --configuration Release
  ```

- [ ] Documentation XML generated
  - Check `bin/Release/net8.0/Synap.SDK.xml` exists

### Documentation

- [ ] README.md up to date
  - [ ] Installation instructions correct
  - [ ] Quick start example works
  - [ ] API examples are current
  - [ ] Links are valid

- [ ] CHANGELOG.md updated
  - [ ] New version documented
  - [ ] All changes listed
  - [ ] Release date set

- [ ] LICENSE file present and correct

### Package Metadata

- [ ] Version number updated in `Synap.SDK.csproj`
- [ ] Package tags appropriate
- [ ] Package description accurate
- [ ] Repository URL correct
- [ ] Authors updated

### Testing

- [ ] All unit tests pass
  ```bash
  dotnet test --configuration Release
  ```

- [ ] Example project runs successfully
  ```bash
  cd examples/BasicUsage
  dotnet run
  ```

- [ ] Integration tests pass (if available)

### Build & Pack

- [ ] Clean build succeeds
  ```bash
  dotnet clean
  dotnet build --configuration Release
  ```

- [ ] Package creates successfully
  ```bash
  dotnet pack --configuration Release
  ```

- [ ] Inspect package contents
  ```bash
  # Extract and verify .nupkg contents
  ```

## Publishing Steps

### 1. Update Version

Update version in `src/Synap.SDK/Synap.SDK.csproj`:

```xml
<Version>0.1.0</Version>
```

### 2. Update CHANGELOG

Add release notes to `CHANGELOG.md`:

```markdown
## [0.1.0] - 2025-10-23

### Added
- Initial release
- KV Store operations
- Queue operations
- Stream operations
- Pub/Sub operations
```

### 3. Commit Changes

```bash
git add .
git commit -m "chore: Release version 0.1.0

- Complete C# SDK implementation
- 40 tests passing with 95%+ coverage
- Full XML documentation
- Ready for NuGet publication"
```

### 4. Create Tag

```bash
git tag -a synap-csharp-v0.1.0 -m "Synap C# SDK v0.1.0

First stable release:
- Key-Value Store with TTL support
- Message Queues with ACK/NACK
- Event Streams with offset tracking
- Pub/Sub with wildcard support
- StreamableHTTP protocol
- 40 tests with 95%+ coverage
- Full async/await support
- Comprehensive XML documentation"
```

### 5. Build Release Package

```bash
dotnet clean
dotnet build --configuration Release
dotnet test --configuration Release
dotnet pack --configuration Release
```

### 6. Publish to NuGet

#### Option A: Via dotnet CLI

```bash
dotnet nuget push bin/Release/HiveLLM.Synap.SDK.0.1.0.nupkg \
  --api-key YOUR_API_KEY \
  --source https://api.nuget.org/v3/index.json
```

#### Option B: Manual Upload

1. Go to https://www.nuget.org/packages/manage/upload
2. Upload `bin/Release/HiveLLM.Synap.SDK.0.1.0.nupkg`
3. Verify package details
4. Publish

### 7. Push to GitHub

```bash
git push origin main
git push origin synap-csharp-v0.1.0
```

### 8. Create GitHub Release

1. Go to GitHub releases page
2. Create new release from tag `synap-csharp-v0.1.0`
3. Copy CHANGELOG content
4. Attach `.nupkg` file
5. Publish release

## Post-Publish Verification

- [ ] Package appears on NuGet.org
  ```
  https://www.nuget.org/packages/HiveLLM.Synap.SDK
  ```

- [ ] Installation works in new project
  ```bash
  mkdir test-install
  cd test-install
  dotnet new console
  dotnet add package HiveLLM.Synap.SDK
  dotnet build
  ```

- [ ] Documentation renders correctly on NuGet
- [ ] GitHub release is visible
- [ ] Tag is pushed to repository

## Version Bumping

After successful publish, bump version for next development cycle:

1. Update version to next pre-release: `0.2.0-alpha`
2. Commit version bump
3. Continue development

## Troubleshooting

### Build Fails

- Clean solution: `dotnet clean`
- Restore packages: `dotnet restore`
- Check .NET SDK version: `dotnet --version`

### Tests Fail

- Run specific test: `dotnet test --filter "FullyQualifiedName~TestName"`
- Check test output: `dotnet test --verbosity detailed`

### Pack Fails

- Ensure all required files exist
- Check project file for errors
- Verify package references

### Push to NuGet Fails

- Verify API key is valid
- Check package ID doesn't conflict
- Ensure version doesn't already exist
- Verify package size < 250MB

## Resources

- [NuGet Publishing Guide](https://learn.microsoft.com/en-us/nuget/nuget-org/publish-a-package)
- [Semantic Versioning](https://semver.org/)
- [Package Metadata](https://learn.microsoft.com/en-us/nuget/create-packages/package-authoring-best-practices)

