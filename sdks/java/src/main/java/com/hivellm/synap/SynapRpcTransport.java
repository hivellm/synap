package com.hivellm.synap;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;

/**
 * SynapRPC binary transport.
 *
 * <p>Wire format: {@code [u32-LE length][msgpack body]}.
 *
 * <p>Request body (msgpack MAP of 3 fields):
 * <pre>{"id": uint32, "command": String, "args": [SynapValue…]}</pre>
 *
 * <p>Response body (msgpack array of 2 elements):
 * <pre>[id: int, {Ok: SynapValue} | {Err: String}]</pre>
 *
 * <p>SynapValue encoding (serde externally-tagged):
 * <ul>
 *   <li>{@code Null} → msgpack string {@code "Null"}</li>
 *   <li>{@code Str(x)} → msgpack map {@code {"Str": "x"}}</li>
 *   <li>{@code Int(42)} → msgpack map {@code {"Int": 42}}</li>
 *   <li>{@code Float(1.5)} → msgpack map {@code {"Float": 1.5}}</li>
 *   <li>{@code Bool(b)} → msgpack map {@code {"Bool": b}}</li>
 *   <li>{@code Bytes([…])} → msgpack map {@code {"Bytes": [bytes…]}}</li>
 *   <li>{@code Array([…])} → msgpack map {@code {"Array": [SynapValue…]}}</li>
 * </ul>
 *
 * <p>Thread-safe: socket I/O is serialized via {@code synchronized} blocks.
 * Connection is lazy (first use) with one auto-reconnect attempt on failure.</p>
 */
final class SynapRpcTransport implements Transport {

    private final String host;
    private final int    port;
    private final int    timeoutMillis;
    private final ObjectMapper mapper;

    private Socket       socket;
    private InputStream  in;
    private OutputStream out;

    private final AtomicInteger idGen = new AtomicInteger(1);

    SynapRpcTransport(String host, int port, int timeoutSeconds, ObjectMapper mapper) {
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

        // Convert args to SynapValue wire format.
        Object[] wireArgs = new Object[mapped.length - 1];
        for (int i = 1; i < mapped.length; i++) {
            wireArgs[i - 1] = toWireValue(mapped[i]);
        }

        Object rawResult = executeWithRetry(nativeCommand.toUpperCase(), wireArgs);
        return CommandMapper.mapResponse(command, fromWireValue(rawResult));
    }

    /** {@inheritDoc} */
    @Override
    public synchronized void close() {
        closeConnection();
    }

    // ── Core send/receive ──────────────────────────────────────────────────────

    /**
     * Sends the command and reads the response; retries once on connection error.
     */
    private Object executeWithRetry(String command, Object[] wireArgs) {
        try {
            return doExecute(command, wireArgs);
        } catch (IOException e) {
            // Attempt reconnect once.
            closeConnection();
            try {
                return doExecute(command, wireArgs);
            } catch (IOException e2) {
                throw SynapException.networkError("SynapRPC request failed after reconnect: " + e2.getMessage(), e2);
            }
        }
    }

    private synchronized Object doExecute(String command, Object[] wireArgs) throws IOException {
        ensureConnected();

        int id = idGen.getAndIncrement();

        // Build request as msgpack MAP: {"id": uint32, "command": "CMD", "args": [SynapValue...]}
        // The server expects a MAP (serde externally-tagged), NOT an array.
        LinkedHashMap<Object, Object> request = new LinkedHashMap<>(4);
        request.put("id",      (long) id);
        request.put("command", command);
        request.put("args",    Arrays.asList(wireArgs));
        byte[] msgBytes = MsgPackEncoder.encode(request);

        // Write 4-byte LE length prefix + body.
        byte[] lenBuf = ByteBuffer.allocate(4)
                .order(ByteOrder.LITTLE_ENDIAN)
                .putInt(msgBytes.length)
                .array();
        out.write(lenBuf);
        out.write(msgBytes);
        out.flush();

        // Read response length (4-byte LE).
        byte[] respLenBuf = readExact(in, 4);
        int respLen = ByteBuffer.wrap(respLenBuf).order(ByteOrder.LITTLE_ENDIAN).getInt();
        if (respLen < 0 || respLen > 64 * 1024 * 1024) {
            throw new IOException("Implausible response length: " + respLen);
        }

        // Read response body.
        byte[] respBuf = readExact(in, respLen);
        Object decoded = MsgPackDecoder.decode(respBuf);

        return parseResponse(id, decoded);
    }

