package synap_test

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	synap "github.com/hivellm/synap-sdk-go"
)

// mockServer creates a test HTTP server that captures the last request body
// and returns the supplied response JSON for every request.
func mockServer(t *testing.T, responseJSON string) (*httptest.Server, *string) {
	t.Helper()
	var lastBody string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			t.Errorf("expected POST, got %s", r.Method)
		}
		if r.Header.Get("Content-Type") != "application/json" {
			t.Errorf("expected Content-Type application/json, got %s", r.Header.Get("Content-Type"))
		}
		var buf struct{ raw json.RawMessage }
		_ = json.NewDecoder(r.Body).Decode(&buf)
		rawBytes, _ := json.Marshal(buf.raw)
		lastBody = string(rawBytes)
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(responseJSON))
	}))
	return srv, &lastBody
}

// mockServerFn creates a test HTTP server with a custom handler function.
func mockServerFn(t *testing.T, fn func(cmd string, payload json.RawMessage) string) *httptest.Server {
	t.Helper()
	return httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var env struct {
			Command string          `json:"command"`
			Payload json.RawMessage `json:"payload"`
		}
		if err := json.NewDecoder(r.Body).Decode(&env); err != nil {
			http.Error(w, "bad request", http.StatusBadRequest)
			return
		}
		resp := fn(env.Command, env.Payload)
		w.Header().Set("Content-Type", "application/json")
		_, _ = w.Write([]byte(resp))
	}))
}

func newTestClient(baseURL string) *synap.SynapClient {
	return synap.NewClient(synap.NewConfig(baseURL))
}

// ── Config ────────────────────────────────────────────────────────────────────

func TestNewConfig_Defaults(t *testing.T) {
	cfg := synap.NewConfig("http://localhost:15500")
	if cfg == nil {
		t.Fatal("expected non-nil config")
	}
}

func TestConfig_WithAuth(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		auth := r.Header.Get("Authorization")
		if auth != "Bearer my-token" {
			t.Errorf("expected Bearer my-token, got %s", auth)
		}
		_, _ = w.Write([]byte(`{"success":true,"request_id":"x","payload":null}`))
	}))
	defer srv.Close()

	client := synap.NewClient(synap.NewConfig(srv.URL).WithAuth("my-token"))
	// kv.stats sends an empty payload; we just care that the auth header was set.
	_, _ = client.KV().Stats(context.Background())
}

func TestConfig_WithBasicAuth(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		auth := r.Header.Get("Authorization")
		if len(auth) < 6 || auth[:6] != "Basic " {
			t.Errorf("expected Basic auth, got %s", auth)
		}
		_, _ = w.Write([]byte(`{"success":true,"request_id":"x","payload":null}`))
	}))
	defer srv.Close()

	client := synap.NewClient(synap.NewConfig(srv.URL).WithBasicAuth("user", "pass"))
	_, _ = client.KV().Stats(context.Background())
}

func TestConfig_WithTimeout(t *testing.T) {
	cfg := synap.NewConfig("http://localhost:15500").WithTimeout(5 * time.Second)
	if cfg == nil {
		t.Fatal("expected non-nil config")
	}
}

// ── Error handling ────────────────────────────────────────────────────────────

func TestServerError_Propagated(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		_, _ = w.Write([]byte(`{"success":false,"request_id":"x","payload":null,"error":"key not found"}`))
	}))
	defer srv.Close()

	_, err := newTestClient(srv.URL).KV().Get(context.Background(), "missing")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
}

func TestHTTPError_Propagated(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		http.Error(w, "internal server error", http.StatusInternalServerError)
	}))
	defer srv.Close()

	_, err := newTestClient(srv.URL).KV().Get(context.Background(), "k")
	if err == nil {
		t.Fatal("expected error, got nil")
	}
}

func TestSynapError_Format(t *testing.T) {
	e := &synap.SynapError{Code: "not_found", Message: "key missing"}
	if e.Error() == "" {
		t.Fatal("expected non-empty error string")
	}
}

// ── KV ────────────────────────────────────────────────────────────────────────

