using Synap.SDK.Exceptions;

namespace Synap.SDK.Tests.Exceptions;

public sealed class SynapExceptionTests
{
    [Fact]
    public void Constructor_WithMessage_CreatesException()
    {
        // Arrange & Act
        var exception = new SynapException("Test error");

        // Assert
        Assert.Equal("Test error", exception.Message);
    }

    [Fact]
    public void Constructor_WithMessageAndInnerException_CreatesException()
    {
        // Arrange
        var innerException = new InvalidOperationException("Inner error");

        // Act
        var exception = new SynapException("Test error", innerException);

        // Assert
        Assert.Equal("Test error", exception.Message);
        Assert.Same(innerException, exception.InnerException);
    }

    [Fact]
    public void HttpError_CreatesFormattedMessage()
    {
        // Arrange & Act
        var exception = SynapException.HttpError("Request failed", 404);

        // Assert
        Assert.Contains("HTTP Error (404)", exception.Message);
        Assert.Contains("Request failed", exception.Message);
    }

    [Fact]
    public void ServerError_CreatesFormattedMessage()
    {
        // Arrange & Act
        var exception = SynapException.ServerError("Internal server error");

        // Assert
        Assert.Contains("Server Error", exception.Message);
        Assert.Contains("Internal server error", exception.Message);
    }

    [Fact]
    public void NetworkError_CreatesFormattedMessage()
    {
        // Arrange & Act
        var exception = SynapException.NetworkError("Connection timeout");

        // Assert
        Assert.Contains("Network Error", exception.Message);
        Assert.Contains("Connection timeout", exception.Message);
    }

    [Fact]
    public void InvalidResponse_CreatesFormattedMessage()
    {
        // Arrange & Act
        var exception = SynapException.InvalidResponse("Malformed JSON");

        // Assert
        Assert.Contains("Invalid Response", exception.Message);
        Assert.Contains("Malformed JSON", exception.Message);
    }

    [Fact]
    public void InvalidConfig_CreatesFormattedMessage()
    {
        // Arrange & Act
        var exception = SynapException.InvalidConfig("Missing URL");

        // Assert
        Assert.Contains("Invalid Configuration", exception.Message);
        Assert.Contains("Missing URL", exception.Message);
    }
}

