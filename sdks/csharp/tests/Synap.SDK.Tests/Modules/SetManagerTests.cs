using Moq;
using Synap.SDK.Modules;
using System.Text.Json;
using Xunit;

namespace Synap.SDK.Tests.Modules;

public sealed class SetManagerTests
{
    private readonly Mock<SynapClient> _mockClient;
    private readonly SetManager _setManager;

    public SetManagerTests()
    {
        var config = new SynapConfig("http://localhost:15500");
        _mockClient = new Mock<SynapClient>(config);
        _setManager = new SetManager(_mockClient.Object);
    }

    [Fact]
    public async Task AddAsync_ShouldReturnAddedCount()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { added = 3 });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _setManager.AddAsync("tags", new List<string> { "python", "redis" });

        // Assert
        Assert.Equal(3, result);
    }

    [Fact]
    public async Task RemAsync_ShouldReturnRemovedCount()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { removed = 1 });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _setManager.RemAsync("tags", new List<string> { "redis" });

        // Assert
        Assert.Equal(1, result);
    }

    [Fact]
    public async Task IsMemberAsync_ShouldReturnTrue()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { is_member = true });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _setManager.IsMemberAsync("tags", "python");

        // Assert
        Assert.True(result);
    }

    [Fact]
    public async Task MembersAsync_ShouldReturnMembers()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { members = new[] { "python", "redis" } });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _setManager.MembersAsync("tags");

        // Assert
        Assert.Equal(2, result.Count);
    }

    [Fact]
    public async Task CardAsync_ShouldReturnCardinality()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { cardinality = 3 });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _setManager.CardAsync("tags");

        // Assert
        Assert.Equal(3, result);
    }

    [Fact]
    public async Task InterAsync_ShouldReturnIntersection()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { members = new[] { "python" } });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _setManager.InterAsync(new List<string> { "tags1", "tags2" });

        // Assert
        Assert.Single(result);
        Assert.Equal("python", result[0]);
    }

    [Fact]
    public async Task UnionAsync_ShouldReturnUnion()
    {
        // Arrange
        var responseJson = JsonSerializer.Serialize(new { members = new[] { "python", "redis", "typescript" } });
        var response = JsonDocument.Parse(responseJson);
        _mockClient
            .Setup(c => c.ExecuteAsync(It.IsAny<string>(), It.IsAny<string>(), It.IsAny<Dictionary<string, object?>>(), It.IsAny<CancellationToken>()))
            .ReturnsAsync(response);

        // Act
        var result = await _setManager.UnionAsync(new List<string> { "tags1", "tags2" });

        // Assert
        Assert.Equal(3, result.Count);
    }
}

