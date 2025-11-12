"""Transaction operations for Synap SDK."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, TypedDict

if TYPE_CHECKING:
    from synap_sdk.client import SynapClient


class TransactionResponse(TypedDict):
    """Response from transaction operations."""

    success: bool
    message: str


class TransactionExecSuccess(TypedDict):
    """Successful transaction execution result."""

    success: bool
    results: list[Any]


class TransactionExecAborted(TypedDict):
    """Aborted transaction execution result."""

    success: bool
    aborted: bool
    message: str | None


TransactionExecResult = TransactionExecSuccess | TransactionExecAborted


class TransactionManager:
    """Manage Transaction operations (Redis-compatible).

    Transaction operations allow atomic multi-key operations with optimistic locking.

    Example:
        >>> async with SynapClient(config) as client:
        ...     client_id = "tx-123"
        ...     await client.transaction.multi(client_id=client_id)
        ...     await client.kv.set("key1", "value1", client_id=client_id)
        ...     await client.kv.set("key2", "value2", client_id=client_id)
        ...     result = await client.transaction.exec(client_id=client_id)
        ...     if result["success"]:
        ...         print(f"Transaction executed: {result['results']}")
    """

    def __init__(self, client: SynapClient) -> None:
        """Initialize TransactionManager.

        Args:
            client: The Synap client instance
        """
        self._client = client

    async def multi(self, *, client_id: str | None = None) -> TransactionResponse:
        """Start a transaction (MULTI).

        Args:
            client_id: Optional client identifier to group commands within the same transaction

        Returns:
            Transaction response with success status and message

        Example:
            >>> await client.transaction.multi(client_id="tx-123")
            {'success': True, 'message': 'Transaction started'}
        """
        payload: dict[str, Any] = {}
        if client_id:
            payload["client_id"] = client_id

        response = await self._client.send_command("transaction.multi", payload)
        return {
            "success": response.get("success", True),
            "message": response.get("message", "Transaction started"),
        }

    async def discard(self, *, client_id: str | None = None) -> TransactionResponse:
        """Discard the current transaction (DISCARD).

        Args:
            client_id: Optional client identifier for the transaction

        Returns:
            Transaction response with success status and message

        Example:
            >>> await client.transaction.discard(client_id="tx-123")
            {'success': True, 'message': 'Transaction discarded'}
        """
        payload: dict[str, Any] = {}
        if client_id:
            payload["client_id"] = client_id

        response = await self._client.send_command("transaction.discard", payload)
        return {
            "success": response.get("success", True),
            "message": response.get("message", "Transaction discarded"),
        }

    async def watch(
        self, keys: list[str], *, client_id: str | None = None
    ) -> TransactionResponse:
        """Watch keys for optimistic locking (WATCH).

        Args:
            keys: List of keys to watch for changes
            client_id: Optional client identifier for the transaction

        Returns:
            Transaction response with success status and message

        Raises:
            ValueError: If keys list is empty

        Example:
            >>> await client.transaction.watch(["key1", "key2"], client_id="tx-123")
            {'success': True, 'message': 'Keys watched'}
        """
        if not keys:
            raise ValueError("Transaction watch requires at least one key")

        payload: dict[str, Any] = {"keys": keys}
        if client_id:
            payload["client_id"] = client_id

        response = await self._client.send_command("transaction.watch", payload)
        return {
            "success": response.get("success", True),
            "message": response.get("message", "Keys watched"),
        }

    async def unwatch(self, *, client_id: str | None = None) -> TransactionResponse:
        """Remove all watched keys (UNWATCH).

        Args:
            client_id: Optional client identifier for the transaction

        Returns:
            Transaction response with success status and message

        Example:
            >>> await client.transaction.unwatch(client_id="tx-123")
            {'success': True, 'message': 'Keys unwatched'}
        """
        payload: dict[str, Any] = {}
        if client_id:
            payload["client_id"] = client_id

        response = await self._client.send_command("transaction.unwatch", payload)
        return {
            "success": response.get("success", True),
            "message": response.get("message", "Keys unwatched"),
        }

    async def exec(
        self, *, client_id: str | None = None
    ) -> TransactionExecResult:
        """Execute queued commands (EXEC).

        Args:
            client_id: Optional client identifier for the transaction

        Returns:
            Transaction execution result. If successful, returns results array.
            If aborted (due to watched keys changed), returns aborted status.

        Example:
            >>> result = await client.transaction.exec(client_id="tx-123")
            >>> if result["success"]:
            ...     print(f"Results: {result['results']}")
            ... else:
            ...     print(f"Transaction aborted: {result.get('message')}")
        """
        payload: dict[str, Any] = {}
        if client_id:
            payload["client_id"] = client_id

        response = await self._client.send_command("transaction.exec", payload)

        if "results" in response and isinstance(response["results"], list):
            return {
                "success": True,
                "results": response["results"],
            }

        return {
            "success": False,
            "aborted": True,
            "message": response.get("message"),
        }