    /**
     * Parses the decoded msgpack response array {@code [id, {Ok: val} | {Err: msg}]}.
     */
    private static Object parseResponse(int expectedId, Object decoded) {
        if (!(decoded instanceof List<?> arr) || arr.size() < 2) {
            throw SynapException.invalidResponse("Unexpected SynapRPC response shape");
        }

        Object resultEnvelope = arr.get(1);
        if (!(resultEnvelope instanceof Map<?, ?> resultMap)) {
            // Bare value — return as-is.
            return resultEnvelope;
        }

        if (resultMap.containsKey("Ok")) {
            return resultMap.get("Ok");
        }
        if (resultMap.containsKey("Err")) {
            Object err = resultMap.get("Err");
            throw SynapException.serverError(err != null ? err.toString() : "Unknown server error");
        }
        // Neither Ok nor Err — return the map itself.
        return resultEnvelope;
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
        in     = s.getInputStream();
        out    = s.getOutputStream();
    }

    private void closeConnection() {
        try {
            if (socket != null) {
                socket.close();
            }
        } catch (IOException ignored) {
            // best-effort
        }
        socket = null;
        in     = null;
        out    = null;
    }

    // ── I/O helpers ───────────────────────────────────────────────────────────

    private static byte[] readExact(InputStream stream, int length) throws IOException {
        byte[] buf = new byte[length];
        int offset = 0;
        while (offset < length) {
            int n = stream.read(buf, offset, length - offset);
            if (n == -1) {
                throw new IOException("Connection closed (expected " + length + " bytes, got " + offset + ")");
            }
            offset += n;
        }
        return buf;
    }

    // ── WireValue conversion ───────────────────────────────────────────────────

    /**
     * Wraps a plain Java value into the SynapRPC externally-tagged WireValue form.
     *
     * <ul>
     *   <li>null → "Null"</li>
     *   <li>String → {"Str": s}</li>
     *   <li>Long/Integer → {"Int": n}</li>
     *   <li>Double/Float → {"Float": d}</li>
     *   <li>Boolean → {"Bool": b}</li>
     *   <li>byte[] → {"Bytes": [bytes…]}</li>
     *   <li>List → {"Array": [wired items…]}</li>
     *   <li>int[] (used by QueueManager byte payloads) → {"Bytes": [ints as bytes]}</li>
     *   <li>anything else → {"Str": toString()}</li>
     * </ul>
     */
    static Object toWireValue(Object v) {
        if (v == null) return "Null";
        if (v instanceof Boolean b) return Map.of("Bool",  b);
        if (v instanceof String s)  return Map.of("Str",   s);
        if (v instanceof Long l)    return Map.of("Int",   l);
        if (v instanceof Integer i) return Map.of("Int",   (long) i);
        if (v instanceof Double d)  return Map.of("Float", d);
        if (v instanceof Float f)   return Map.of("Float", (double) f);
        if (v instanceof byte[] bytes) {
            List<Object> list = new ArrayList<>(bytes.length);
            for (byte b : bytes) list.add((long)(b & 0xFF));
            return Map.of("Bytes", list);
        }
        if (v instanceof int[] ints) {
            List<Object> list = new ArrayList<>(ints.length);
            for (int b : ints) list.add((long)(b & 0xFF));
            return Map.of("Bytes", list);
        }
        if (v instanceof List<?> list) {
            List<Object> wired = new ArrayList<>(list.size());
            for (Object item : list) wired.add(toWireValue(item));
            return Map.of("Array", wired);
        }
        if (v instanceof Object[] arr) {
            List<Object> wired = new ArrayList<>(arr.length);
            for (Object item : arr) wired.add(toWireValue(item));
            return Map.of("Array", wired);
        }
        // Fallback: stringify.
        return Map.of("Str", v.toString());
    }

