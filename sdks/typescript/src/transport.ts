/**
 * Synap TypeScript SDK - Transport facade (backward-compatible re-exports)
 *
 * This file is the original transport entry point. All implementations have
 * been moved to `./transports/` for better modularity. This module re-exports
 * everything so existing imports (`from './transport'`) continue to work.
 */

export type { TransportMode } from './types';

export {
  SynapRpcTransport,
  toWireValue,
  fromWireValue,
} from './transports/synap-rpc';

export type { WireValue } from './transports/synap-rpc';

export { Resp3Transport } from './transports/resp3';

export { mapCommand, mapResponse } from './transports/command-map';
export type { MappedCommand } from './transports/command-map';
