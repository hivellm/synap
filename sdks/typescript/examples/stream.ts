/**
 * Stream Operations Examples
 * 
 * Demonstrates event stream operations: CREATE ROOM, PUBLISH, CONSUME, STATS, LIST
 */

import { Synap } from '../src/index';

const synap = new Synap({
  url: 'http://localhost:15500',
  timeout: 30000,
});

async function runStreamExamples() {
  console.log('🌊 === STREAM OPERATIONS EXAMPLES ===\n');

  try {
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

    // PUBLISH
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

    console.log('\n✅ Stream operations examples completed!');
  } catch (error) {
    console.error('❌ Error:', error);
    throw error;
  } finally {
    synap.close();
  }
}

runStreamExamples().catch(console.error);

