"""Bitmap S2S Integration Tests

These tests require a running Synap server.
Run with: SYNAP_URL=http://localhost:15500 pytest tests/test_bitmap_s2s.py
"""

import os
import pytest
from synap_sdk import SynapClient, SynapConfig

SYNAP_URL = os.getenv('SYNAP_URL', 'http://localhost:15500')
SKIP_S2S = not os.getenv('SYNAP_URL') and not os.getenv('CI')


@pytest.mark.skipif(SKIP_S2S, reason='S2S tests require SYNAP_URL or CI env')
class TestBitmapS2S:
    @pytest.fixture
    def client(self):
        config = SynapConfig(SYNAP_URL)
        async_client = SynapClient(config)
        return async_client

    @pytest.mark.asyncio
    async def test_setbit_getbit(self, client):
        async with client:
            key = f'test:bitmap:{os.getpid()}'

            # Set bit 5 to 1
            old_value = await client.bitmap.setbit(key, 5, 1)
            assert old_value == 0

            # Get bit 5
            value = await client.bitmap.getbit(key, 5)
            assert value == 1

            # Set bit 5 back to 0
            old_value2 = await client.bitmap.setbit(key, 5, 0)
            assert old_value2 == 1

            # Get bit 5 again
            value2 = await client.bitmap.getbit(key, 5)
            assert value2 == 0

    @pytest.mark.asyncio
    async def test_bitcount(self, client):
        async with client:
            key = f'test:bitmap:count:{os.getpid()}'

            # Set multiple bits
            await client.bitmap.setbit(key, 0, 1)
            await client.bitmap.setbit(key, 2, 1)
            await client.bitmap.setbit(key, 4, 1)
            await client.bitmap.setbit(key, 6, 1)

            # Count all bits
            count = await client.bitmap.bitcount(key)
            assert count == 4

    @pytest.mark.asyncio
    async def test_bitpos(self, client):
        async with client:
            key = f'test:bitmap:pos:{os.getpid()}'

            # Set bit at position 7
            await client.bitmap.setbit(key, 7, 1)

            # Find first set bit
            pos = await client.bitmap.bitpos(key, 1)
            assert pos == 7

    @pytest.mark.asyncio
    async def test_bitop_and(self, client):
        async with client:
            timestamp = os.getpid()
            key1 = f'test:bitmap:and1:{timestamp}'
            key2 = f'test:bitmap:and2:{timestamp}'
            dest = f'test:bitmap:and_result:{timestamp}'

            # Set bits in bitmap1 (bits 0, 1, 2)
            await client.bitmap.setbit(key1, 0, 1)
            await client.bitmap.setbit(key1, 1, 1)
            await client.bitmap.setbit(key1, 2, 1)

            # Set bits in bitmap2 (bits 1, 2, 3)
            await client.bitmap.setbit(key2, 1, 1)
            await client.bitmap.setbit(key2, 2, 1)
            await client.bitmap.setbit(key2, 3, 1)

            # AND operation
            length = await client.bitmap.bitop('AND', dest, [key1, key2])
            assert length > 0

            # Check result: should have bits 1 and 2 set
            assert await client.bitmap.getbit(dest, 0) == 0
            assert await client.bitmap.getbit(dest, 1) == 1
            assert await client.bitmap.getbit(dest, 2) == 1
            assert await client.bitmap.getbit(dest, 3) == 0

    @pytest.mark.asyncio
    async def test_bitfield_get_set(self, client):
        async with client:
            key = f'test:bitmap:bitfield:{os.getpid()}'

            # SET operation: Set 8-bit unsigned value 42 at offset 0
            operations = [
                {
                    'operation': 'SET',
                    'offset': 0,
                    'width': 8,
                    'signed': False,
                    'value': 42
                }
            ]
            results = await client.bitmap.bitfield(key, operations)
            assert len(results) == 1
            assert results[0] == 0  # Old value was 0

            # GET operation: Read back the value
            operations = [
                {
                    'operation': 'GET',
                    'offset': 0,
                    'width': 8,
                    'signed': False
                }
            ]
            results = await client.bitmap.bitfield(key, operations)
            assert len(results) == 1
            assert results[0] == 42

    @pytest.mark.asyncio
    async def test_bitfield_incrby_wrap(self, client):
        async with client:
            key = f'test:bitmap:bitfield_incr:{os.getpid()}'

            # Set initial value
            operations = [
                {
                    'operation': 'SET',
                    'offset': 0,
                    'width': 8,
                    'signed': False,
                    'value': 250
                }
            ]
            await client.bitmap.bitfield(key, operations)

            # INCRBY with wrap: 250 + 10 = 260 wraps to 4
            operations = [
                {
                    'operation': 'INCRBY',
                    'offset': 0,
                    'width': 8,
                    'signed': False,
                    'increment': 10,
                    'overflow': 'WRAP'
                }
            ]
            results = await client.bitmap.bitfield(key, operations)
            assert len(results) == 1
            assert results[0] == 4  # 250 + 10 = 260 wraps to 4 (260 - 256)

    @pytest.mark.asyncio
    async def test_bitfield_incrby_sat(self, client):
        async with client:
            key = f'test:bitmap:bitfield_sat:{os.getpid()}'

            # Set 4-bit unsigned value to 14
            operations = [
                {
                    'operation': 'SET',
                    'offset': 0,
                    'width': 4,
                    'signed': False,
                    'value': 14
                }
            ]
            await client.bitmap.bitfield(key, operations)

            # INCRBY with saturate: 14 + 1 = 15 (max), then stays at 15
            operations = [
                {
                    'operation': 'INCRBY',
                    'offset': 0,
                    'width': 4,
                    'signed': False,
                    'increment': 1,
                    'overflow': 'SAT'
                }
            ]
            results = await client.bitmap.bitfield(key, operations)
            assert len(results) == 1
            assert results[0] == 15

            # Try to increment again (should saturate at 15)
            results = await client.bitmap.bitfield(key, operations)
            assert len(results) == 1
            assert results[0] == 15

    @pytest.mark.asyncio
    async def test_bitfield_multiple_operations(self, client):
        async with client:
            key = f'test:bitmap:bitfield_multi:{os.getpid()}'

            # Execute multiple operations in sequence
            operations = [
                {
                    'operation': 'SET',
                    'offset': 0,
                    'width': 8,
                    'signed': False,
                    'value': 100
                },
                {
                    'operation': 'SET',
                    'offset': 8,
                    'width': 8,
                    'signed': False,
                    'value': 200
                },
                {
                    'operation': 'GET',
                    'offset': 0,
                    'width': 8,
                    'signed': False
                },
                {
                    'operation': 'GET',
                    'offset': 8,
                    'width': 8,
                    'signed': False
                },
                {
                    'operation': 'INCRBY',
                    'offset': 0,
                    'width': 8,
                    'signed': False,
                    'increment': 50,
                    'overflow': 'WRAP'
                }
            ]
            results = await client.bitmap.bitfield(key, operations)
            assert len(results) == 5
            assert results[0] == 0  # Old value at offset 0
            assert results[1] == 0  # Old value at offset 8
            assert results[2] == 100  # Read back offset 0
            assert results[3] == 200  # Read back offset 8
            assert results[4] == 150  # Incremented offset 0

    @pytest.mark.asyncio
    async def test_bitfield_signed_values(self, client):
        async with client:
            key = f'test:bitmap:bitfield_signed:{os.getpid()}'

            # Set signed 8-bit negative value
            operations = [
                {
                    'operation': 'SET',
                    'offset': 0,
                    'width': 8,
                    'signed': True,
                    'value': -10
                }
            ]
            await client.bitmap.bitfield(key, operations)

            # Read back as signed
            operations = [
                {
                    'operation': 'GET',
                    'offset': 0,
                    'width': 8,
                    'signed': True
                }
            ]
            results = await client.bitmap.bitfield(key, operations)
            assert len(results) == 1
            assert results[0] == -10

    @pytest.mark.asyncio
    async def test_stats(self, client):
        async with client:
            key = f'test:bitmap:stats:{os.getpid()}'

            # Perform some operations
            await client.bitmap.setbit(key, 0, 1)
            await client.bitmap.getbit(key, 0)
            await client.bitmap.bitcount(key)

            stats = await client.bitmap.stats()
            assert stats['setbit_count'] >= 1
            assert stats['getbit_count'] >= 1
            assert stats['bitcount_count'] >= 1

