"""S2S (Server-to-Server) integration tests for Transaction operations."""

import os
import uuid

import pytest

from synap_sdk.client import SynapClient
from synap_sdk.config import SynapConfig


@pytest.fixture
def client():
    """Create a Synap client for testing."""
    config = SynapConfig(
        url=os.getenv("SYNAP_URL", "http://localhost:15500"),
    )
    return SynapClient(config)


class TestTransactionS2S:
    """S2S integration tests for Transaction operations."""

    @pytest.mark.asyncio
    async def test_multi_exec(self, client):
        """Test MULTI/EXEC transaction workflow."""
        async with client:
            client_id = f"test:{uuid.uuid4()}"
            
            # Start transaction
            result = await client.transaction.multi(client_id=client_id)
            assert result["success"] is True
            
            # Queue commands using send_command with client_id (automatic queuing)
            await client.send_command("kv.set", {"key": "tx:key1", "value": "value1", "client_id": client_id})
            await client.send_command("kv.set", {"key": "tx:key2", "value": "value2", "client_id": client_id})
            
            # Execute transaction
            exec_result = await client.transaction.exec(client_id=client_id)
            assert exec_result["success"] is True
            assert "results" in exec_result
            assert len(exec_result["results"]) == 2
            
            # Verify values were set
            value1 = await client.kv.get("tx:key1")
            value2 = await client.kv.get("tx:key2")
            assert value1 == "value1"
            assert value2 == "value2"

    @pytest.mark.asyncio
    async def test_discard(self, client):
        """Test DISCARD transaction."""
        async with client:
            client_id = f"test:{uuid.uuid4()}"
            
            # Start transaction
            await client.transaction.multi(client_id=client_id)
            
            # Queue a command (will be discarded)
            await client.send_command("kv.set", {"key": "tx:discard:key", "value": "value", "client_id": client_id})
            
            # Discard transaction
            result = await client.transaction.discard(client_id=client_id)
            assert result["success"] is True
            
            # Verify value was NOT set
            value = await client.kv.get("tx:discard:key")
            assert value is None

    @pytest.mark.asyncio
    async def test_watch_unwatch(self, client):
        """Test WATCH/UNWATCH operations."""
        async with client:
            client_id = f"test:{uuid.uuid4()}"
            
            # Start transaction
            await client.transaction.multi(client_id=client_id)
            
            # Watch keys
            result = await client.transaction.watch(["watch:key1", "watch:key2"], client_id=client_id)
            assert result["success"] is True
            
            # Unwatch
            result = await client.transaction.unwatch(client_id=client_id)
            assert result["success"] is True

    @pytest.mark.asyncio
    async def test_watch_abort_on_conflict(self, client):
        """Test that WATCH aborts transaction when keys change."""
        async with client:
            client_id = f"test:{uuid.uuid4()}"
            
            # Set initial value
            await client.kv.set("watch:conflict:key", "initial")
            
            # Start transaction and watch
            await client.transaction.multi(client_id=client_id)
            await client.transaction.watch(["watch:conflict:key"], client_id=client_id)
            
            # Modify watched key from another client (simulate conflict)
            await client.kv.set("watch:conflict:key", "modified")
            
            # Try to execute transaction (should abort)
            exec_result = await client.transaction.exec(client_id=client_id)
            assert exec_result["success"] is False
            assert exec_result.get("aborted") is True

    @pytest.mark.asyncio
    async def test_empty_transaction(self, client):
        """Test executing empty transaction."""
        async with client:
            client_id = f"test:{uuid.uuid4()}"
            
            # Start transaction
            await client.transaction.multi(client_id=client_id)
            
            # Execute without queuing commands
            exec_result = await client.transaction.exec(client_id=client_id)
            assert exec_result["success"] is True
            assert exec_result["results"] == []

