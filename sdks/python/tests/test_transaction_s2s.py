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
            
            # Execute empty transaction (commands queuing requires handler modification)
            exec_result = await client.transaction.exec(client_id=client_id)
            assert exec_result["success"] is True
            assert "results" in exec_result
            assert isinstance(exec_result["results"], list)

    @pytest.mark.asyncio
    async def test_discard(self, client):
        """Test DISCARD transaction."""
        async with client:
            client_id = f"test:{uuid.uuid4()}"
            
            # Start transaction
            await client.transaction.multi(client_id=client_id)
            
            # Discard transaction
            result = await client.transaction.discard(client_id=client_id)
            assert result["success"] is True

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

