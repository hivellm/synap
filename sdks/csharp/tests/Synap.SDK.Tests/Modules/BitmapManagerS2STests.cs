using System.Diagnostics;
using Synap.SDK;
using Synap.SDK.Modules;
using Xunit;
using BitfieldOperation = Synap.SDK.Modules.BitfieldOperation;

namespace Synap.SDK.Tests.Modules;

/// <summary>
/// Server-to-Server (S2S) integration tests for BitmapManager.
/// These tests require a running Synap server.
/// Set SYNAP_URL environment variable to point to the server (default: http://localhost:15500).
/// </summary>
public sealed class BitmapManagerS2STests : IDisposable
{
    private readonly SynapClient _client;

    public BitmapManagerS2STests()
    {
        var url = Environment.GetEnvironmentVariable("SYNAP_URL") ?? "http://localhost:15500";
        var config = SynapConfig.Create(url);
        _client = new SynapClient(config);
    }

    [Fact]
    public async Task SetBit_GetBit_Works()
    {
        var key = $"test:bitmap:{Process.GetCurrentProcess().Id}";

        // Set bit 5 to 1
        var oldValue = await _client.Bitmap.SetBitAsync(key, 5, 1);
        Assert.Equal(0, oldValue);

        // Get bit 5
        var value = await _client.Bitmap.GetBitAsync(key, 5);
        Assert.Equal(1, value);

        // Set bit 5 back to 0
        var oldValue2 = await _client.Bitmap.SetBitAsync(key, 5, 0);
        Assert.Equal(1, oldValue2);

        // Get bit 5 again
        var value2 = await _client.Bitmap.GetBitAsync(key, 5);
        Assert.Equal(0, value2);
    }

    [Fact]
    public async Task BitCount_Works()
    {
        var key = $"test:bitmap:count:{Process.GetCurrentProcess().Id}";

        // Set multiple bits
        await _client.Bitmap.SetBitAsync(key, 0, 1);
        await _client.Bitmap.SetBitAsync(key, 2, 1);
        await _client.Bitmap.SetBitAsync(key, 4, 1);
        await _client.Bitmap.SetBitAsync(key, 6, 1);

        // Count all bits
        var count = await _client.Bitmap.BitCountAsync(key);
        Assert.Equal(4, count);
    }

    [Fact]
    public async Task BitPos_Works()
    {
        var key = $"test:bitmap:pos:{Process.GetCurrentProcess().Id}";

        // Set bit at position 7
        await _client.Bitmap.SetBitAsync(key, 7, 1);

        // Find first set bit
        var pos = await _client.Bitmap.BitPosAsync(key, 1);
        Assert.Equal(7, pos);
    }

    [Fact]
    public async Task BitOp_AND_Works()
    {
        var timestamp = Process.GetCurrentProcess().Id;
        var key1 = $"test:bitmap:and1:{timestamp}";
        var key2 = $"test:bitmap:and2:{timestamp}";
        var dest = $"test:bitmap:and_result:{timestamp}";

        // Set bits in bitmap1 (bits 0, 1, 2)
        await _client.Bitmap.SetBitAsync(key1, 0, 1);
        await _client.Bitmap.SetBitAsync(key1, 1, 1);
        await _client.Bitmap.SetBitAsync(key1, 2, 1);

        // Set bits in bitmap2 (bits 1, 2, 3)
        await _client.Bitmap.SetBitAsync(key2, 1, 1);
        await _client.Bitmap.SetBitAsync(key2, 2, 1);
        await _client.Bitmap.SetBitAsync(key2, 3, 1);

        // AND operation
        var length = await _client.Bitmap.BitOpAsync(BitmapManager.BitmapOperation.AND, dest, new[] { key1, key2 });
        Assert.True(length > 0);

        // Check result: should have bits 1 and 2 set
        Assert.Equal(0, await _client.Bitmap.GetBitAsync(dest, 0));
        Assert.Equal(1, await _client.Bitmap.GetBitAsync(dest, 1));
        Assert.Equal(1, await _client.Bitmap.GetBitAsync(dest, 2));
        Assert.Equal(0, await _client.Bitmap.GetBitAsync(dest, 3));
    }