func TestKV_Set(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "kv.set" {
			t.Errorf("expected kv.set, got %s", cmd)
		}
		var p struct {
			Key   string  `json:"key"`
			Value string  `json:"value"`
			TTL   *uint64 `json:"ttl"`
		}
		if err := json.Unmarshal(payload, &p); err != nil {
			t.Fatal(err)
		}
		if p.Key != "hello" {
			t.Errorf("expected key=hello, got %s", p.Key)
		}
		if p.Value != "world" {
			t.Errorf("expected value=world, got %s", p.Value)
		}
		if p.TTL != nil {
			t.Errorf("expected nil TTL, got %v", *p.TTL)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).KV().Set(context.Background(), "hello", "world", 0)
	if err != nil {
		t.Fatal(err)
	}
}

func TestKV_Set_WithTTL(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		var p struct {
			TTL *uint64 `json:"ttl"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.TTL == nil || *p.TTL != 3600 {
			t.Errorf("expected TTL=3600, got %v", p.TTL)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).KV().Set(context.Background(), "k", "v", time.Hour)
	if err != nil {
		t.Fatal(err)
	}
}

func TestKV_Get_Found(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "kv.get" {
			t.Errorf("expected kv.get, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":"world"}`
	})
	defer srv.Close()

	val, err := newTestClient(srv.URL).KV().Get(context.Background(), "hello")
	if err != nil {
		t.Fatal(err)
	}
	if val != "world" {
		t.Errorf("expected world, got %s", val)
	}
}

