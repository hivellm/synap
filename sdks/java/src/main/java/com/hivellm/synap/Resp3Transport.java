package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * RESP3 transport (Redis Serialization Protocol v3 compatible).
 *
 * <p>Sends commands as RESP2 inline multibulk arrays and parses responses
 * according to the RESP2/RESP3 subset used by Synap:</p>
 * <ul>
 *   <li>{@code +SimpleString}</li>
 *   <li>{@code -Error}</li>
 *   <li>{@code :Integer}</li>
 *   <li>{@code $N\r\ndata\r\n} (BulkString)</li>
 *   <li>{@code *N\r\n…} (Array)</li>
 *   <li>{@code _\r\n} (Null — RESP3)</li>
 * </ul>
 *
 * <p>Thread-safe: all I/O is protected by {@code synchronized}.  Connection is
 * lazy with one auto-reconnect attempt on failure.</p>
 */
final class Resp3Transport implements Transport {

    private final String host;
    private final int    port;
    private final int    timeoutMillis;
    private final ObjectMapper mapper;

    private Socket          socket;
    private LineReader      reader;
    private OutputStream    writer;

    Resp3Transport(String host, int port, int timeoutSeconds, ObjectMapper mapper) {
        this.host          = host;
        this.port          = port;
        this.timeoutMillis = timeoutSeconds * 1000;
        this.mapper        = mapper;
    }

    // ── Transport interface ────────────────────────────────────────────────────

    /** {@inheritDoc} */
    @Override
    public JsonNode execute(String command, Map<String, Object> payload) {
        Object[] mapped = CommandMapper.mapCommand(command, payload);
        String nativeCommand = (String) mapped[0];

        // Build the args array (index 1..end).
        String[] args = new String[mapped.length - 1];
        for (int i = 1; i < mapped.length; i++) {
            args[i - 1] = toRespString(mapped[i]);
        }

        Object raw = executeWithRetry(nativeCommand.toUpperCase(), args);
        return CommandMapper.mapResponse(command, raw);
    }

    /** {@inheritDoc} */
    @Override
    public synchronized void close() {
        closeConnection();
    }

    // ── Core I/O ──────────────────────────────────────────────────────────────

    private Object executeWithRetry(String command, String[] args) {
        try {
            return doExecute(command, args);
        } catch (IOException e) {
            closeConnection();
            try {
                return doExecute(command, args);
            } catch (IOException e2) {
                throw SynapException.networkError("RESP3 request failed after reconnect: " + e2.getMessage(), e2);
            }
        }
    }

    private synchronized Object doExecute(String command, String[] args) throws IOException {
        ensureConnected();
        sendMultibulk(command, args);
        return readValue();
    }

    /**
     * Sends a RESP2 multibulk array: {@code *N\r\n$len\r\narg\r\n…}
     *
     * <p>Uses binary-safe bulk-string framing so that argument bytes are written
     * exactly, with the length prefix derived from the UTF-8 byte count (not the
     * Java String character count), ensuring correctness for non-ASCII values.</p>
     */
    private void sendMultibulk(String command, String[] args) throws IOException {
        ByteArrayOutputStream buf = new ByteArrayOutputStream(256);

        int total = 1 + args.length;
        writeAscii(buf, "*" + total + "\r\n");

        // command (always ASCII)
        byte[] cmdBytes = command.getBytes(StandardCharsets.UTF_8);
        writeAscii(buf, "$" + cmdBytes.length + "\r\n");
        buf.write(cmdBytes);
        writeAscii(buf, "\r\n");

        // arguments — binary-safe: compute byte length, then write raw bytes
        for (String arg : args) {
            byte[] argBytes = arg.getBytes(StandardCharsets.UTF_8);
            writeAscii(buf, "$" + argBytes.length + "\r\n");
            buf.write(argBytes);
            writeAscii(buf, "\r\n");
        }

        writer.write(buf.toByteArray());
        writer.flush();
    }

    /** Writes a pure-ASCII string to the buffer (no encoding overhead). */
    private static void writeAscii(ByteArrayOutputStream buf, String s) {
        // All control characters (\r, \n) and RESP protocol chars (*, $, :, +, -)
        // are in the ASCII range — getBytes(ISO_8859_1) is identical to getBytes(UTF_8)
        // for these and avoids allocating a Charset lookup.
        byte[] b = s.getBytes(StandardCharsets.US_ASCII);
        buf.write(b, 0, b.length);
    }

    // ── RESP parser ────────────────────────────────────────────────────────────