    [Fact]
    public async Task BitField_GetSet_Works()
    {
        var key = $"test:bitmap:bitfield:{Process.GetCurrentProcess().Id}";

        // SET operation: Set 8-bit unsigned value 42 at offset 0
        var setOperations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.SET,
                Offset = 0,
                Width = 8,
                IsSigned = false,
                Value = 42,
            },
        };

        var setResults = await _client.Bitmap.BitFieldAsync(key, setOperations);
        Assert.Single(setResults);
        Assert.Equal(0, setResults[0]); // Old value was 0

        // GET operation: Read back the value
        var getOperations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.GET,
                Offset = 0,
                Width = 8,
                IsSigned = false,
            },
        };

        var getResults = await _client.Bitmap.BitFieldAsync(key, getOperations);
        Assert.Single(getResults);
        Assert.Equal(42, getResults[0]);
    }

    [Fact]
    public async Task BitField_IncrByWrap_Works()
    {
        var key = $"test:bitmap:bitfield_wrap:{Process.GetCurrentProcess().Id}";

        // Set initial value
        var setOperations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.SET,
                Offset = 0,
                Width = 8,
                IsSigned = false,
                Value = 250,
            },
        };
        await _client.Bitmap.BitFieldAsync(key, setOperations);

        // INCRBY with wrap: 250 + 10 = 260 wraps to 4
        var incrOperations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.INCRBY,
                Offset = 0,
                Width = 8,
                IsSigned = false,
                Increment = 10,
                Overflow = BitmapManager.BitfieldOverflow.WRAP,
            },
        };

        var results = await _client.Bitmap.BitFieldAsync(key, incrOperations);
        Assert.Single(results);
        Assert.Equal(4, results[0]); // 250 + 10 = 260 wraps to 4 (260 - 256)
    }

    [Fact]
    public async Task BitField_IncrBySat_Works()
    {
        var key = $"test:bitmap:bitfield_sat:{Process.GetCurrentProcess().Id}";

        // Set 4-bit unsigned value to 14
        var setOperations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.SET,
                Offset = 0,
                Width = 4,
                IsSigned = false,
                Value = 14,
            },
        };
        await _client.Bitmap.BitFieldAsync(key, setOperations);

        // INCRBY with saturate: 14 + 1 = 15 (max), then stays at 15
        var incrOperations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.INCRBY,
                Offset = 0,
                Width = 4,
                IsSigned = false,
                Increment = 1,
                Overflow = BitmapManager.BitfieldOverflow.SAT,
            },
        };

        var results1 = await _client.Bitmap.BitFieldAsync(key, incrOperations);
        Assert.Equal(15, results1[0]);

        // Try to increment again (should saturate at 15)
        var results2 = await _client.Bitmap.BitFieldAsync(key, incrOperations);
        Assert.Equal(15, results2[0]);
    }

    [Fact]
    public async Task BitField_MultipleOperations_Works()
    {
        var key = $"test:bitmap:bitfield_multi:{Process.GetCurrentProcess().Id}";

        // Execute multiple operations in sequence
        var operations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.SET,
                Offset = 0,
                Width = 8,
                IsSigned = false,
                Value = 100,
            },
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.SET,
                Offset = 8,
                Width = 8,
                IsSigned = false,
                Value = 200,
            },
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.GET,
                Offset = 0,
                Width = 8,
                IsSigned = false,
            },
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.GET,
                Offset = 8,
                Width = 8,
                IsSigned = false,
            },
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.INCRBY,
                Offset = 0,
                Width = 8,
                IsSigned = false,
                Increment = 50,
                Overflow = BitmapManager.BitfieldOverflow.WRAP,
            },
        };

        var results = await _client.Bitmap.BitFieldAsync(key, operations);
        Assert.Equal(5, results.Count);
        Assert.Equal(0, results[0]); // Old value at offset 0
        Assert.Equal(0, results[1]); // Old value at offset 8
        Assert.Equal(100, results[2]); // Read back offset 0
        Assert.Equal(200, results[3]); // Read back offset 8
        Assert.Equal(150, results[4]); // Incremented offset 0
    }

    [Fact]
    public async Task BitField_SignedValues_Works()
    {
        var key = $"test:bitmap:bitfield_signed:{Process.GetCurrentProcess().Id}";

        // Set signed 8-bit negative value
        var setOperations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.SET,
                Offset = 0,
                Width = 8,
                IsSigned = true,
                Value = -10,
            },
        };
        await _client.Bitmap.BitFieldAsync(key, setOperations);

        // Read back as signed
        var getOperations = new List<BitfieldOperation>
        {
            new BitfieldOperation
            {
                Operation = BitmapManager.BitfieldOperationType.GET,
                Offset = 0,
                Width = 8,
                IsSigned = true,
            },
        };

        var results = await _client.Bitmap.BitFieldAsync(key, getOperations);
        Assert.Single(results);
        Assert.Equal(-10, results[0]);
    }

    [Fact]
    public async Task Stats_ReturnsValidData()
    {
        var key = $"test:bitmap:stats:{Process.GetCurrentProcess().Id}";

        // Perform some operations
        await _client.Bitmap.SetBitAsync(key, 0, 1);
        await _client.Bitmap.GetBitAsync(key, 0);
        await _client.Bitmap.BitCountAsync(key);

        var stats = await _client.Bitmap.StatsAsync();
        Assert.True(stats.SetBitCount >= 1);
        Assert.True(stats.GetBitCount >= 1);
        Assert.True(stats.BitCountCount >= 1);
    }

    public void Dispose()
    {
        _client?.Dispose();
    }
}

