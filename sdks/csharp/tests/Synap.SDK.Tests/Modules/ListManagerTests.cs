using Moq;
using Synap.SDK.Modules;
using System.Text.Json;
using Xunit;

namespace Synap.SDK.Tests.Modules;

public sealed class ListManagerTests
{
    private readonly Mock<SynapClient> _mockClient;
    private readonly ListManager _listManager;

    public ListManagerTests()
    {
        var config = new SynapConfig("http://localhost:15500");
        _mockClient = new Mock<SynapClient>(config);
        _listManager = new ListManager(_mockClient.Object);
    }

    [Fact]
    public async Task LPushAsync_ShouldReturnLength()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { length = 3 });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _listManager.LPushAsync("tasks", new List<string> { "task1", "task2" });

        // Assert
        Assert.Equal(3, result);
    }

    [Fact]
    public async Task RPushAsync_ShouldReturnLength()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { length = 2 });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _listManager.RPushAsync("tasks", new List<string> { "task1" });

        // Assert
        Assert.Equal(2, result);
    }

    [Fact]
    public async Task LPopAsync_ShouldReturnValues()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { values = new[] { "task1" } });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _listManager.LPopAsync("tasks");

        // Assert
        Assert.Single(result);
        Assert.Equal("task1", result[0]);
    }

    [Fact]
    public async Task RangeAsync_ShouldReturnRange()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { values = new[] { "task1", "task2", "task3" } });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _listManager.RangeAsync("tasks");

        // Assert
        Assert.Equal(3, result.Count);
    }

    [Fact]
    public async Task LenAsync_ShouldReturnLength()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { length = 5 });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _listManager.LenAsync("tasks");

        // Assert
        Assert.Equal(5, result);
    }
}