    /**
     * Reads a single RESP value from the stream.
     *
     * <p>Supports: {@code + - : $ * _} (simple string, error, integer,
     * bulk string, array, null).</p>
     */
    private Object readValue() throws IOException {
        String line = reader.readLine();
        if (line == null || line.isEmpty()) {
            throw new IOException("Connection closed or empty RESP line");
        }

        char prefix = line.charAt(0);
        String rest = line.length() > 1 ? line.substring(1) : "";

        return switch (prefix) {
            case '+' -> rest;                           // SimpleString
            case '-' -> throw SynapException.serverError(rest); // Error
            case ':' -> parseLong(rest);                // Integer
            case '_' -> null;                           // Null (RESP3)
            case '$' -> readBulkString(rest);           // BulkString
            case '*' -> readArray(parseInt(rest));      // Array
            default  -> throw SynapException.invalidResponse(
                                "Unknown RESP prefix '" + prefix + "' in line: " + line);
        };
    }

    /**
     * Reads a bulk string given the length prefix string.
     *
     * @param lenStr the string after {@code $}, e.g. {@code "5"} or {@code "-1"}
     */
    private Object readBulkString(String lenStr) throws IOException {
        int len = parseInt(lenStr);
        if (len == -1) return null; // null bulk string

        // Read exactly len bytes, then consume CRLF.
        byte[] data = new byte[len];
        int offset = 0;
        InputStream raw = reader.rawStream();
        while (offset < len) {
            int n = raw.read(data, offset, len - offset);
            if (n == -1) throw new IOException("Connection closed reading bulk string");
            offset += n;
        }
        // Consume trailing CRLF.
        int cr = raw.read();
        int lf = raw.read();
        if (cr != '\r' || lf != '\n') {
            throw new IOException("Expected CRLF after bulk string, got: " + cr + " " + lf);
        }
        return new String(data, StandardCharsets.UTF_8);
    }

    /**
     * Reads an array of {@code count} RESP values.
     */
    private Object readArray(int count) throws IOException {
        if (count == -1) return null;
        List<Object> list = new ArrayList<>(Math.max(count, 0));
        for (int i = 0; i < count; i++) {
            list.add(readValue());
        }
        return list;
    }

    // ── Connection management ──────────────────────────────────────────────────

    private void ensureConnected() throws IOException {
        if (socket != null && !socket.isClosed() && socket.isConnected()) {
            return;
        }
        closeConnection();
        Socket s = new Socket();
        s.setSoTimeout(timeoutMillis);
        s.connect(new InetSocketAddress(host, port), timeoutMillis);
        socket = s;
        BufferedInputStream bis = new BufferedInputStream(s.getInputStream(), 4096);
        reader  = new LineReader(bis);
        writer  = new BufferedOutputStream(s.getOutputStream(), 4096);
    }

    private void closeConnection() {
        try {
            if (socket != null) socket.close();
        } catch (IOException ignored) {}
        socket = null;
        reader = null;
        writer = null;
    }

    // ── Helpers ────────────────────────────────────────────────────────────────

    private static String toRespString(Object v) {
        if (v == null) return "";
        if (v instanceof Boolean b) return b ? "1" : "0";
        if (v instanceof Double d)  return Double.toString(d);
        if (v instanceof Float f)   return Float.toString(f);
        if (v instanceof Long l)    return Long.toString(l);
        if (v instanceof Integer i) return Integer.toString(i);
        return v.toString();
    }

    private static long parseLong(String s) {
        try { return Long.parseLong(s.trim()); }
        catch (NumberFormatException e) { return 0L; }
    }

    private static int parseInt(String s) {
        try { return Integer.parseInt(s.trim()); }
        catch (NumberFormatException e) { return 0; }
    }

    // ── Line reader (CRLF-terminated) ─────────────────────────────────────────

    /**
     * Minimal line reader that reads CRLF-terminated lines from a BufferedInputStream
     * while also allowing raw byte access for bulk-string bodies.
     */
    private static final class LineReader {

        private final BufferedInputStream stream;

        LineReader(BufferedInputStream stream) {
            this.stream = stream;
        }

        /** Returns the underlying stream for bulk-body reads. */
        InputStream rawStream() { return stream; }

        /**
         * Reads one CRLF-terminated line from the stream (strips the trailing CRLF).
         *
         * @return the line text, or {@code null} if the stream is closed
         */
        String readLine() throws IOException {
            StringBuilder sb = new StringBuilder(64);
            int c;
            while ((c = stream.read()) != -1) {
                if (c == '\r') {
                    int next = stream.read();
                    if (next == '\n') break;
                    if (next == -1)   break;
                    sb.append((char) c);
                    sb.append((char) next);
                } else {
                    sb.append((char) c);
                }
            }
            return c == -1 && sb.length() == 0 ? null : sb.toString();
        }
    }
}