    /**
     * Unwraps a SynapRPC WireValue returned from the server back to a plain Java value.
     *
     * <ul>
     *   <li>"Null" → null</li>
     *   <li>{"Str": s} → String</li>
     *   <li>{"Int": n} → Long</li>
     *   <li>{"Float": d} → Double</li>
     *   <li>{"Bool": b} → Boolean</li>
     *   <li>{"Bytes": […]} → List&lt;Long&gt;</li>
     *   <li>{"Array": […]} → List&lt;Object&gt; (recursively unwrapped)</li>
     *   <li>anything else → the value as-is</li>
     * </ul>
     */
    @SuppressWarnings("unchecked")
    static Object fromWireValue(Object wire) {
        if (wire == null) return null;
        if ("Null".equals(wire)) return null;

        if (wire instanceof Map<?, ?> map) {
            if (map.containsKey("Str"))   return map.get("Str");
            if (map.containsKey("Int"))   return toLong(map.get("Int"));
            if (map.containsKey("Float")) return toDouble(map.get("Float"));
            if (map.containsKey("Bool"))  return map.get("Bool");
            if (map.containsKey("Bytes")) return map.get("Bytes"); // List<Long>
            if (map.containsKey("Array")) {
                Object arr = map.get("Array");
                if (arr instanceof List<?> list) {
                    List<Object> result = new ArrayList<>(list.size());
                    for (Object item : list) result.add(fromWireValue(item));
                    return result;
                }
            }
            // Unrecognised map — pass through (for compound server responses).
            Map<String, Object> result = new HashMap<>();
            for (Map.Entry<?, ?> e : map.entrySet()) {
                result.put(e.getKey().toString(), fromWireValue(e.getValue()));
            }
            return result;
        }

        if (wire instanceof List<?> list) {
            List<Object> result = new ArrayList<>(list.size());
            for (Object item : list) result.add(fromWireValue(item));
            return result;
        }

        return wire; // Long, Boolean, Double, String — pass through
    }

    private static long toLong(Object o) {
        if (o instanceof Long l) return l;
        if (o instanceof Integer i) return i.longValue();
        if (o instanceof Double d) return d.longValue();
        if (o != null) {
            try { return Long.parseLong(o.toString()); } catch (NumberFormatException ignored) {}
        }
        return 0L;
    }

    private static double toDouble(Object o) {
        if (o instanceof Double d) return d;
        if (o instanceof Long l) return l.doubleValue();
        if (o instanceof Integer i) return i.doubleValue();
        if (o instanceof Float f) return f.doubleValue();
        if (o != null) {
            try { return Double.parseDouble(o.toString()); } catch (NumberFormatException ignored) {}
        }
        return 0.0;
    }

    // =========================================================================
    // Minimal MessagePack encoder
    // Handles: null, Boolean, Long, Integer, Double, Float, String, byte[],
    //          List<?>, Object[], Map<String,Object>, Map<Object,Object>
    // =========================================================================
    private static final class MsgPackEncoder {

        static byte[] encode(Object value) {
            ByteArrayOutputStream baos = new ByteArrayOutputStream(256);
            writeValue(baos, value);
            return baos.toByteArray();
        }

        @SuppressWarnings("unchecked")
        private static void writeValue(ByteArrayOutputStream s, Object v) {
            if (v == null) {
                s.write(0xc0); // nil
            } else if (v instanceof Boolean b) {
                s.write(b ? 0xc3 : 0xc2);
            } else if (v instanceof Long l) {
                writeInt64(s, l);
            } else if (v instanceof Integer i) {
                writeInt64(s, i);
            } else if (v instanceof Double d) {
                writeFloat64(s, d);
            } else if (v instanceof Float f) {
                writeFloat64(s, (double) f);
            } else if (v instanceof String str) {
                writeStr(s, str);
            } else if (v instanceof byte[] bytes) {
                writeBin(s, bytes);
            } else if (v instanceof Object[] arr) {
                writeArray(s, arr);
            } else if (v instanceof List<?> list) {
                writeList(s, (List<Object>) list);
            } else if (v instanceof Map<?, ?> map) {
                writeMap(s, (Map<Object, Object>) map);
            } else {
                writeStr(s, v.toString());
            }
        }

        private static void writeStr(ByteArrayOutputStream s, String str) {
            byte[] bytes = str.getBytes(StandardCharsets.UTF_8);
            int len = bytes.length;
            if (len <= 31) {
                s.write(0xa0 | len);
            } else if (len <= 0xff) {
                s.write(0xd9);
                s.write(len);
            } else if (len <= 0xffff) {
                s.write(0xda);
                writeBe16(s, (short) len);
            } else {
                s.write(0xdb);
                writeBe32(s, len);
            }
            try { s.write(bytes); } catch (IOException ignored) {}
        }

        private static void writeBin(ByteArrayOutputStream s, byte[] bytes) {
            int len = bytes.length;
            if (len <= 0xff) {
                s.write(0xc4);
                s.write(len);
            } else if (len <= 0xffff) {
                s.write(0xc5);
                writeBe16(s, (short) len);
            } else {
                s.write(0xc6);
                writeBe32(s, len);
            }
            try { s.write(bytes); } catch (IOException ignored) {}
        }

