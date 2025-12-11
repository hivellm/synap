using Synap.SDK.Exceptions;

namespace Synap.SDK.Tests;

public sealed class SynapConfigTests
{
    [Fact]
    public void Constructor_WithValidUrl_CreatesConfig()
    {
        // Arrange & Act
        var config = new SynapConfig("http://localhost:15500");

        // Assert
        Assert.Equal("http://localhost:15500", config.BaseUrl);
        Assert.Equal(30, config.Timeout);
        Assert.Null(config.AuthToken);
        Assert.Equal(3, config.MaxRetries);
    }

    [Fact]
    public void Constructor_WithTrailingSlash_RemovesSlash()
    {
        // Arrange & Act
        var config = new SynapConfig("http://localhost:15500/");

        // Assert
        Assert.Equal("http://localhost:15500", config.BaseUrl);
    }

    [Theory]
    [InlineData("")]
    [InlineData("   ")]
    public void Constructor_WithEmptyUrl_ThrowsException(string url)
    {
        // Act & Assert
        var exception = Assert.Throws<SynapException>(() => new SynapConfig(url));
        Assert.Contains("Base URL cannot be empty", exception.Message);
    }

    [Fact]
    public void Create_ReturnsNewConfig()
    {
        // Arrange & Act
        var config = SynapConfig.Create("http://localhost:15500");

        // Assert
        Assert.NotNull(config);
        Assert.Equal("http://localhost:15500", config.BaseUrl);
    }

    [Fact]
    public void WithTimeout_ReturnsNewConfigWithUpdatedTimeout()
    {
        // Arrange
        var config = SynapConfig.Create("http://localhost:15500");

        // Act
        var newConfig = config.WithTimeout(60);

        // Assert
        Assert.Equal(30, config.Timeout);
        Assert.Equal(60, newConfig.Timeout);
        Assert.NotSame(config, newConfig);
    }

    [Fact]
    public void WithAuthToken_ReturnsNewConfigWithToken()
    {
        // Arrange
        var config = SynapConfig.Create("http://localhost:15500");

        // Act
        var newConfig = config.WithAuthToken("test-token");

        // Assert
        Assert.Null(config.AuthToken);
        Assert.Equal("test-token", newConfig.AuthToken);
        Assert.NotSame(config, newConfig);
    }

    [Fact]
    public void WithMaxRetries_ReturnsNewConfigWithUpdatedRetries()
    {
        // Arrange
        var config = SynapConfig.Create("http://localhost:15500");

        // Act
        var newConfig = config.WithMaxRetries(5);

        // Assert
        Assert.Equal(3, config.MaxRetries);
        Assert.Equal(5, newConfig.MaxRetries);
        Assert.NotSame(config, newConfig);
    }

    [Fact]
    public void ChainedWith_AppliesAllChanges()
    {
        // Arrange
        var config = SynapConfig.Create("http://localhost:15500");

        // Act
        var newConfig = config
            .WithTimeout(60)
            .WithAuthToken("my-token")
            .WithMaxRetries(5);

        // Assert
        Assert.Equal(60, newConfig.Timeout);
        Assert.Equal("my-token", newConfig.AuthToken);
        Assert.Equal(5, newConfig.MaxRetries);
    }
}

