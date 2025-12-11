"""HyperLogLog S2S Integration Tests

These tests require a running Synap server.
Run with: SYNAP_URL=http://localhost:15500 pytest tests/test_hyperloglog_s2s.py
"""

import os
import pytest
from synap_sdk import SynapClient, SynapConfig

SYNAP_URL = os.getenv('SYNAP_URL', 'http://localhost:15500')
SKIP_S2S = not os.getenv('SYNAP_URL') and not os.getenv('CI')


@pytest.mark.skipif(SKIP_S2S, reason='S2S tests require SYNAP_URL or CI env')
class TestHyperLogLogS2S:
    @pytest.fixture
    def client(self):
        config = SynapConfig(SYNAP_URL)
        async_client = SynapClient(config)
        return async_client

    @pytest.mark.asyncio
    async def test_pfadd_pfcount(self, client):
        async with client:
            key = f'test:hll:{os.getpid()}'

            added = await client.hyperloglog.pfadd(key, ['user:1', 'user:2', 'user:3'])
            assert added >= 0 and added <= 3

            count = await client.hyperloglog.pfcount(key)
            assert count >= 2 and count <= 4  # Approximate

    @pytest.mark.asyncio
    async def test_pfmerge(self, client):
        async with client:
            timestamp = os.getpid()
            key1 = f'test:hll:merge1:{timestamp}'
            key2 = f'test:hll:merge2:{timestamp}'
            dest = f'test:hll:merge_dest:{timestamp}'

            await client.hyperloglog.pfadd(key1, ['user:1', 'user:2', 'user:3'])
            await client.hyperloglog.pfadd(key2, ['user:4', 'user:5', 'user:6'])

            count = await client.hyperloglog.pfmerge(dest, [key1, key2])
            assert count >= 5 and count <= 7  # Approximate

    @pytest.mark.asyncio
    async def test_stats(self, client):
        async with client:
            key = f'test:hll:stats:{os.getpid()}'

            await client.hyperloglog.pfadd(key, ['user:1', 'user:2'])
            await client.hyperloglog.pfcount(key)

            stats = await client.hyperloglog.stats()
            assert stats['pfadd_count'] >= 1
            assert stats['pfcount_count'] >= 1