        private static void writeInt64(ByteArrayOutputStream s, long v) {
            if (v >= 0) {
                writeUInt64(s, v);
            } else if (v >= -32) {
                s.write((int)(0xe0 | (v + 32))); // negative fixint
            } else if (v >= -128) {
                s.write(0xd0);
                s.write((int)(byte) v);
            } else if (v >= -32768) {
                s.write(0xd1);
                writeBe16(s, (short) v);
            } else if (v >= Integer.MIN_VALUE) {
                s.write(0xd2);
                writeBe32(s, (int) v);
            } else {
                s.write(0xd3);
                writeBe64(s, v);
            }
        }

        private static void writeUInt64(ByteArrayOutputStream s, long v) {
            if (v <= 127) {
                s.write((int) v); // positive fixint
            } else if (v <= 0xff) {
                s.write(0xcc);
                s.write((int) v);
            } else if (v <= 0xffff) {
                s.write(0xcd);
                writeBe16(s, (short)(int) v);
            } else if (v <= 0xffffffffL) {
                s.write(0xce);
                writeBe32(s, (int) v);
            } else {
                s.write(0xcf);
                writeBe64(s, v);
            }
        }

        private static void writeFloat64(ByteArrayOutputStream s, double v) {
            s.write(0xcb);
            long bits = Double.doubleToRawLongBits(v);
            writeBe64(s, bits);
        }

        private static void writeArray(ByteArrayOutputStream s, Object[] arr) {
            int len = arr.length;
            if (len <= 15) {
                s.write(0x90 | len);
            } else if (len <= 0xffff) {
                s.write(0xdc);
                writeBe16(s, (short) len);
            } else {
                s.write(0xdd);
                writeBe32(s, len);
            }
            for (Object item : arr) writeValue(s, item);
        }

        @SuppressWarnings("unchecked")
        private static void writeList(ByteArrayOutputStream s, List<Object> list) {
            int len = list.size();
            if (len <= 15) {
                s.write(0x90 | len);
            } else if (len <= 0xffff) {
                s.write(0xdc);
                writeBe16(s, (short) len);
            } else {
                s.write(0xdd);
                writeBe32(s, len);
            }
            for (Object item : list) writeValue(s, item);
        }

        @SuppressWarnings("unchecked")
        private static void writeMap(ByteArrayOutputStream s, Map<Object, Object> map) {
            int len = map.size();
            if (len <= 15) {
                s.write(0x80 | len);
            } else if (len <= 0xffff) {
                s.write(0xde);
                writeBe16(s, (short) len);
            } else {
                s.write(0xdf);
                writeBe32(s, len);
            }
            for (Map.Entry<Object, Object> entry : map.entrySet()) {
                writeValue(s, entry.getKey());
                writeValue(s, entry.getValue());
            }
        }

        private static void writeBe16(ByteArrayOutputStream s, short v) {
            s.write((v >>> 8) & 0xff);
            s.write(v        & 0xff);
        }

        private static void writeBe32(ByteArrayOutputStream s, int v) {
            s.write((v >>> 24) & 0xff);
            s.write((v >>> 16) & 0xff);
            s.write((v >>> 8)  & 0xff);
            s.write( v         & 0xff);
        }

        private static void writeBe64(ByteArrayOutputStream s, long v) {
            for (int i = 7; i >= 0; i--) {
                s.write((int)((v >>> (i * 8)) & 0xff));
            }
        }
    }

    // =========================================================================
    // Minimal MessagePack decoder
    // Returns: null, Boolean, Long, Double, String, byte[],
    //          List<Object>, Map<Object,Object>
    // =========================================================================
    private static final class MsgPackDecoder {

        static Object decode(byte[] bytes) throws IOException {
            int[] pos = {0};
            return readValue(bytes, pos);
        }