func TestKV_Get_NotFound(t *testing.T) {
	srv := mockServerFn(t, func(_ string, _ json.RawMessage) string {
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	val, err := newTestClient(srv.URL).KV().Get(context.Background(), "missing")
	if err != nil {
		t.Fatal(err)
	}
	if val != "" {
		t.Errorf("expected empty string for missing key, got %s", val)
	}
}

func TestKV_Delete_Found(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "kv.del" {
			t.Errorf("expected kv.del, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"deleted":true}}`
	})
	defer srv.Close()

	deleted, err := newTestClient(srv.URL).KV().Delete(context.Background(), "k")
	if err != nil {
		t.Fatal(err)
	}
	if !deleted {
		t.Error("expected deleted=true")
	}
}

func TestKV_Delete_Missing(t *testing.T) {
	srv := mockServerFn(t, func(_ string, _ json.RawMessage) string {
		return `{"success":true,"request_id":"x","payload":{"deleted":false}}`
	})
	defer srv.Close()

	deleted, err := newTestClient(srv.URL).KV().Delete(context.Background(), "gone")
	if err != nil {
		t.Fatal(err)
	}
	if deleted {
		t.Error("expected deleted=false for missing key")
	}
}

func TestKV_Exists_True(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "kv.exists" {
			t.Errorf("expected kv.exists, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"exists":true}}`
	})
	defer srv.Close()

	exists, err := newTestClient(srv.URL).KV().Exists(context.Background(), "k")
	if err != nil {
		t.Fatal(err)
	}
	if !exists {
		t.Error("expected exists=true")
	}
}

func TestKV_Exists_False(t *testing.T) {
	srv := mockServerFn(t, func(_ string, _ json.RawMessage) string {
		return `{"success":true,"request_id":"x","payload":{"exists":false}}`
	})
	defer srv.Close()

	exists, err := newTestClient(srv.URL).KV().Exists(context.Background(), "gone")
	if err != nil {
		t.Fatal(err)
	}
	if exists {
		t.Error("expected exists=false")
	}
}

func TestKV_Incr(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "kv.incr" {
			t.Errorf("expected kv.incr, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"value":5}}`
	})
	defer srv.Close()

	val, err := newTestClient(srv.URL).KV().Incr(context.Background(), "counter")
	if err != nil {
		t.Fatal(err)
	}
	if val != 5 {
		t.Errorf("expected 5, got %d", val)
	}
}

func TestKV_Decr(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "kv.decr" {
			t.Errorf("expected kv.decr, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"value":3}}`
	})
	defer srv.Close()

	val, err := newTestClient(srv.URL).KV().Decr(context.Background(), "counter")
	if err != nil {
		t.Fatal(err)
	}
	if val != 3 {
		t.Errorf("expected 3, got %d", val)
	}
}

func TestKV_Stats(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "kv.stats" {
			t.Errorf("expected kv.stats, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"total_keys":10,"total_memory_bytes":1024,"hit_rate":0.95}}`
	})
	defer srv.Close()

	stats, err := newTestClient(srv.URL).KV().Stats(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if stats.TotalKeys != 10 {
		t.Errorf("expected total_keys=10, got %d", stats.TotalKeys)
	}
	if stats.TotalMemoryBytes != 1024 {
		t.Errorf("expected total_memory_bytes=1024, got %d", stats.TotalMemoryBytes)
	}
	if stats.HitRate != 0.95 {
		t.Errorf("expected hit_rate=0.95, got %f", stats.HitRate)
	}
}

// ── Queue ─────────────────────────────────────────────────────────────────────

func TestQueue_Create(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "queue.create" {
			t.Errorf("expected queue.create, got %s", cmd)
		}
		var p struct {
			Name string `json:"name"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Name != "tasks" {
			t.Errorf("expected name=tasks, got %s", p.Name)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).Queue().Create(context.Background(), "tasks", 0, 0)
	if err != nil {
		t.Fatal(err)
	}
}

func TestQueue_Create_WithOptions(t *testing.T) {
	srv := mockServerFn(t, func(_ string, payload json.RawMessage) string {
		var p struct {
			Config *struct {
				MaxDepth        int `json:"max_depth"`
				AckDeadlineSecs int `json:"ack_deadline_secs"`
			} `json:"config"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Config == nil {
			t.Error("expected config block")
		} else {
			if p.Config.MaxDepth != 100 {
				t.Errorf("expected max_depth=100, got %d", p.Config.MaxDepth)
			}
			if p.Config.AckDeadlineSecs != 30 {
				t.Errorf("expected ack_deadline_secs=30, got %d", p.Config.AckDeadlineSecs)
			}
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).Queue().Create(context.Background(), "tasks", 100, 30)
	if err != nil {
		t.Fatal(err)
	}
}

func TestQueue_Publish(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "queue.publish" {
			t.Errorf("expected queue.publish, got %s", cmd)
		}
		var p struct {
			Queue    string `json:"queue"`
			Priority uint8  `json:"priority"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Queue != "tasks" {
			t.Errorf("expected queue=tasks, got %s", p.Queue)
		}
		if p.Priority != 5 {
			t.Errorf("expected priority=5, got %d", p.Priority)
		}
		return `{"success":true,"request_id":"x","payload":{"message_id":"msg-123"}}`
	})
	defer srv.Close()

	id, err := newTestClient(srv.URL).Queue().Publish(context.Background(), "tasks", []byte("work"), 5, 0)
	if err != nil {
		t.Fatal(err)
	}
	if id != "msg-123" {
		t.Errorf("expected msg-123, got %s", id)
	}
}

func TestQueue_Consume_WithMessage(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "queue.consume" {
			t.Errorf("expected queue.consume, got %s", cmd)
		}
		var p struct {
			Queue      string `json:"queue"`
			ConsumerID string `json:"consumer_id"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Queue != "tasks" {
			t.Errorf("expected queue=tasks, got %s", p.Queue)
		}
		if p.ConsumerID != "worker-1" {
			t.Errorf("expected consumer_id=worker-1, got %s", p.ConsumerID)
		}
		return `{"success":true,"request_id":"x","payload":{"message":{"id":"msg-1","payload":[104,105],"priority":5,"retry_count":0,"max_retries":3}}}`
	})
	defer srv.Close()

	msg, err := newTestClient(srv.URL).Queue().Consume(context.Background(), "tasks", "worker-1")
	if err != nil {
		t.Fatal(err)
	}
	if msg == nil {
		t.Fatal("expected a message, got nil")
	}
	if msg.ID != "msg-1" {
		t.Errorf("expected id=msg-1, got %s", msg.ID)
	}
}

func TestQueue_Consume_Empty(t *testing.T) {
	srv := mockServerFn(t, func(_ string, _ json.RawMessage) string {
		return `{"success":true,"request_id":"x","payload":{"message":null}}`
	})
	defer srv.Close()

	msg, err := newTestClient(srv.URL).Queue().Consume(context.Background(), "tasks", "w1")
	if err != nil {
		t.Fatal(err)
	}
	if msg != nil {
		t.Errorf("expected nil message for empty queue, got %+v", msg)
	}
}

func TestQueue_Ack(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "queue.ack" {
			t.Errorf("expected queue.ack, got %s", cmd)
		}
		var p struct {
			Queue     string `json:"queue"`
			MessageID string `json:"message_id"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.MessageID != "msg-1" {
			t.Errorf("expected message_id=msg-1, got %s", p.MessageID)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).Queue().Ack(context.Background(), "tasks", "msg-1")
	if err != nil {
		t.Fatal(err)
	}
}

func TestQueue_Nack(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "queue.nack" {
			t.Errorf("expected queue.nack, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).Queue().Nack(context.Background(), "tasks", "msg-1")
	if err != nil {
		t.Fatal(err)
	}
}

func TestQueue_Stats(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "queue.stats" {
			t.Errorf("expected queue.stats, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"depth":3,"consumers":1,"published":10,"consumed":7,"acked":7,"nacked":0,"dead_lettered":0}}`
	})
	defer srv.Close()

	stats, err := newTestClient(srv.URL).Queue().Stats(context.Background(), "tasks")
	if err != nil {
		t.Fatal(err)
	}
	if stats.Depth != 3 {
		t.Errorf("expected depth=3, got %d", stats.Depth)
	}
	if stats.Published != 10 {
		t.Errorf("expected published=10, got %d", stats.Published)
	}
}

func TestQueue_List(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "queue.list" {
			t.Errorf("expected queue.list, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"queues":["tasks","emails"]}}`
	})
	defer srv.Close()

	queues, err := newTestClient(srv.URL).Queue().List(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(queues) != 2 {
		t.Fatalf("expected 2 queues, got %d", len(queues))
	}
	if queues[0] != "tasks" || queues[1] != "emails" {
		t.Errorf("unexpected queues: %v", queues)
	}
}

func TestQueue_Delete(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "queue.delete" {
			t.Errorf("expected queue.delete, got %s", cmd)
		}
		var p struct {
			Queue string `json:"queue"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Queue != "tasks" {
			t.Errorf("expected queue=tasks, got %s", p.Queue)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).Queue().Delete(context.Background(), "tasks")
	if err != nil {
		t.Fatal(err)
	}
}

// ── Stream ────────────────────────────────────────────────────────────────────

func TestStream_Create(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "stream.create" {
			t.Errorf("expected stream.create, got %s", cmd)
		}
		var p struct {
			Room string `json:"room"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Room != "chat" {
			t.Errorf("expected room=chat, got %s", p.Room)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).Stream().Create(context.Background(), "chat", 0)
	if err != nil {
		t.Fatal(err)
	}
}

func TestStream_Publish(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "stream.publish" {
			t.Errorf("expected stream.publish, got %s", cmd)
		}
		var p struct {
			Room  string `json:"room"`
			Event string `json:"event"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Room != "chat" {
			t.Errorf("expected room=chat, got %s", p.Room)
		}
		if p.Event != "message" {
			t.Errorf("expected event=message, got %s", p.Event)
		}
		return `{"success":true,"request_id":"x","payload":{"offset":42}}`
	})
	defer srv.Close()

	offset, err := newTestClient(srv.URL).Stream().Publish(context.Background(), "chat", "message", map[string]string{"text": "hello"})
	if err != nil {
		t.Fatal(err)
	}
	if offset != 42 {
		t.Errorf("expected offset=42, got %d", offset)
	}
}

func TestStream_Consume(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "stream.consume" {
			t.Errorf("expected stream.consume, got %s", cmd)
		}
		var p struct {
			Room         string `json:"room"`
			SubscriberID string `json:"subscriber_id"`
			FromOffset   uint64 `json:"from_offset"`
			Limit        int    `json:"limit"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.SubscriberID != "sdk-default" {
			t.Errorf("expected subscriber_id=sdk-default, got %s", p.SubscriberID)
		}
		if p.FromOffset != 0 {
			t.Errorf("expected from_offset=0, got %d", p.FromOffset)
		}
		return `{"success":true,"request_id":"x","payload":{"events":[{"offset":1,"event":"message","data":{"text":"hello"}}]}}`
	})
	defer srv.Close()

	events, err := newTestClient(srv.URL).Stream().Consume(context.Background(), "chat", 0, 10)
	if err != nil {
		t.Fatal(err)
	}
	if len(events) != 1 {
		t.Fatalf("expected 1 event, got %d", len(events))
	}
	if events[0].Offset != 1 {
		t.Errorf("expected offset=1, got %d", events[0].Offset)
	}
	if events[0].Event != "message" {
		t.Errorf("expected event=message, got %s", events[0].Event)
	}
}

func TestStream_List(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "stream.list" {
			t.Errorf("expected stream.list, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"rooms":["chat","logs"]}}`
	})
	defer srv.Close()

	rooms, err := newTestClient(srv.URL).Stream().List(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(rooms) != 2 {
		t.Fatalf("expected 2 rooms, got %d", len(rooms))
	}
}

func TestStream_Delete(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "stream.delete" {
			t.Errorf("expected stream.delete, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).Stream().Delete(context.Background(), "chat")
	if err != nil {
		t.Fatal(err)
	}
}

// ── PubSub ────────────────────────────────────────────────────────────────────

func TestPubSub_Publish(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "pubsub.publish" {
			t.Errorf("expected pubsub.publish, got %s", cmd)
		}
		var p struct {
			Topic    string `json:"topic"`
			Priority uint8  `json:"priority"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Topic != "user.created" {
			t.Errorf("expected topic=user.created, got %s", p.Topic)
		}
		if p.Priority != 3 {
			t.Errorf("expected priority=3, got %d", p.Priority)
		}
		return `{"success":true,"request_id":"x","payload":{"subscribers_matched":5}}`
	})
	defer srv.Close()

	n, err := newTestClient(srv.URL).PubSub().Publish(context.Background(), "user.created", map[string]int{"id": 1}, 3)
	if err != nil {
		t.Fatal(err)
	}
	if n != 5 {
		t.Errorf("expected 5 subscribers, got %d", n)
	}
}

func TestPubSub_Subscribe(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "pubsub.subscribe" {
			t.Errorf("expected pubsub.subscribe, got %s", cmd)
		}
		var p struct {
			SubscriberID string   `json:"subscriber_id"`
			Topics       []string `json:"topics"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.SubscriberID != "svc-1" {
			t.Errorf("expected subscriber_id=svc-1, got %s", p.SubscriberID)
		}
		if len(p.Topics) != 2 {
			t.Errorf("expected 2 topics, got %d", len(p.Topics))
		}
		return `{"success":true,"request_id":"x","payload":{"subscriber_id":"svc-1"}}`
	})
	defer srv.Close()

	id, err := newTestClient(srv.URL).PubSub().Subscribe(context.Background(), "svc-1", []string{"user.*", "order.#"})
	if err != nil {
		t.Fatal(err)
	}
	if id != "svc-1" {
		t.Errorf("expected svc-1, got %s", id)
	}
}

func TestPubSub_Unsubscribe(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "pubsub.unsubscribe" {
			t.Errorf("expected pubsub.unsubscribe, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":null}`
	})
	defer srv.Close()

	err := newTestClient(srv.URL).PubSub().Unsubscribe(context.Background(), "svc-1", []string{"user.*"})
	if err != nil {
		t.Fatal(err)
	}
}

func TestPubSub_ListTopics(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "pubsub.topics" {
			t.Errorf("expected pubsub.topics, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"topics":["user.created","order.placed"]}}`
	})
	defer srv.Close()

	topics, err := newTestClient(srv.URL).PubSub().ListTopics(context.Background())
	if err != nil {
		t.Fatal(err)
	}
	if len(topics) != 2 {
		t.Fatalf("expected 2 topics, got %d", len(topics))
	}
}

// ── Hash ──────────────────────────────────────────────────────────────────────

func TestHash_Set(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "hash.set" {
			t.Errorf("expected hash.set, got %s", cmd)
		}
		var p struct {
			Key   string `json:"key"`
			Field string `json:"field"`
			Value string `json:"value"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Key != "user:1" || p.Field != "name" || p.Value != "Alice" {
			t.Errorf("unexpected payload: %+v", p)
		}
		return `{"success":true,"request_id":"x","payload":{"success":true}}`
	})
	defer srv.Close()

	ok, err := newTestClient(srv.URL).Hash().Set(context.Background(), "user:1", "name", "Alice")
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Error("expected success=true")
	}
}

func TestHash_Get(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "hash.get" {
			t.Errorf("expected hash.get, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"value":"Alice"}}`
	})
	defer srv.Close()

	val, err := newTestClient(srv.URL).Hash().Get(context.Background(), "user:1", "name")
	if err != nil {
		t.Fatal(err)
	}
	if val != "Alice" {
		t.Errorf("expected Alice, got %s", val)
	}
}

func TestHash_GetAll(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "hash.getall" {
			t.Errorf("expected hash.getall, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"fields":{"name":"Alice","age":"30"}}}`
	})
	defer srv.Close()

	fields, err := newTestClient(srv.URL).Hash().GetAll(context.Background(), "user:1")
	if err != nil {
		t.Fatal(err)
	}
	if fields["name"] != "Alice" {
		t.Errorf("expected name=Alice, got %s", fields["name"])
	}
	if fields["age"] != "30" {
		t.Errorf("expected age=30, got %s", fields["age"])
	}
}

func TestHash_Del(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "hash.del" {
			t.Errorf("expected hash.del, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"deleted":1}}`
	})
	defer srv.Close()

	n, err := newTestClient(srv.URL).Hash().Del(context.Background(), "user:1", "name")
	if err != nil {
		t.Fatal(err)
	}
	if n != 1 {
		t.Errorf("expected 1 deleted, got %d", n)
	}
}

func TestHash_Exists(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "hash.exists" {
			t.Errorf("expected hash.exists, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"exists":true}}`
	})
	defer srv.Close()

	exists, err := newTestClient(srv.URL).Hash().Exists(context.Background(), "user:1", "name")
	if err != nil {
		t.Fatal(err)
	}
	if !exists {
		t.Error("expected exists=true")
	}
}

// ── List ──────────────────────────────────────────────────────────────────────

func TestList_LPush(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "list.lpush" {
			t.Errorf("expected list.lpush, got %s", cmd)
		}
		var p struct {
			Key    string   `json:"key"`
			Values []string `json:"values"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Key != "mylist" {
			t.Errorf("expected key=mylist, got %s", p.Key)
		}
		if len(p.Values) != 2 {
			t.Errorf("expected 2 values, got %d", len(p.Values))
		}
		return `{"success":true,"request_id":"x","payload":{"length":2}}`
	})
	defer srv.Close()

	n, err := newTestClient(srv.URL).List().LPush(context.Background(), "mylist", []string{"a", "b"})
	if err != nil {
		t.Fatal(err)
	}
	if n != 2 {
		t.Errorf("expected length=2, got %d", n)
	}
}

func TestList_RPush(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "list.rpush" {
			t.Errorf("expected list.rpush, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"length":3}}`
	})
	defer srv.Close()

	n, err := newTestClient(srv.URL).List().RPush(context.Background(), "mylist", []string{"c"})
	if err != nil {
		t.Fatal(err)
	}
	if n != 3 {
		t.Errorf("expected length=3, got %d", n)
	}
}

func TestList_LPop(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "list.lpop" {
			t.Errorf("expected list.lpop, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"values":["a"]}}`
	})
	defer srv.Close()

	vals, err := newTestClient(srv.URL).List().LPop(context.Background(), "mylist", 1)
	if err != nil {
		t.Fatal(err)
	}
	if len(vals) != 1 || vals[0] != "a" {
		t.Errorf("unexpected values: %v", vals)
	}
}

func TestList_RPop(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "list.rpop" {
			t.Errorf("expected list.rpop, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"values":["c"]}}`
	})
	defer srv.Close()

	vals, err := newTestClient(srv.URL).List().RPop(context.Background(), "mylist", 1)
	if err != nil {
		t.Fatal(err)
	}
	if len(vals) != 1 || vals[0] != "c" {
		t.Errorf("unexpected values: %v", vals)
	}
}

func TestList_Range(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "list.range" {
			t.Errorf("expected list.range, got %s", cmd)
		}
		var p struct {
			Start int `json:"start"`
			Stop  int `json:"stop"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Start != 0 || p.Stop != -1 {
			t.Errorf("unexpected range: %d..%d", p.Start, p.Stop)
		}
		return `{"success":true,"request_id":"x","payload":{"values":["a","b","c"]}}`
	})
	defer srv.Close()

	vals, err := newTestClient(srv.URL).List().Range(context.Background(), "mylist", 0, -1)
	if err != nil {
		t.Fatal(err)
	}
	if len(vals) != 3 {
		t.Fatalf("expected 3 values, got %d", len(vals))
	}
}

func TestList_Len(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "list.len" {
			t.Errorf("expected list.len, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"length":5}}`
	})
	defer srv.Close()

	n, err := newTestClient(srv.URL).List().Len(context.Background(), "mylist")
	if err != nil {
		t.Fatal(err)
	}
	if n != 5 {
		t.Errorf("expected 5, got %d", n)
	}
}

// ── Set ───────────────────────────────────────────────────────────────────────

func TestSet_Add(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "set.add" {
			t.Errorf("expected set.add, got %s", cmd)
		}
		var p struct {
			Key     string   `json:"key"`
			Members []string `json:"members"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Key != "myset" {
			t.Errorf("expected key=myset, got %s", p.Key)
		}
		return `{"success":true,"request_id":"x","payload":{"added":2}}`
	})
	defer srv.Close()

	n, err := newTestClient(srv.URL).Set().Add(context.Background(), "myset", []string{"a", "b"})
	if err != nil {
		t.Fatal(err)
	}
	if n != 2 {
		t.Errorf("expected 2, got %d", n)
	}
}

func TestSet_Members(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "set.members" {
			t.Errorf("expected set.members, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"members":["a","b"]}}`
	})
	defer srv.Close()

	members, err := newTestClient(srv.URL).Set().Members(context.Background(), "myset")
	if err != nil {
		t.Fatal(err)
	}
	if len(members) != 2 {
		t.Fatalf("expected 2 members, got %d", len(members))
	}
}

