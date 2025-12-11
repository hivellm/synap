"""Tests for SynapConfig."""

import pytest

from synap_sdk.config import SynapConfig
from synap_sdk.exceptions import SynapException


def test_constructor_with_valid_url() -> None:
    """Test constructor with valid URL."""
    config = SynapConfig("http://localhost:15500")

    assert config.base_url == "http://localhost:15500"
    assert config.timeout == 30
    assert config.auth_token is None
    assert config.max_retries == 3


def test_constructor_removes_trailing_slash() -> None:
    """Test constructor removes trailing slash."""
    config = SynapConfig("http://localhost:15500/")

    assert config.base_url == "http://localhost:15500"


@pytest.mark.parametrize("url", ["", "   "])
def test_constructor_with_empty_url_raises(url: str) -> None:
    """Test constructor with empty URL raises exception."""
    with pytest.raises(SynapException, match="Base URL cannot be empty"):
        SynapConfig(url)


def test_create_returns_new_config() -> None:
    """Test create factory method."""
    config = SynapConfig.create("http://localhost:15500")

    assert config is not None
    assert config.base_url == "http://localhost:15500"


def test_with_timeout_returns_new_config() -> None:
    """Test with_timeout returns new config."""
    config = SynapConfig.create("http://localhost:15500")
    new_config = config.with_timeout(60)

    assert config.timeout == 30
    assert new_config.timeout == 60
    assert config is not new_config


def test_with_auth_token_returns_new_config() -> None:
    """Test with_auth_token returns new config."""
    config = SynapConfig.create("http://localhost:15500")
    new_config = config.with_auth_token("test-token")

    assert config.auth_token is None
    assert new_config.auth_token == "test-token"
    assert config is not new_config


def test_with_max_retries_returns_new_config() -> None:
    """Test with_max_retries returns new config."""
    config = SynapConfig.create("http://localhost:15500")
    new_config = config.with_max_retries(5)

    assert config.max_retries == 3
    assert new_config.max_retries == 5
    assert config is not new_config


def test_chained_with_methods() -> None:
    """Test chaining with methods."""
    config = (
        SynapConfig.create("http://localhost:15500")
        .with_timeout(60)
        .with_auth_token("my-token")
        .with_max_retries(5)
    )

    assert config.timeout == 60
    assert config.auth_token == "my-token"
    assert config.max_retries == 5
