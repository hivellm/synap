import { SynapClient } from './client';
import { KVStore } from './kv';
import { HashManager } from './hash';
import { ListManager } from './list';
import { SetManager } from './set';
import { SortedSetManager } from './sorted-set';
import { QueueManager } from './queue';
import { StreamManager } from './stream';
import { PubSubManager } from './pubsub';
import { ScriptManager } from './scripting';
import { HyperLogLogManager } from './hyperloglog';

export interface TransactionOptions {
  /** Optional client identifier to group commands within the same transaction */
  clientId?: string;
}

export interface TransactionWatchOptions extends TransactionOptions {
  /** Keys to watch for changes */
  keys: string[];
}

export interface TransactionResponse {
  /** Informational message returned by the server */
  message: string;
  /** Indicates whether the operation succeeded */
  success: boolean;
}

export interface TransactionExecSuccess<T = unknown> {
  success: true;
  results: T[];
}

export interface TransactionExecAborted {
  success: false;
  aborted: true;
  message?: string;
}

export type TransactionExecResult<T = unknown> =
  | TransactionExecSuccess<T>
  | TransactionExecAborted;

export interface TransactionScope {
  clientId: string;
  kv: KVStore;
  hash: HashManager;
  list: ListManager;
  set: SetManager;
  sortedSet: SortedSetManager;
  queue: QueueManager;
  stream: StreamManager;
  pubsub: PubSubManager;
  script: ScriptManager;
  hyperloglog: HyperLogLogManager;
}

/**
 * Transaction manager for Redis-compatible MULTI/EXEC workflow.
 *
 * ```typescript
 * const client = new Synap({ url: 'http://localhost:15500' });
 * const txClientId = crypto.randomUUID();
 *
 * await client.transaction.multi({ clientId: txClientId });
 * await client.kv.set('counter', 1, { clientId: txClientId });
 * const result = await client.transaction.exec({ clientId: txClientId });
 * ```
 */
export class TransactionManager {
  constructor(private readonly client: SynapClient) {}

  private buildPayload(
    options?: TransactionOptions,
    extra: Record<string, unknown> = {}
  ): Record<string, unknown> {
    const payload: Record<string, unknown> = { ...extra };

    if (options?.clientId) {
      payload.client_id = options.clientId;
    }

    return payload;
  }

  /**
   * Start a new transaction (MULTI)
   */
  async multi(options?: TransactionOptions): Promise<TransactionResponse> {
    const response = await this.client.sendCommand<TransactionResponse>(
      'transaction.multi',
      this.buildPayload(options)
    );

    return {
      success: response.success ?? true,
      message: response.message ?? 'Transaction started',
    };
  }

  /**
   * Discard the current transaction (DISCARD)
   */
  async discard(options?: TransactionOptions): Promise<TransactionResponse> {
    const response = await this.client.sendCommand<TransactionResponse>(
      'transaction.discard',
      this.buildPayload(options)
    );

    return {
      success: response.success ?? true,
      message: response.message ?? 'Transaction discarded',
    };
  }

  /**
   * Watch keys for optimistic locking (WATCH)
   */
  async watch(options: TransactionWatchOptions): Promise<TransactionResponse> {
    if (!options.keys || options.keys.length === 0) {
      throw new Error('Transaction watch requires at least one key');
    }

    const response = await this.client.sendCommand<TransactionResponse>(
      'transaction.watch',
      this.buildPayload(options, { keys: options.keys })
    );

    return {
      success: response.success ?? true,
      message: response.message ?? 'Keys watched',
    };
  }

  /**
   * Remove all watched keys (UNWATCH)
   */
  async unwatch(options?: TransactionOptions): Promise<TransactionResponse> {
    const response = await this.client.sendCommand<TransactionResponse>(
      'transaction.unwatch',
      this.buildPayload(options)
    );

    return {
      success: response.success ?? true,
      message: response.message ?? 'Keys unwatched',
    };
  }

  /**
   * Execute queued commands (EXEC)
   */
  async exec<T = unknown>(options?: TransactionOptions): Promise<TransactionExecResult<T>> {
    const response = await this.client.sendCommand<Record<string, unknown>>(
      'transaction.exec',
      this.buildPayload(options)
    );

    if (Array.isArray(response?.results)) {
      return {
        success: true,
        results: response.results as T[],
      };
    }

    return {
      success: false,
      aborted: Boolean(response?.aborted ?? true),
      message: typeof response?.message === 'string' ? (response.message as string) : undefined,
    };
  }

  /**
   * Create a scoped view of the SDK where every command automatically carries the provided client id.
   */
  scope(clientId: string): TransactionScope {
    if (!clientId) {
      throw new Error('Transaction scope requires a clientId');
    }

    const baseClient = this.client;
    const scopedClient = {
      sendCommand: (command: string, payload: Record<string, unknown> = {}) => {
        const scopedPayload = { ...payload };
        if (!('client_id' in scopedPayload)) {
          scopedPayload.client_id = clientId;
        }
        return baseClient.sendCommand(command, scopedPayload);
      },
      ping: baseClient.ping.bind(baseClient),
      health: baseClient.health.bind(baseClient),
      close: baseClient.close.bind(baseClient),
    } as unknown as SynapClient;

    return {
      clientId,
      kv: new KVStore(scopedClient),
      hash: new HashManager(scopedClient),
      list: new ListManager(scopedClient),
      set: new SetManager(scopedClient),
      sortedSet: new SortedSetManager(scopedClient),
      queue: new QueueManager(scopedClient),
      stream: new StreamManager(scopedClient),
      pubsub: new PubSubManager(scopedClient),
      script: new ScriptManager(scopedClient),
      hyperloglog: new HyperLogLogManager(scopedClient),
    };
  }
}
