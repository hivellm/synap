using Moq;
using Synap.SDK.Modules;
using System.Text.Json;
using Xunit;

namespace Synap.SDK.Tests.Modules;

public sealed class HashManagerTests
{
    private readonly Mock<SynapClient> _mockClient;
    private readonly HashManager _hashManager;

    public HashManagerTests()
    {
        var config = new SynapConfig("http://localhost:15500");
        _mockClient = new Mock<SynapClient>(config);
        _hashManager = new HashManager(_mockClient.Object);
    }

    [Fact]
    public async Task SetAsync_ShouldReturnTrue()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { success = true });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _hashManager.SetAsync("user:1", "name", "Alice");

        // Assert
        Assert.True(result);
    }

    [Fact]
    public async Task GetAsync_ShouldReturnValue()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { value = "Alice" });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _hashManager.GetAsync("user:1", "name");

        // Assert
        Assert.Equal("Alice", result);
    }

    [Fact]
    public async Task GetAllAsync_ShouldReturnAllFields()
    {
        // Arrange
        var fields = new Dictionary<string, string> { { "name", "Alice" }, { "age", "30" } };
        var responseJson = JsonSerializer.Serialize(new { fields });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _hashManager.GetAllAsync("user:1");

        // Assert
        Assert.Equal(2, result.Count);
        Assert.Equal("Alice", result["name"]);
        Assert.Equal("30", result["age"]);
    }

    [Fact]
    public async Task DeleteAsync_ShouldReturnDeletedCount()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { deleted = 1 });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _hashManager.DeleteAsync("user:1", "name");

        // Assert
        Assert.Equal(1, result);
    }

    [Fact]
    public async Task ExistsAsync_ShouldReturnTrue()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { exists = true });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _hashManager.ExistsAsync("user:1", "name");

        // Assert
        Assert.True(result);
    }

    [Fact]
    public async Task LenAsync_ShouldReturnLength()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { length = 2 });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _hashManager.LenAsync("user:1");

        // Assert
        Assert.Equal(2, result);
    }

    [Fact]
    public async Task IncrByAsync_ShouldReturnIncrementedValue()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { value = 5L });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _hashManager.IncrByAsync("counters", "visits", 1);

        // Assert
        Assert.Equal(5, result);
    }
}