func TestSet_IsMember_True(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, payload json.RawMessage) string {
		if cmd != "set.ismember" {
			t.Errorf("expected set.ismember, got %s", cmd)
		}
		var p struct {
			Member string `json:"member"`
		}
		_ = json.Unmarshal(payload, &p)
		if p.Member != "a" {
			t.Errorf("expected member=a, got %s", p.Member)
		}
		return `{"success":true,"request_id":"x","payload":{"is_member":true}}`
	})
	defer srv.Close()

	ok, err := newTestClient(srv.URL).Set().IsMember(context.Background(), "myset", "a")
	if err != nil {
		t.Fatal(err)
	}
	if !ok {
		t.Error("expected is_member=true")
	}
}

func TestSet_Remove(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "set.rem" {
			t.Errorf("expected set.rem, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"removed":1}}`
	})
	defer srv.Close()

	n, err := newTestClient(srv.URL).Set().Remove(context.Background(), "myset", []string{"a"})
	if err != nil {
		t.Fatal(err)
	}
	if n != 1 {
		t.Errorf("expected 1, got %d", n)
	}
}

func TestSet_Card(t *testing.T) {
	srv := mockServerFn(t, func(cmd string, _ json.RawMessage) string {
		if cmd != "set.card" {
			t.Errorf("expected set.card, got %s", cmd)
		}
		return `{"success":true,"request_id":"x","payload":{"cardinality":4}}`
	})
	defer srv.Close()

	n, err := newTestClient(srv.URL).Set().Card(context.Background(), "myset")
	if err != nil {
		t.Fatal(err)
	}
	if n != 4 {
		t.Errorf("expected 4, got %d", n)
	}
}

