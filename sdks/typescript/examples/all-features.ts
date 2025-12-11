/**
 * Complete Examples for All Synap Features
 * 
 * This file demonstrates all available features of the Synap TypeScript SDK
 * connected to a Docker server running on localhost:15500
 */

import { Synap } from '../src/index';

// Create client connected to Docker server
const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runAllExamples() {
  console.log('🚀 Starting Synap SDK Examples\n');

  try {
    // Test connection
    console.log('📡 Testing connection...');
    const health = await synap.health();
    console.log('✅ Server health:', health);
    console.log('');

    // ============================================
    // 1. KEY-VALUE STORE
    // ============================================
    console.log('📦 === KEY-VALUE STORE ===');
    
    // SET/GET
    await synap.kv.set('user:1', { name: 'Alice', age: 30, email: 'alice@example.com' });
    const user = await synap.kv.get('user:1');
    console.log('✅ GET user:1:', user);

    // SET with TTL
    await synap.kv.set('session:abc123', { userId: 1, expiresAt: Date.now() }, { ttl: 3600 });
    console.log('✅ SET with TTL');

    // APPEND (via kv.append command)
    await synap.kv.set('log:app', 'Initial log');
    await synap.getClient().sendCommand('kv.append', { key: 'log:app', value: '\nNew log entry' });
    const log = await synap.kv.get('log:app');
    console.log('✅ APPEND result:', log);

    // STRLEN (via kv.strlen command)
    const strlenResult = await synap.getClient().sendCommand<{ length: number }>('kv.strlen', { key: 'log:app' });
    console.log('✅ STRLEN:', strlenResult.length);

    // GETRANGE (via kv.getrange command)
    const rangeResult = await synap.getClient().sendCommand<{ range: string }>('kv.getrange', { 
      key: 'log:app', 
      start: 0, 
      end: 10 
    });
    console.log('✅ GETRANGE:', rangeResult.range);

    // DELETE
    await synap.kv.del('session:abc123');
    console.log('✅ DELETE session:abc123');

    // STATS
    const kvStats = await synap.kv.stats();
    console.log('✅ KV Stats:', kvStats);
    console.log('');

    // ============================================
    // 2. HASH OPERATIONS
    // ============================================
    console.log('🗂️  === HASH OPERATIONS ===');
    
    // HSET
    await synap.hash.set('user:profile:1', 'name', 'Bob');
    await synap.hash.set('user:profile:1', 'age', '25');
    await synap.hash.set('user:profile:1', 'city', 'New York');
    console.log('✅ HSET multiple fields');

    // HGET
    const name = await synap.hash.get('user:profile:1', 'name');
    console.log('✅ HGET name:', name);

    // HGETALL
    const profile = await synap.hash.getAll('user:profile:1');
    console.log('✅ HGETALL:', profile);

    // HMSET
    await synap.hash.mset('user:profile:2', {
      name: 'Charlie',
      age: '35',
      city: 'London',
    });
    console.log('✅ HMSET');

    // HINCRBY (works with new field or existing numeric field)
    // Use a different field name to avoid conflicts from previous runs
    await synap.hash.incrBy('user:profile:1', 'visits', 5);  // Creates field and sets to 5
    const visits = await synap.hash.get('user:profile:1', 'visits');
    console.log('✅ HINCRBY visits (created new field):', visits);

    // HLEN
    const hashLen = await synap.hash.len('user:profile:1');
    console.log('✅ HLEN:', hashLen);

    // HDEL (server expects fields array, SDK sends single field - using sendCommand directly)
    await synap.getClient().sendCommand('hash.del', { 
      key: 'user:profile:1', 
      fields: ['city'] 
    });
    console.log('✅ HDEL city field');

    // STATS (via hash.stats command)
    const hashStats = await synap.getClient().sendCommand('hash.stats', {});
    console.log('✅ Hash Stats:', hashStats);
    console.log('');

    // ============================================
    // 3. LIST OPERATIONS
    // ============================================
    console.log('📋 === LIST OPERATIONS ===');
    
    // LPUSH
    await synap.list.lpush('tasks', 'task1', 'task2', 'task3');
    console.log('✅ LPUSH 3 tasks');

    // RPUSH
    await synap.list.rpush('tasks', 'task4', 'task5');
    console.log('✅ RPUSH 2 tasks');

    // LLEN (using list.llen command)
    const llenResult = await synap.getClient().sendCommand<{ length: number }>('list.llen', { key: 'tasks' });
    console.log('✅ LLEN:', llenResult.length);

    // LRANGE (using list.lrange command)
    const lrangeResult = await synap.getClient().sendCommand<{ values: string[] }>('list.lrange', { 
      key: 'tasks', 
      start: 0, 
      stop: -1 
    });
    console.log('✅ LRANGE:', lrangeResult.values);

    // LPOP
    const firstTask = await synap.list.lpop('tasks');
    console.log('✅ LPOP:', firstTask);

    // RPOP
    const lastTask = await synap.list.rpop('tasks');
    console.log('✅ RPOP:', lastTask);

    // LINDEX (using list.lindex command)
    const lindexResult = await synap.getClient().sendCommand<{ value: string | null }>('list.lindex', { 
      key: 'tasks', 
      index: 0 
    });
    console.log('✅ LINDEX 0:', lindexResult.value);

    // STATS (via list.stats command)
    const listStats = await synap.getClient().sendCommand('list.stats', {});
    console.log('✅ List Stats:', listStats);
    console.log('');

    // ============================================
    // 4. SET OPERATIONS
    // ============================================
    console.log('🔢 === SET OPERATIONS ===');
    
    // SADD
    await synap.set.add('tags', 'javascript', 'typescript', 'nodejs', 'redis');
    console.log('✅ SADD tags');

    // SMEMBERS
    const tags = await synap.set.members('tags');
    console.log('✅ SMEMBERS:', tags);

    // SCARD (using set.size command)
    const sizeResult = await synap.getClient().sendCommand<{ size: number }>('set.size', { key: 'tags' });
    console.log('✅ SCARD:', sizeResult.size);

    // SISMEMBER
    const isMember = await synap.set.isMember('tags', 'typescript');
    console.log('✅ SISMEMBER typescript:', isMember);

    // SPOP
    const popped = await synap.set.pop('tags', 1);
    console.log('✅ SPOP:', popped);

    // SREM
    await synap.set.rem('tags', 'javascript');
    console.log('✅ SREM javascript');

    // STATS (via set.stats command)
    const setStats = await synap.getClient().sendCommand('set.stats', {});
    console.log('✅ Set Stats:', setStats);
    console.log('');

    // ============================================
    // 5. SORTED SET OPERATIONS
    // ============================================
    console.log('⭐ === SORTED SET OPERATIONS ===');
    
    // ZADD
    await synap.sortedSet.add('leaderboard', 'player1', 100);
    await synap.sortedSet.add('leaderboard', 'player2', 200);
    await synap.sortedSet.add('leaderboard', 'player3', 150);
    console.log('✅ ZADD 3 players');

    // ZCARD
    const leaderboardSize = await synap.sortedSet.card('leaderboard');
    console.log('✅ ZCARD:', leaderboardSize);

    // ZRANGE
    const topPlayers = await synap.sortedSet.range('leaderboard', 0, -1, true);
    console.log('✅ ZRANGE:', topPlayers);

    // ZRANK
    const rank = await synap.sortedSet.rank('leaderboard', 'player2');
    console.log('✅ ZRANK player2:', rank);

    // ZSCORE
    const score = await synap.sortedSet.score('leaderboard', 'player2');
    console.log('✅ ZSCORE player2:', score);

    // ZINCRBY
    await synap.sortedSet.incrBy('leaderboard', 'player1', 50);
    const newScore = await synap.sortedSet.score('leaderboard', 'player1');
    console.log('✅ ZINCRBY player1 +50:', newScore);

    // STATS
    const sortedSetStats = await synap.sortedSet.stats();
    console.log('✅ Sorted Set Stats:', sortedSetStats);
    console.log('');

    // ============================================
    // 6. QUEUE OPERATIONS
    // ============================================
    console.log('📨 === QUEUE OPERATIONS ===');
    
    // CREATE QUEUE
    await synap.queue.createQueue('job-queue', {
      max_depth: 1000,
      ack_deadline_secs: 300,
    });
    console.log('✅ CREATE QUEUE');

    // PUBLISH (queue.publish expects string or Uint8Array)
    const msgId1 = await synap.queue.publish('job-queue', JSON.stringify({ type: 'email', to: 'user@example.com' }));
    const msgId2 = await synap.queue.publish('job-queue', JSON.stringify({ type: 'sms', to: '+1234567890' }));
    console.log('✅ PUBLISH messages:', msgId1, msgId2);

    // CONSUME
    const message = await synap.queue.consume('job-queue', 'worker-1');
    if (message) {
      console.log('✅ CONSUME message:', message.id);
      
      // ACK
      await synap.queue.ack('job-queue', message.id);
      console.log('✅ ACK message');
    }

    // STATS
    const queueStats = await synap.queue.stats('job-queue');
    console.log('✅ Queue Stats:', queueStats);

    // LIST QUEUES
    const queues = await synap.queue.listQueues();
    console.log('✅ LIST QUEUES:', queues);
    console.log('');

    // ============================================
    // 7. STREAM OPERATIONS
    // ============================================
    console.log('🌊 === STREAM OPERATIONS ===');
    
    // CREATE ROOM (rooms are auto-created on publish, but we can create explicitly)
    const roomName = `chat-room-${Date.now()}`;
    try {
      await synap.stream.createRoom(roomName);
      console.log('✅ CREATE STREAM ROOM');
    } catch (error: any) {
      // Room might already exist, that's OK
      if (!error.message?.includes('already exists')) {
        throw error;
      }
      console.log('✅ STREAM ROOM (already exists, using existing)');
    }

    // PUBLISH (using the room name)
    const offset1 = await synap.stream.publish(roomName, 'message.sent', {
      user: 'alice',
      text: 'Hello, world!',
      timestamp: Date.now(),
    });
    const offset2 = await synap.stream.publish(roomName, 'message.sent', {
      user: 'bob',
      text: 'Hi there!',
      timestamp: Date.now(),
    });
    console.log('✅ PUBLISH events at offsets:', offset1, offset2);

    // CONSUME
    const events = await synap.stream.consume(roomName, 'subscriber-1', 0);
    console.log('✅ CONSUME events:', events.length);

    // STATS
    const streamStats = await synap.stream.stats(roomName);
    console.log('✅ Stream Stats:', streamStats);

    // LIST ROOMS
    const rooms = await synap.stream.listRooms();
    console.log('✅ LIST ROOMS:', rooms);
    console.log('');

    // ============================================
    // 8. PUB/SUB OPERATIONS
    // ============================================
    console.log('📢 === PUB/SUB OPERATIONS ===');
    
    // PUBLISH
    await synap.pubsub.publish('user.created', { id: 1, name: 'Alice' });
    await synap.pubsub.publish('user.updated', { id: 1, name: 'Alice Updated' });
    await synap.pubsub.publish('order.placed', { orderId: 123, amount: 99.99 });
    console.log('✅ PUBLISH to topics');

    // STATS (pubsub.stats requires a topic)
    const pubsubStats = await synap.pubsub.stats('user.created');
    console.log('✅ PubSub Stats:', pubsubStats);

    // LIST TOPICS (using pubsub.topics command)
    const topicsResult = await synap.getClient().sendCommand<{ topics: string[] }>('pubsub.topics', {});
    console.log('✅ LIST TOPICS:', topicsResult.topics);
    console.log('');

    // ============================================
    // 9. TRANSACTION OPERATIONS
    // ============================================
    console.log('🔄 === TRANSACTION OPERATIONS ===');
    
    const txClientId = `tx-${Date.now()}`;
    
    // WATCH (creates transaction implicitly)
    await synap.transaction.watch({ keys: ['user:1', 'user:2'], clientId: txClientId });
    console.log('✅ WATCH keys (transaction created implicitly)');

    // Queue commands (need to pass clientId in options)
    // Note: SDK doesn't support clientId in kv.set options yet, so we'll use sendCommand directly
    await synap.getClient().sendCommand('kv.set', { 
      key: 'user:1', 
      value: { balance: 100 },
      client_id: txClientId 
    });
    await synap.getClient().sendCommand('kv.set', { 
      key: 'user:2', 
      value: { balance: 50 },
      client_id: txClientId 
    });
    console.log('✅ Queued commands in transaction');

    // EXEC
    const execResult = await synap.transaction.exec({ clientId: txClientId });
    if (!execResult.success) {
      console.log('❌ Transaction aborted (watched keys changed)');
    } else {
      console.log('✅ EXEC transaction:', execResult.results?.length || 0, 'commands executed');
    }

    // UNWATCH
    await synap.transaction.unwatch({ clientId: txClientId });
    console.log('✅ UNWATCH');
    console.log('');

    // ============================================
    // 10. KEY MANAGEMENT OPERATIONS
    // ============================================
    console.log('🔑 === KEY MANAGEMENT OPERATIONS ===');
    
    // EXISTS (via key.exists command)
    const existsResult = await synap.getClient().sendCommand<{ exists: boolean }>('key.exists', { key: 'user:1' });
    console.log('✅ EXISTS user:1:', existsResult.exists);

    // TYPE (via key.type command)
    const typeResult = await synap.getClient().sendCommand<{ type: string }>('key.type', { key: 'user:1' });
    console.log('✅ TYPE user:1:', typeResult.type);

    // RENAME (via key.rename command)
    await synap.kv.set('old-key', 'value');
    await synap.getClient().sendCommand('key.rename', { source: 'old-key', destination: 'new-key' });
    const renamedValue = await synap.kv.get('new-key');
    console.log('✅ RENAME:', renamedValue);

    // COPY (via key.copy command)
    await synap.kv.set('source-key', 'source-value');
    await synap.getClient().sendCommand('key.copy', { source: 'source-key', destination: 'dest-key' });
    const copiedValue = await synap.kv.get('dest-key');
    console.log('✅ COPY:', copiedValue);

    // RANDOMKEY (via key.randomkey command)
    const randomKeyResult = await synap.getClient().sendCommand<{ key: string | null }>('key.randomkey', {});
    console.log('✅ RANDOMKEY:', randomKeyResult.key);
    console.log('');

    // ============================================
    // 11. HYPERLOGLOG OPERATIONS
    // ============================================
    console.log('📊 === HYPERLOGLOG OPERATIONS ===');
    
    // PFADD
    await synap.hyperloglog.pfadd('unique-visitors', ['user1', 'user2', 'user3', 'user1']);
    console.log('✅ PFADD unique visitors');

    // PFCOUNT
    const count = await synap.hyperloglog.pfcount('unique-visitors');
    console.log('✅ PFCOUNT:', count);

    // PFMERGE
    await synap.hyperloglog.pfadd('visitors-day1', ['user1', 'user2']);
    await synap.hyperloglog.pfadd('visitors-day2', ['user2', 'user3']);
    await synap.hyperloglog.pfmerge('visitors-total', ['visitors-day1', 'visitors-day2']);
    const totalCount = await synap.hyperloglog.pfcount('visitors-total');
    console.log('✅ PFMERGE total:', totalCount);

    // STATS
    const hllStats = await synap.hyperloglog.stats();
    console.log('✅ HyperLogLog Stats:', hllStats);
    console.log('');

    // ============================================
    // 12. BITMAP OPERATIONS
    // ============================================
    console.log('🔲 === BITMAP OPERATIONS ===');
    
    // SETBIT
    await synap.bitmap.setbit('user:online', 0, 1);
    await synap.bitmap.setbit('user:online', 5, 1);
    await synap.bitmap.setbit('user:online', 10, 1);
    console.log('✅ SETBIT user:online');

    // GETBIT
    const bit0 = await synap.bitmap.getbit('user:online', 0);
    const bit1 = await synap.bitmap.getbit('user:online', 1);
    console.log('✅ GETBIT 0:', bit0, 'GETBIT 1:', bit1);

    // BITCOUNT
    const bitCount = await synap.bitmap.bitcount('user:online');
    console.log('✅ BITCOUNT:', bitCount);

    // BITPOS
    const firstSet = await synap.bitmap.bitpos('user:online', 1);
    console.log('✅ BITPOS first 1:', firstSet);

    // STATS
    const bitmapStats = await synap.bitmap.stats();
    console.log('✅ Bitmap Stats:', bitmapStats);
    console.log('');

    // ============================================
    // 13. GEOSPATIAL OPERATIONS
    // ============================================
    console.log('🌍 === GEOSPATIAL OPERATIONS ===');
    
    // GEOADD
    await synap.geospatial.geoadd('locations', [
      { member: 'restaurant1', lon: -122.4194, lat: 37.7749 },
      { member: 'restaurant2', lon: -122.4094, lat: 37.7849 },
      { member: 'restaurant3', lon: -122.4294, lat: 37.7649 },
    ]);
    console.log('✅ GEOADD locations');

    // GEODIST
    const distance = await synap.geospatial.geodist(
      'locations',
      'restaurant1',
      'restaurant2',
      'km'
    );
    console.log('✅ GEODIST:', distance, 'km');

    // GEORADIUS
    const nearby = await synap.geospatial.georadius(
      'locations',
      37.7749,  // centerLat
      -122.4194,  // centerLon
      5,
      'km'
    );
    console.log('✅ GEORADIUS:', nearby);

    // GEOPOS
    const position = await synap.geospatial.geopos('locations', ['restaurant1']);
    console.log('✅ GEOPOS:', position);

    // STATS
    const geoStats = await synap.geospatial.stats();
    console.log('✅ Geospatial Stats:', geoStats);
    console.log('');

    // ============================================
    // 14. SCRIPTING OPERATIONS
    // ============================================
    console.log('📜 === SCRIPTING OPERATIONS ===');
    
    // EVAL
    const scriptResult = await synap.script.eval(
      `return tonumber(ARGV[1]) + tonumber(ARGV[2])`,
      { keys: [], args: ['10', '20'] }
    );
    console.log('✅ EVAL script result:', scriptResult.result);

    // LOAD
    const script = `return KEYS[1] .. ":" .. ARGV[1]`;
    const sha = await synap.script.load(script);
    console.log('✅ LOAD script SHA:', sha);

    // EVALSHA
    const evalshaResult = await synap.script.evalsha(sha, { keys: ['key1'], args: ['value1'] });
    console.log('✅ EVALSHA result:', evalshaResult.result);

    // EXISTS
    const scriptExists = await synap.script.exists([sha]);
    console.log('✅ SCRIPT EXISTS:', scriptExists[0]);

    // FLUSH
    await synap.script.flush();
    console.log('✅ FLUSH scripts');
    console.log('');

    console.log('✅ All examples completed successfully!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

// Run examples
runAllExamples().catch(console.error);