        private static Object readValue(byte[] b, int[] pos) throws IOException {
            if (pos[0] >= b.length) throw new IOException("Unexpected end of MessagePack data");
            int hdr = b[pos[0]++] & 0xff;

            // positive fixint
            if (hdr <= 0x7f) return (long) hdr;
            // negative fixint
            if (hdr >= 0xe0) return (long)(byte) hdr;
            // fixstr
            if (hdr >= 0xa0 && hdr <= 0xbf) return readStr(b, pos, hdr & 0x1f);
            // fixarray
            if (hdr >= 0x90 && hdr <= 0x9f) return readArray(b, pos, hdr & 0x0f);
            // fixmap
            if (hdr >= 0x80 && hdr <= 0x8f) return readMap(b, pos, hdr & 0x0f);

            return switch (hdr) {
                case 0xc0 -> null;            // nil
                case 0xc2 -> Boolean.FALSE;
                case 0xc3 -> Boolean.TRUE;
                case 0xca -> (double) readFloat32(b, pos);
                case 0xcb -> readFloat64(b, pos);
                case 0xcc -> (long) readU8(b, pos);
                case 0xcd -> (long) readBe16(b, pos);
                case 0xce -> readBe32unsigned(b, pos);
                case 0xcf -> readBe64(b, pos);
                case 0xd0 -> (long)(byte) readU8(b, pos);
                case 0xd1 -> (long)(short) readBe16(b, pos);
                case 0xd2 -> (long)(int) (int) readBe32(b, pos);
                case 0xd3 -> readBe64(b, pos);
                case 0xd9 -> readStr(b, pos, readU8(b, pos));
                case 0xda -> readStr(b, pos, readBe16(b, pos));
                case 0xdb -> readStr(b, pos, (int) readBe32unsigned(b, pos));
                case 0xdc -> readArray(b, pos, readBe16(b, pos));
                case 0xdd -> readArray(b, pos, (int) readBe32unsigned(b, pos));
                case 0xde -> readMap(b, pos, readBe16(b, pos));
                case 0xdf -> readMap(b, pos, (int) readBe32unsigned(b, pos));
                case 0xc4 -> readBin(b, pos, readU8(b, pos));
                case 0xc5 -> readBin(b, pos, readBe16(b, pos));
                case 0xc6 -> readBin(b, pos, (int) readBe32unsigned(b, pos));
                default   -> throw new IOException(
                        String.format("Unknown MessagePack format byte 0x%02x at pos %d", hdr, pos[0] - 1));
            };
        }

        private static int readU8(byte[] b, int[] pos) throws IOException {
            checkBounds(b, pos, 1);
            return b[pos[0]++] & 0xff;
        }

        private static int readBe16(byte[] b, int[] pos) throws IOException {
            checkBounds(b, pos, 2);
            return ((b[pos[0]++] & 0xff) << 8) | (b[pos[0]++] & 0xff);
        }

        private static int readBe32(byte[] b, int[] pos) throws IOException {
            checkBounds(b, pos, 4);
            return ((b[pos[0]++] & 0xff) << 24)
                 | ((b[pos[0]++] & 0xff) << 16)
                 | ((b[pos[0]++] & 0xff) << 8)
                 |  (b[pos[0]++] & 0xff);
        }

        private static long readBe32unsigned(byte[] b, int[] pos) throws IOException {
            return readBe32(b, pos) & 0xFFFFFFFFL;
        }

        private static long readBe64(byte[] b, int[] pos) throws IOException {
            checkBounds(b, pos, 8);
            long v = 0;
            for (int i = 0; i < 8; i++) {
                v = (v << 8) | (b[pos[0]++] & 0xffL);
            }
            return v;
        }

        private static float readFloat32(byte[] b, int[] pos) throws IOException {
            int bits = readBe32(b, pos);
            return Float.intBitsToFloat(bits);
        }

        private static double readFloat64(byte[] b, int[] pos) throws IOException {
            long bits = readBe64(b, pos);
            return Double.longBitsToDouble(bits);
        }

        private static String readStr(byte[] b, int[] pos, int len) throws IOException {
            checkBounds(b, pos, len);
            String s = new String(b, pos[0], len, StandardCharsets.UTF_8);
            pos[0] += len;
            return s;
        }

        private static byte[] readBin(byte[] b, int[] pos, int len) throws IOException {
            checkBounds(b, pos, len);
            byte[] data = new byte[len];
            System.arraycopy(b, pos[0], data, 0, len);
            pos[0] += len;
            return data;
        }

        private static List<Object> readArray(byte[] b, int[] pos, int count) throws IOException {
            List<Object> list = new ArrayList<>(count);
            for (int i = 0; i < count; i++) {
                list.add(readValue(b, pos));
            }
            return list;
        }

        private static Map<Object, Object> readMap(byte[] b, int[] pos, int count) throws IOException {
            Map<Object, Object> map = new HashMap<>(count * 2);
            for (int i = 0; i < count; i++) {
                Object k = readValue(b, pos);
                Object v = readValue(b, pos);
                if (k != null) map.put(k, v);
            }
            return map;
        }

        private static void checkBounds(byte[] b, int[] pos, int need) throws IOException {
            if (pos[0] + need > b.length) {
                throw new IOException("Unexpected end of MessagePack data (need " + need
                        + " bytes at offset " + pos[0] + " of " + b.length + ")");
            }
        }
    }
}
