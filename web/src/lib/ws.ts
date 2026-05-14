import type { WsMessage } from '../types/api';
import { getToken } from './auth';
import { apiOrigin, basePath } from './basePath';
import { isTauri } from './tauri';
import { generateUUID } from './uuid';

export type WsMessageHandler = (msg: WsMessage) => void;
export type WsOpenHandler = () => void;
export type WsCloseHandler = (ev: CloseEvent) => void;
export type WsErrorHandler = (ev: Event) => void;

export interface WebSocketClientOptions {
  /** Base URL override. Defaults to current host with ws(s) protocol. */
  baseUrl?: string;
  /** Delay in ms before attempting reconnect. Doubles on each failure up to maxReconnectDelay. */
  reconnectDelay?: number;
  /** Maximum reconnect delay in ms. */
  maxReconnectDelay?: number;
  /** Set to false to disable auto-reconnect. Default true. */
  autoReconnect?: boolean;
}

const DEFAULT_RECONNECT_DELAY = 1000;
const MAX_RECONNECT_DELAY = 30000;

export const SESSION_ID_STORAGE_KEY = 'zeroclaw_session_id';

/**
 * Return a stable session ID for the given tenant, persisted in
 * localStorage. When `tenantId` is omitted, returns the legacy
 * single-tenant session id (for backwards compatibility).
 *
 * Per-tenant session ids let each company keep its own chat thread,
 * its own backend session-state row, and its own scoped memory.
 * Switching tenant in the dashboard pulls up that tenant's chat
 * instead of leaking conversation across companies.
 */
export function getOrCreateSessionId(tenantId?: string | null): string {
  const key = tenantId && tenantId.length > 0
    ? `${SESSION_ID_STORAGE_KEY}__t_${tenantId}`
    : SESSION_ID_STORAGE_KEY;
  let id = localStorage.getItem(key);
  if (!id) {
    id = generateUUID();
    localStorage.setItem(key, id);
  }
  return id;
}

export class WebSocketClient {
  private ws: WebSocket | null = null;
  private currentDelay: number;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private intentionallyClosed = false;

  public onMessage: WsMessageHandler | null = null;
  public onOpen: WsOpenHandler | null = null;
  public onClose: WsCloseHandler | null = null;
  public onError: WsErrorHandler | null = null;

  private readonly baseUrl: string;
  private readonly reconnectDelay: number;
  private readonly maxReconnectDelay: number;
  private readonly autoReconnect: boolean;

  constructor(options: WebSocketClientOptions = {}) {
    let defaultBase: string;
    if (isTauri() && apiOrigin) {
      // In Tauri, derive ws URL from the gateway origin.
      defaultBase = apiOrigin.replace(/^http/, 'ws');
    } else {
      const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
      defaultBase = `${protocol}//${window.location.host}`;
    }
    this.baseUrl = options.baseUrl ?? defaultBase;
    this.reconnectDelay = options.reconnectDelay ?? DEFAULT_RECONNECT_DELAY;
    this.maxReconnectDelay = options.maxReconnectDelay ?? MAX_RECONNECT_DELAY;
    this.autoReconnect = options.autoReconnect ?? true;
    this.currentDelay = this.reconnectDelay;
  }

  /** Open the WebSocket connection. */
  connect(): void {
    this.intentionallyClosed = false;
    this.clearReconnectTimer();

    const token = getToken();
    // Read active tenant first so we can derive a per-tenant session
    // id (each tenant keeps its own thread + backend session-state row).
    let activeTenant: string | null = null;
    try {
      activeTenant = localStorage.getItem('octopus_active_tenant');
    } catch {
      // localStorage may be blocked — fall back to no-tenant scope
    }
    const sessionId = getOrCreateSessionId(activeTenant);
    const params = new URLSearchParams();
    if (token) params.set('token', token);
    params.set('session_id', sessionId);
    if (activeTenant) params.set('tenant', activeTenant);
    const url = `${this.baseUrl}${basePath}/ws/chat?${params.toString()}`;

    const protocols: string[] = ['zeroclaw.v1'];
    if (token) protocols.push(`bearer.${token}`);
    this.ws = new WebSocket(url, protocols);

    this.ws.onopen = () => {
      this.currentDelay = this.reconnectDelay;
      this.onOpen?.();
    };

    this.ws.onmessage = (ev: MessageEvent) => {
      try {
        const msg = JSON.parse(ev.data) as WsMessage;
        this.onMessage?.(msg);
      } catch {
        // Ignore non-JSON frames
      }
    };

    this.ws.onclose = (ev: CloseEvent) => {
      this.onClose?.(ev);
      this.scheduleReconnect();
    };

    this.ws.onerror = (ev: Event) => {
      this.onError?.(ev);
    };
  }

  /** Send a chat message to the agent. */
  sendMessage(content: string): void {
    if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
      throw new Error('WebSocket is not connected');
    }
    this.ws.send(JSON.stringify({ type: 'message', content }));
  }

  /** Close the connection without auto-reconnecting. */
  disconnect(): void {
    this.intentionallyClosed = true;
    this.clearReconnectTimer();
    if (this.ws) {
      this.ws.close();
      this.ws = null;
    }
  }

  /** Returns true if the socket is open. */
  get connected(): boolean {
    return this.ws?.readyState === WebSocket.OPEN;
  }

  // ---------------------------------------------------------------------------
  // Reconnection logic
  // ---------------------------------------------------------------------------

  private scheduleReconnect(): void {
    if (this.intentionallyClosed || !this.autoReconnect) return;

    this.reconnectTimer = setTimeout(() => {
      this.currentDelay = Math.min(this.currentDelay * 2, this.maxReconnectDelay);
      this.connect();
    }, this.currentDelay);
  }

  private clearReconnectTimer(): void {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }
}
