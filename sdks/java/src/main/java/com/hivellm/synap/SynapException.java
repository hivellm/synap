package com.hivellm.synap;

/**
 * Base exception for all Synap SDK errors.
 */
public class SynapException extends RuntimeException {

    /**
     * Creates a new SynapException with the given message.
     *
     * @param message the error message
     */
    public SynapException(String message) {
        super(message);
    }

    /**
     * Creates a new SynapException with the given message and cause.
     *
     * @param message the error message
     * @param cause   the underlying cause
     */
    public SynapException(String message, Throwable cause) {
        super(message, cause);
    }

    /**
     * Creates a network-level error exception.
     *
     * @param message description of the network failure
     * @return a new SynapException
     */
    public static SynapException networkError(String message) {
        return new SynapException("Network Error: " + message);
    }

    /**
     * Creates a network-level error exception wrapping a cause.
     *
     * @param message description of the network failure
     * @param cause   the underlying exception
     * @return a new SynapException
     */
    public static SynapException networkError(String message, Throwable cause) {
        return new SynapException("Network Error: " + message, cause);
    }

    /**
     * Creates a server-side error exception.
     *
     * @param message the error message returned by the server
     * @return a new SynapException
     */
    public static SynapException serverError(String message) {
        return new SynapException("Server Error: " + message);
    }

    /**
     * Creates an HTTP-level error exception.
     *
     * @param message    description of the failure
     * @param statusCode the HTTP status code received
     * @return a new SynapException
     */
    public static SynapException httpError(String message, int statusCode) {
        return new SynapException("HTTP Error (" + statusCode + "): " + message);
    }

    /**
     * Creates an invalid-response exception (e.g. unparseable JSON).
     *
     * @param message description of the parse failure
     * @return a new SynapException
     */
    public static SynapException invalidResponse(String message) {
        return new SynapException("Invalid Response: " + message);
    }

    /**
     * Creates an invalid-configuration exception.
     *
     * @param message description of the configuration problem
     * @return a new SynapException
     */
    public static SynapException invalidConfig(String message) {
        return new SynapException("Invalid Configuration: " + message);
    }
}
