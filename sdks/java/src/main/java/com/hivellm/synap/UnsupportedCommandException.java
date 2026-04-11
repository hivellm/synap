package com.hivellm.synap;

/**
 * Thrown when a command has no mapping in the selected transport.
 *
 * <p>For the HTTP transport this should never be raised; it is reserved for
 * future native-transport support analogous to the C# SDK.</p>
 */
public class UnsupportedCommandException extends SynapException {

    private final String command;
    private final String transport;

    /**
     * Creates a new UnsupportedCommandException.
     *
     * @param command   the command name that could not be mapped
     * @param transport the transport that does not support the command
     */
    public UnsupportedCommandException(String command, String transport) {
        super("Command '" + command + "' is not supported by the '" + transport + "' transport");
        this.command = command;
        this.transport = transport;
    }

    /**
     * Returns the unsupported command name.
     *
     * @return command name
     */
    public String getCommand() {
        return command;
    }

    /**
     * Returns the transport that does not support this command.
     *
     * @return transport name
     */
    public String getTransport() {
        return transport;
    }
}
