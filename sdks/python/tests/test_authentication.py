"""Tests for authentication (Basic Auth and API Key)."""

import os
import pytest

from synap_sdk import SynapClient, SynapConfig
from synap_sdk.exceptions import SynapException

SYNAP_URL = os.getenv("SYNAP_URL", "http://localhost:15500")
TEST_USERNAME = os.getenv("SYNAP_TEST_USERNAME", "root")
TEST_PASSWORD = os.getenv("SYNAP_TEST_PASSWORD", "root")


@pytest.mark.asyncio
class TestBasicAuth:
    """Tests for Basic Auth authentication."""

    async def test_basic_auth_success(self):
        """Test successful authentication with Basic Auth."""
        config = SynapConfig.create(SYNAP_URL).with_basic_auth(TEST_USERNAME, TEST_PASSWORD)
        async with SynapClient(config) as client:
            # Test health check
            health = await client.health()
            assert health is not None

            # Test KV operation
            await client.kv.set("auth:test:basic", "test_value")
            value = await client.kv.get("auth:test:basic")
            assert value == "test_value"

            # Cleanup
            await client.kv.delete("auth:test:basic")

    async def test_basic_auth_invalid_credentials(self):
        """Test authentication failure with invalid credentials."""
        config = SynapConfig.create(SYNAP_URL).with_basic_auth("invalid", "invalid")
        async with SynapClient(config) as client:
            with pytest.raises(Exception):  # Should raise connection or auth error
                await client.health()

    async def test_basic_auth_missing_password(self):
        """Test authentication failure with missing password."""
        config = SynapConfig.create(SYNAP_URL).with_basic_auth(TEST_USERNAME, "")
        async with SynapClient(config) as client:
            with pytest.raises(Exception):
                await client.health()

    async def test_basic_auth_config_validation(self):
        """Test that config prevents using both auth_token and Basic Auth."""
        with pytest.raises(SynapException, match="Cannot use both"):
            SynapConfig(
                SYNAP_URL,
                auth_token="test",
                username="user",
                password="pass"
            )


@pytest.mark.asyncio
class TestApiKeyAuth:
    """Tests for API Key authentication."""

    async def test_api_key_auth_success(self):
        """Test successful authentication with API Key."""
        # Note: This test requires a valid API key to be created first
        # In real scenario, create API key via REST API before running this test
        api_key = os.getenv("SYNAP_TEST_API_KEY")
        if not api_key:
            pytest.skip("No API key available - set SYNAP_TEST_API_KEY env var")

        config = SynapConfig.create(SYNAP_URL).with_auth_token(api_key)
        async with SynapClient(config) as client:
            # Test health check
            health = await client.health()
            assert health is not None

            # Test KV operation
            await client.kv.set("auth:test:apikey", "test_value")
            value = await client.kv.get("auth:test:apikey")
            assert value == "test_value"

            # Cleanup
            await client.kv.delete("auth:test:apikey")

    async def test_api_key_auth_invalid_key(self):
        """Test authentication failure with invalid API key."""
        config = SynapConfig.create(SYNAP_URL).with_auth_token("invalid-api-key-12345")
        async with SynapClient(config) as client:
            with pytest.raises(Exception):
                await client.health()

    async def test_api_key_auth_empty_key(self):
        """Test authentication failure with empty API key."""
        config = SynapConfig.create(SYNAP_URL).with_auth_token("")
        async with SynapClient(config) as client:
            with pytest.raises(Exception):
                await client.health()


@pytest.mark.asyncio
class TestNoAuth:
    """Tests for no authentication (when auth is disabled)."""

    async def test_no_auth_when_disabled(self):
        """Test that client works without auth when server auth is disabled."""
        config = SynapConfig.create(SYNAP_URL)
        async with SynapClient(config) as client:
            try:
                # This will only work if auth is disabled on server
                health = await client.health()
                assert health is not None
            except Exception:
                # Expected if auth is required
                pass


class TestAuthConfig:
    """Tests for authentication configuration."""

    def test_basic_auth_config_creation(self):
        """Test creating config with Basic Auth."""
        config = SynapConfig.create(SYNAP_URL).with_basic_auth("user", "pass")
        assert config.username == "user"
        assert config.password == "pass"
        assert config.auth_token is None

    def test_api_key_config_creation(self):
        """Test creating config with API Key."""
        config = SynapConfig.create(SYNAP_URL).with_auth_token("sk_test123")
        assert config.auth_token == "sk_test123"
        assert config.username is None
        assert config.password is None

    def test_config_builder_pattern(self):
        """Test config builder pattern."""
        config = (
            SynapConfig.create(SYNAP_URL)
            .with_basic_auth("user", "pass")
            .with_timeout(60)
        )
        assert config.username == "user"
        assert config.password == "pass"
        assert config.timeout == 60

    def test_config_with_both_auth_methods_raises_error(self):
        """Test that providing both auth methods raises error."""
        with pytest.raises(SynapException, match="Cannot use both"):
            SynapConfig(
                SYNAP_URL,
                auth_token="test",
                username="user",
                password="pass"
            )