// ── Request ID ────────────────────────────────────────────────────────────────

func TestRequestID_IsUUID(t *testing.T) {
	var lastID string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var env struct {
			RequestID string `json:"request_id"`
		}
		_ = json.NewDecoder(r.Body).Decode(&env)
		lastID = env.RequestID
		_, _ = w.Write([]byte(`{"success":true,"request_id":"x","payload":null}`))
	}))
	defer srv.Close()

	_ = newTestClient(srv.URL).KV().Set(context.Background(), "k", "v", 0)
	// UUID v4 pattern: 8-4-4-4-12 hex chars
	if len(lastID) != 36 {
		t.Errorf("expected UUID length 36, got %d: %s", len(lastID), lastID)
	}
	if lastID[8] != '-' || lastID[13] != '-' || lastID[18] != '-' || lastID[23] != '-' {
		t.Errorf("unexpected UUID format: %s", lastID)
	}
}

func TestRequestID_Unique(t *testing.T) {
	var ids []string
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		var env struct {
			RequestID string `json:"request_id"`
		}
		_ = json.NewDecoder(r.Body).Decode(&env)
		ids = append(ids, env.RequestID)
		_, _ = w.Write([]byte(`{"success":true,"request_id":"x","payload":null}`))
	}))
	defer srv.Close()

	client := newTestClient(srv.URL)
	for i := 0; i < 5; i++ {
		_ = client.KV().Set(context.Background(), "k", "v", 0)
	}
	seen := make(map[string]bool)
	for _, id := range ids {
		if seen[id] {
			t.Errorf("duplicate request_id: %s", id)
		}
		seen[id] = true
	}
}
