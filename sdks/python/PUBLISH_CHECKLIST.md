# Publishing Checklist for Synap Python SDK

This checklist ensures the SDK is ready for publication to PyPI.

## Pre-Publish Checklist

### Code Quality

- [ ] All tests passing (100%)
  ```bash
  pytest
  ```

- [ ] Coverage meets threshold (95%+)
  ```bash
  pytest --cov=synap_sdk --cov-report=term-missing
  ```

- [ ] Code formatted
  ```bash
  ruff format synap_sdk tests examples --check
  ```

- [ ] Linting passes
  ```bash
  ruff check synap_sdk
  ```

- [ ] Type checking passes
  ```bash
  mypy synap_sdk
  ```

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

- [ ] Version number updated in `pyproject.toml`
- [ ] Package classifiers appropriate
- [ ] Package description accurate
- [ ] Repository URL correct
- [ ] Authors updated

### Testing

- [ ] All unit tests pass
  ```bash
  pytest
  ```

- [ ] Example script runs successfully
  ```bash
  python examples/basic_usage.py
  ```

### Build & Distribution

- [ ] Clean build succeeds
  ```bash
  rm -rf dist/ build/ *.egg-info
  python -m build
  ```

- [ ] Package installs correctly
  ```bash
  pip install dist/synap_sdk-0.1.0-py3-none-any.whl
  ```

## Publishing Steps

### 1. Update Version

Update version in `pyproject.toml`:

```toml
[project]
version = "0.1.0"
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

### 3. Run Quality Checks

```bash
# Format code
ruff format synap_sdk tests examples

# Check linting
ruff check synap_sdk

# Type check
mypy synap_sdk

# Run tests with coverage
pytest --cov=synap_sdk --cov-report=term-missing
```

### 4. Commit Changes

```bash
git add .
git commit -m "chore: Release version 0.1.0

- Complete Python SDK implementation
- 54 tests passing with 96.79% coverage
- Full type hints with mypy strict mode
- Ready for PyPI publication"
```

### 5. Create Tag

```bash
git tag -a synap-python-v0.1.0 -m "Synap Python SDK v0.1.0

First stable release:
- Key-Value Store with TTL support
- Message Queues with ACK/NACK
- Event Streams with offset tracking
- Pub/Sub with wildcard support
- StreamableHTTP protocol
- 54 tests with 96.79% coverage
- Full async/await support
- Comprehensive type hints"
```

### 6. Build Distribution

```bash
# Install build tools
pip install build twine

# Build package
python -m build

# Check package
twine check dist/*
```

### 7. Publish to PyPI

#### Option A: Test PyPI First (Recommended)

```bash
twine upload --repository testpypi dist/*
```

#### Option B: Production PyPI

```bash
twine upload dist/*
```

You'll be prompted for your PyPI credentials or API token.

### 8. Push to GitHub

```bash
git push origin main
git push origin synap-python-v0.1.0
```

### 9. Create GitHub Release

1. Go to GitHub releases page
2. Create new release from tag `synap-python-v0.1.0`
3. Copy CHANGELOG content
4. Attach distribution files (`.whl` and `.tar.gz`)
5. Publish release

## Post-Publish Verification

- [ ] Package appears on PyPI
  ```
  https://pypi.org/project/synap-sdk
  ```

- [ ] Installation works in new project
  ```bash
  python -m venv test-env
  source test-env/bin/activate
  pip install synap-sdk
  python -c "from synap_sdk import SynapClient; print('OK')"
  ```

- [ ] Documentation renders correctly on PyPI
- [ ] GitHub release is visible
- [ ] Tag is pushed to repository

## Version Bumping

After successful publish, bump version for next development cycle:

1. Update version to next pre-release: `0.2.0.dev0`
2. Commit version bump
3. Continue development

## Troubleshooting

### Tests Fail

- Run with verbose output: `pytest -vv`
- Run specific test: `pytest tests/test_kv_store.py::test_name -vv`
- Check test output: `pytest --tb=long`

### Type Checking Fails

- Run mypy with verbose: `mypy --show-error-codes synap_sdk`
- Check specific file: `mypy synap_sdk/client.py`

### Build Fails

- Ensure pyproject.toml is valid
- Check all required files exist
- Verify dependencies are correct

### Upload to PyPI Fails

- Verify API token is valid (use `~/.pypirc`)
- Check package name doesn't conflict
- Ensure version doesn't already exist
- Verify package size < 60MB

## PyPI Configuration

Create `~/.pypirc`:

```ini
[distutils]
index-servers =
    pypi
    testpypi

[pypi]
username = __token__
password = pypi-...your-token...

[testpypi]
repository = https://test.pypi.org/legacy/
username = __token__
password = pypi-...your-token...
```

## Resources

- [PyPI Publishing Guide](https://packaging.python.org/tutorials/packaging-projects/)
- [Semantic Versioning](https://semver.org/)
- [Python Packaging Guide](https://packaging.python.org/)

