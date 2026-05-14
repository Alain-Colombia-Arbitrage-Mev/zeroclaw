import { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { Send, Bot, User, AlertCircle, Copy, Check, X, Trash2, Minimize2, Maximize2, Building2 } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { WsMessage } from '@/types/api';
import { WebSocketClient, getOrCreateSessionId } from '@/lib/ws';
import { generateUUID } from '@/lib/uuid';
import { useDraft } from '@/hooks/useDraft';
import { useTenant } from '@/contexts/TenantContext';
import { t } from '@/lib/i18n';
import { abortSession, deleteSession, getSessionMessages } from '@/lib/api';
import { SESSION_ID_STORAGE_KEY } from '@/lib/ws';
import ToolCallCard from '@/components/ToolCallCard';
import type { ToolCallInfo } from '@/components/ToolCallCard';
import {
  loadChatHistory,
  mapServerMessagesToPersisted,
  persistedToUiMessages,
  saveChatHistory,
  uiMessagesToPersisted,
} from '@/lib/chatHistoryStorage';

interface ChatMessage {
  id: string;
  role: 'user' | 'agent';
  content: string;
  thinking?: string;
  markdown?: boolean;
  toolCall?: ToolCallInfo;
  timestamp: Date;
}

const DRAFT_KEY = 'agent-chat';

export default function AgentChat() {
  // Per-tenant chat: each company gets its own session id, its own
  // chat thread, its own backend session-state row. Switching tenant
  // pulls up that tenant's conversation instead of leaking across
  // companies. When no tenant is active (fresh install) we fall
  // through to the legacy single-thread session id.
  const { active: activeTenant } = useTenant();
  const tenantId = activeTenant?.id ?? null;
  const sessionId = useMemo(() => getOrCreateSessionId(tenantId), [tenantId]);
  const sessionIdRef = useRef(sessionId);
  // Keep the ref in sync — it's used by the persistence effect and
  // the WS reconnect, both of which need the current value at call
  // time without re-running on every state change.
  useEffect(() => {
    sessionIdRef.current = sessionId;
  }, [sessionId]);

  const { draft, saveDraft, clearDraft } = useDraft(DRAFT_KEY);
  const [messages, setMessages] = useState<ChatMessage[]>(() => {
    const persisted = loadChatHistory(sessionIdRef.current);
    return persisted.length > 0 ? persistedToUiMessages(persisted) : [];
  });
  const [historyReady, setHistoryReady] = useState(false);
  const [input, setInput] = useState(draft);
  const [typing, setTyping] = useState(false);
  const [connected, setConnected] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const wsRef = useRef<WebSocketClient | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [compact, setCompact] = useState(() => {
    try { return localStorage.getItem('zeroclaw_chat_compact') === '1'; } catch { return false; }
  });
  const pendingContentRef = useRef('');
  const pendingThinkingRef = useRef('');
  // Snapshot of thinking captured at chunk_reset, so it survives the reset.
  const capturedThinkingRef = useRef('');
  const [streamingContent, setStreamingContent] = useState('');
  const [streamingThinking, setStreamingThinking] = useState('');

  // Persist draft to in-memory store so it survives route changes
  useEffect(() => {
    saveDraft(input);
  }, [input, saveDraft]);

  // Hydrate chat from server (preferred) or localStorage fallback.
  // Re-runs when the active tenant (and therefore session_id) changes,
  // so switching tenant in the orchestrator pulls up that tenant's
  // own thread instead of carrying the previous one over.
  useEffect(() => {
    const sid = sessionId;
    let cancelled = false;
    setHistoryReady(false);

    // Show whatever's already in localStorage for this session
    // immediately so the chat doesn't flash empty before the server
    // round-trip resolves.
    const local = loadChatHistory(sid);
    setMessages(local.length ? persistedToUiMessages(local) : []);

    (async () => {
      try {
        const res = await getSessionMessages(sid);
        if (cancelled) return;
        if (res.session_persistence && res.messages.length > 0) {
          setMessages(persistedToUiMessages(mapServerMessagesToPersisted(res.messages)));
        }
      } catch {
        // server unreachable — keep the localStorage snapshot we
        // already painted above
      } finally {
        if (!cancelled) setHistoryReady(true);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [sessionId]);

  // Mirror transcript to localStorage (bounded); server remains source of truth when persistence is on
  useEffect(() => {
    if (!historyReady) return;
    saveChatHistory(sessionIdRef.current, uiMessagesToPersisted(messages));
  }, [messages, historyReady]);

  useEffect(() => {
    const ws = new WebSocketClient();

    ws.onOpen = () => {
      setConnected(true);
      setError(null);
    };

    ws.onClose = (ev: CloseEvent) => {
      setConnected(false);
      if (ev.code !== 1000 && ev.code !== 1001) {
        setError(`Connection closed unexpectedly (code: ${ev.code}). Please check your configuration.`);
      }
    };

    ws.onError = () => {
      setError(t('agent.connection_error'));
    };

    ws.onMessage = (msg: WsMessage) => {
      switch (msg.type) {
        case 'session_start':
        case 'connected':
          break;

        case 'thinking':
          setTyping(true);
          pendingThinkingRef.current += msg.content ?? '';
          setStreamingThinking(pendingThinkingRef.current);
          break;

        case 'chunk':
          setTyping(true);
          pendingContentRef.current += msg.content ?? '';
          setStreamingContent(pendingContentRef.current);
          break;

        case 'chunk_reset':
          // Server signals that the authoritative done message follows.
          // Snapshot thinking before clearing display state.
          capturedThinkingRef.current = pendingThinkingRef.current;
          pendingContentRef.current = '';
          pendingThinkingRef.current = '';
          setStreamingContent('');
          setStreamingThinking('');
          break;

        case 'message':
        case 'done': {
          const content = msg.full_response ?? msg.content ?? pendingContentRef.current;
          const thinking = capturedThinkingRef.current || pendingThinkingRef.current || undefined;
          if (content) {
            setMessages((prev) => [
              ...prev,
              {
                id: generateUUID(),
                role: 'agent',
                content,
                thinking,
                markdown: true,
                timestamp: new Date(),
              },
            ]);
          }
          pendingContentRef.current = '';
          pendingThinkingRef.current = '';
          capturedThinkingRef.current = '';
          setStreamingContent('');
          setStreamingThinking('');
          setTyping(false);
          break;
        }

        case 'tool_call': {
          const toolName = msg.name ?? 'unknown';
          const toolArgs = msg.args;
          setMessages((prev) => {
            // Dedup: backend streaming may re-send tool_call events before execution.
            // Skip if an unresolved card with the same name+args already exists.
            const argsKey = JSON.stringify(toolArgs ?? {});
            const isDuplicate = prev.some(
              (m) => m.toolCall
                && m.toolCall.output === undefined
                && m.toolCall.name === toolName
                && JSON.stringify(m.toolCall.args ?? {}) === argsKey,
            );
            if (isDuplicate) return prev;

            return [
              ...prev,
              {
                id: generateUUID(),
                role: 'agent' as const,
                content: `${t('agent.tool_call_prefix')} ${toolName}(${argsKey})`,
                toolCall: { name: toolName, args: toolArgs },
                timestamp: new Date(),
              },
            ];
          });
          break;
        }

        case 'tool_result': {
          setMessages((prev) => {
            // Forward scan: find the FIRST unresolved toolCall (order-guaranteed by backend)
            const idx = prev.findIndex((m) => m.toolCall && m.toolCall.output === undefined);
            if (idx !== -1) {
              const updated = [...prev];
              const existing = prev[idx]!;
              updated[idx] = {
                ...existing,
                toolCall: { ...existing.toolCall!, output: msg.output ?? '' },
              };
              return updated;
            }
            // Fallback: no unresolved call found — append standalone card
            return [
              ...prev,
              {
                id: generateUUID(),
                role: 'agent' as const,
                content: `${t('agent.tool_result_prefix')} ${msg.output ?? ''}`,
                toolCall: { name: msg.name ?? 'unknown', output: msg.output ?? '' },
                timestamp: new Date(),
              },
            ];
          });
          break;
        }

        case 'cron_result': {
          const cronOutput = msg.output ?? '';
          if (cronOutput) {
            setMessages((prev) => [
              ...prev,
              {
                id: generateUUID(),
                role: 'agent' as const,
                content: cronOutput,
                markdown: true,
                timestamp: new Date(msg.timestamp ?? Date.now()),
              },
            ]);
          }
          break;
        }

        case 'error':
          setMessages((prev) => [
            ...prev,
            {
              id: generateUUID(),
              role: 'agent',
              content: `${t('agent.error_prefix')} ${msg.message ?? t('agent.unknown_error')}`,
              timestamp: new Date(),
            },
          ]);
          if (msg.code === 'AGENT_INIT_FAILED' || msg.code === 'AUTH_ERROR' || msg.code === 'PROVIDER_ERROR') {
            setError(`Configuration error: ${msg.message}. Please check your provider settings (API key, model, etc.).`);
          } else if (msg.code === 'INVALID_JSON' || msg.code === 'UNKNOWN_MESSAGE_TYPE' || msg.code === 'EMPTY_CONTENT') {
            setError(`Message error: ${msg.message}`);
          }
          setTyping(false);
          pendingContentRef.current = '';
          pendingThinkingRef.current = '';
          setStreamingContent('');
          setStreamingThinking('');
          break;
      }
    };

    ws.connect();
    wsRef.current = ws;

    return () => {
      ws.disconnect();
    };
    // Re-key WS by sessionId so switching tenant tears down the
    // current connection and opens a new one with the per-tenant
    // session_id query param. Backend then pins the session row +
    // tenant scope to that row.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sessionId]);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' });
  }, [messages, typing, streamingContent]);

  const handleSend = () => {
    const trimmed = input.trim();
    if (!trimmed || !wsRef.current?.connected) return;

    setMessages((prev) => [
      ...prev,
      {
        id: generateUUID(),
        role: 'user',
        content: trimmed,
        timestamp: new Date(),
      },
    ]);

    try {
      wsRef.current.sendMessage(trimmed);
      setTyping(true);
      pendingContentRef.current = '';
      pendingThinkingRef.current = '';
    } catch {
      setError(t('agent.send_error'));
    }

    setInput('');
    clearDraft();
    if (inputRef.current) {
      inputRef.current.style.height = 'auto';
      inputRef.current.focus();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleTextareaChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setInput(e.target.value);
    e.target.style.height = 'auto';
    e.target.style.height = `${Math.min(e.target.scrollHeight, 200)}px`;
  };

  const handleCopy = useCallback((msgId: string, content: string) => {
    const onSuccess = () => {
      setCopiedId(msgId);
      setTimeout(() => setCopiedId((prev) => (prev === msgId ? null : prev)), 2000);
    };

    if (navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(content).then(onSuccess).catch(() => {
        // Fallback for insecure contexts (HTTP)
        fallbackCopy(content) && onSuccess();
      });
    } else {
      fallbackCopy(content) && onSuccess();
    }
  }, []);

  const handleDeleteMessage = useCallback((msgId: string) => {
    setMessages((prev) => prev.filter((m) => m.id !== msgId));
  }, []);

  const handleClearAll = useCallback(async () => {
    const sid = sessionIdRef.current;
    setMessages([]);
    setStreamingContent('');
    setStreamingThinking('');
    pendingContentRef.current = '';
    pendingThinkingRef.current = '';
    capturedThinkingRef.current = '';

    // Clear server-side session state (messages + state row) so a
    // refresh doesn't re-hydrate the deleted thread from SQLite.
    try {
      await deleteSession(sid);
    } catch {
      // Server unreachable — local clear still happened
    }

    // Burn the per-tenant session id so the next message starts a
    // fresh thread on a new id rather than reusing the deleted one.
    const key = tenantId
      ? `${SESSION_ID_STORAGE_KEY}__t_${tenantId}`
      : SESSION_ID_STORAGE_KEY;
    try {
      localStorage.removeItem(key);
      localStorage.removeItem(`zeroclaw_chat_history_v1:${sid}`);
    } catch {
      /* ignore */
    }

    // Force a reconnect with a fresh session id by toggling tenant
    // dependency — easiest is a hard reload of this page.
    window.location.reload();
  }, [tenantId]);

  const handleAbort = useCallback(async () => {
    const sid = sessionIdRef.current;
    try {
      await abortSession(sid);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Abort failed');
    }
  }, []);

  const toggleCompact = useCallback(() => {
    setCompact((prev) => {
      const next = !prev;
      try { localStorage.setItem('zeroclaw_chat_compact', next ? '1' : '0'); } catch { /* noop */ }
      return next;
    });
  }, []);

  /**
   * Fallback copy using a temporary textarea for HTTP contexts
   * where navigator.clipboard is unavailable.
   */
  function fallbackCopy(text: string): boolean {
    const textarea = document.createElement('textarea');
    textarea.value = text;
    textarea.style.position = 'fixed';
    textarea.style.opacity = '0';
    document.body.appendChild(textarea);
    textarea.select();
    try {
      document.execCommand('copy');
      return true;
    } catch {
      return false;
    } finally {
      document.body.removeChild(textarea);
    }
  }

  return (
    <div className="flex flex-col h-[calc(100vh-3.5rem)]">
      {/* Tenant identity strip — shows which company this chat
          belongs to. Each tenant has its own thread + session id;
          switching tenant in /orchestrator pulls up the other one's
          thread instead of leaking conversation across companies. */}
      <div
        className="px-4 py-2 border-b flex items-center gap-3 text-xs"
        style={{
          background: activeTenant
            ? 'rgba(125, 211, 252, 0.04)'
            : 'rgba(125, 211, 252, 0.02)',
          borderColor: 'var(--pc-border)',
          fontFamily: 'ui-monospace, monospace',
        }}
      >
        <Building2
          className="h-3.5 w-3.5 shrink-0"
          style={{ color: activeTenant ? '#7DD3FC' : '#5BA8D9' }}
        />
        {activeTenant ? (
          <>
            <span style={{ color: '#5BA8D9', letterSpacing: '0.25em' }}>
              MISSION
            </span>
            <span style={{ color: '#E0F2FE', letterSpacing: '0.08em', fontWeight: 600 }}>
              {activeTenant.name.toUpperCase()}
            </span>
            <span style={{ color: 'rgba(125, 211, 252, 0.3)' }}>·</span>
            <span style={{ color: '#94A3B8', letterSpacing: '0.18em' }}>
              {activeTenant.category.toUpperCase()}
            </span>
            {activeTenant.activities && activeTenant.activities.length > 0 && (
              <>
                <span style={{ color: 'rgba(125, 211, 252, 0.3)' }}>·</span>
                <span style={{ color: '#94A3B8' }}>
                  +{activeTenant.activities.length} ACT
                </span>
              </>
            )}
            <span className="ml-auto" style={{ color: '#5BA8D9', letterSpacing: '0.15em' }}>
              SESSION {sessionIdRef.current.slice(0, 8)}
            </span>
          </>
        ) : (
          <>
            <span style={{ color: '#94A3B8', letterSpacing: '0.18em' }}>
              NO MISSION SELECTED · using shared thread. Pick a mission from
              /orchestrator to scope this chat.
            </span>
          </>
        )}
      </div>

      {/* Connection status bar */}
      {error && (
        <div className="px-4 py-2 border-b flex items-center gap-2 text-sm animate-fade-in" style={{ background: 'rgba(239, 68, 68, 0.08)', borderColor: 'rgba(239, 68, 68, 0.2)', color: '#f87171', }}>
          <AlertCircle className="h-4 w-4 shrink-0" />
          {error}
        </div>
      )}

      {/* Chat toolbar */}
      {(messages.length > 0 || typing) && (
        <div
          className="flex items-center justify-end gap-2 px-4 py-2 border-b"
          style={{ background: 'var(--pc-bg-surface)', borderColor: 'var(--pc-border)' }}
        >
          {typing && (
            <button
              type="button"
              onClick={handleAbort}
              className="flex items-center gap-1.5 text-xs px-3 py-1.5 rounded-md"
              style={{
                background: 'rgba(252, 211, 77, 0.12)',
                border: '1px solid #FCD34D',
                color: '#FCD34D',
                fontFamily: 'ui-monospace, monospace',
                letterSpacing: '0.15em',
                fontWeight: 600,
              }}
              title="Abort the in-flight agent loop"
              aria-label="Abort run"
            >
              <X className="h-3 w-3" /> ABORT RUN
            </button>
          )}
          <button
            type="button"
            onClick={toggleCompact}
            className="btn-secondary flex items-center gap-1.5 text-xs"
            style={{ padding: '0.3rem 0.75rem', borderRadius: '0.5rem' }}
            aria-label={t('agent.compact_mode')}
          >
            {compact ? <Maximize2 className="h-3 w-3" /> : <Minimize2 className="h-3 w-3" />}
            {t('agent.compact_mode')}
          </button>
          <button
            type="button"
            onClick={handleClearAll}
            className="btn-danger flex items-center gap-1.5 text-xs"
            style={{ padding: '0.3rem 0.75rem', borderRadius: '0.5rem' }}
            title="Delete this thread (server + local) and start fresh"
            aria-label={t('agent.clear_all')}
          >
            <Trash2 className="h-3 w-3" />
            {t('agent.clear_all')}
          </button>
        </div>
      )}

      {/* Messages area */}
      <div className={`flex-1 overflow-y-auto p-4 ${compact ? 'space-y-1.5' : 'space-y-4'}`}>
        {messages.length === 0 && (
          <div className="flex flex-col items-center justify-center h-full text-center animate-fade-in" style={{ color: 'var(--pc-text-muted)' }}>
            <div className="h-16 w-16 rounded-3xl flex items-center justify-center mb-4 animate-float" style={{ background: 'var(--pc-accent-glow)' }}>
              <Bot className="h-8 w-8" style={{ color: 'var(--pc-accent)' }} />
            </div>
            <p className="text-lg font-semibold mb-1" style={{ color: 'var(--pc-text-primary)' }}>Octopus Labs Agent</p>
            <p className="text-sm" style={{ color: 'var(--pc-text-muted)' }}>{t('agent.start_conversation')}</p>
          </div>
        )}

        {messages.map((msg, idx) => (
          <div
            key={msg.id}
            className={`group flex items-start ${compact ? 'gap-2' : 'gap-3'} ${
              msg.role === 'user' ? 'flex-row-reverse animate-slide-in-right' : 'animate-slide-in-left'
            }`}
            style={{ animationDelay: `${Math.min(idx * 30, 200)}ms` }}
          >
            {!compact && (
              <div
                className="flex-shrink-0 w-9 h-9 rounded-2xl flex items-center justify-center border"
                style={{
                  background: msg.role === 'user' ? 'var(--pc-accent)' : 'var(--pc-bg-elevated)',
                  borderColor: msg.role === 'user' ? 'var(--pc-accent)' : 'var(--pc-border)',
                }}
              >
                {msg.role === 'user' ? (
                  <User className="h-4 w-4 text-white" />
                ) : (
                  <Bot className="h-4 w-4" style={{ color: 'var(--pc-accent)' }} />
                )}
              </div>
            )}
            <div className="relative max-w-[75%]">
              <div
                className={compact ? 'rounded-xl px-3 py-1.5 border' : 'rounded-2xl px-4 py-3 border'}
                style={
                  msg.role === 'user'
                    ? { background: 'var(--pc-accent-glow)', borderColor: 'var(--pc-accent-dim)', color: 'var(--pc-text-primary)', }
                    : { background: 'var(--pc-bg-elevated)', borderColor: 'var(--pc-border)', color: 'var(--pc-text-primary)', }
                }
              >
                {msg.thinking && (
                  <details className="mb-2">
                    <summary className="text-xs cursor-pointer select-none" style={{ color: 'var(--pc-text-muted)' }}>Thinking</summary>
                    <pre className="text-xs mt-1 whitespace-pre-wrap break-words leading-relaxed overflow-auto max-h-60 p-2 rounded-lg" style={{ color: 'var(--pc-text-muted)', background: 'var(--pc-bg-surface)' }}>{msg.thinking}</pre>
                  </details>
                )}
                {msg.toolCall ? (
                  <ToolCallCard toolCall={msg.toolCall} />
                ) : msg.markdown ? (
                  <div className={`${compact ? 'text-xs' : 'text-sm'} break-words leading-relaxed chat-markdown`}><ReactMarkdown remarkPlugins={[remarkGfm]}>{msg.content}</ReactMarkdown></div>
                ) : (
                  <p className={`${compact ? 'text-xs' : 'text-sm'} whitespace-pre-wrap break-words leading-relaxed`}>{msg.content}</p>
                )}
                {!compact && (
                  <p
                    className="text-[10px] mt-1.5" style={{ color: msg.role === 'user' ? 'var(--pc-accent-light)' : 'var(--pc-text-faint)' }}>
                    {msg.timestamp.toLocaleTimeString()}
                  </p>
                )}
              </div>
              {/* Hover action buttons — below the bubble, right-aligned */}
              <div className="flex items-center justify-end gap-1 mt-1 opacity-0 group-hover:opacity-100 transition-opacity">
                <button
                  onClick={() => handleCopy(msg.id, msg.content)}
                  aria-label={t('agent.copy_message')}
                  className="p-1 rounded-lg"
                  style={{ color: 'var(--pc-text-muted)' }}
                  onMouseEnter={(e) => { e.currentTarget.style.color = 'var(--pc-text-primary)'; }}
                  onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--pc-text-muted)'; }}
                >
                  {copiedId === msg.id ? (
                    <Check className="h-3.5 w-3.5" style={{ color: '#34d399' }} />
                  ) : (
                    <Copy className="h-3.5 w-3.5" />
                  )}
                </button>
                <button
                  onClick={() => handleDeleteMessage(msg.id)}
                  aria-label={t('agent.delete_message')}
                  className="p-1 rounded-lg"
                  style={{ color: 'var(--pc-text-muted)' }}
                  onMouseEnter={(e) => { e.currentTarget.style.color = '#f87171'; }}
                  onMouseLeave={(e) => { e.currentTarget.style.color = 'var(--pc-text-muted)'; }}
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
          </div>
        ))}

        {typing && (
          <div className="flex items-start gap-3 animate-fade-in">
            <div className="flex-shrink-0 w-9 h-9 rounded-2xl flex items-center justify-center border" style={{ background: 'var(--pc-bg-elevated)', borderColor: 'var(--pc-border)' }}>
              <Bot className="h-4 w-4" style={{ color: 'var(--pc-accent)' }} />
            </div>
            {streamingContent || streamingThinking ? (
              <div className="rounded-2xl px-4 py-3 border max-w-[75%]" style={{ background: 'var(--pc-bg-elevated)', borderColor: 'var(--pc-border)', color: 'var(--pc-text-primary)' }}>
                {streamingThinking && (
                  <details className="mb-2" open={!streamingContent}>
                    <summary className="text-xs cursor-pointer select-none" style={{ color: 'var(--pc-text-muted)' }}>Thinking{!streamingContent && '...'}</summary>
                    <pre className="text-xs mt-1 whitespace-pre-wrap break-words leading-relaxed overflow-auto max-h-60 p-2 rounded-lg" style={{ color: 'var(--pc-text-muted)', background: 'var(--pc-bg-surface)' }}>{streamingThinking}</pre>
                  </details>
                )}
                {streamingContent && <p className="text-sm whitespace-pre-wrap break-words leading-relaxed">{streamingContent}</p>}
              </div>
            ) : (
              <div className="rounded-2xl px-4 py-3 border flex items-center gap-1.5" style={{ background: 'var(--pc-bg-elevated)', borderColor: 'var(--pc-border)' }}>
                <span className="bounce-dot w-1.5 h-1.5 rounded-full" style={{ background: 'var(--pc-accent)' }} />
                <span className="bounce-dot w-1.5 h-1.5 rounded-full" style={{ background: 'var(--pc-accent)' }} />
                <span className="bounce-dot w-1.5 h-1.5 rounded-full" style={{ background: 'var(--pc-accent)' }} />
              </div>
            )}
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      {/* Input area */}
      <div className="border-t p-4" style={{ borderColor: 'var(--pc-border)', background: 'var(--pc-bg-surface)' }}>
        <div className="flex items-center gap-3 max-w-4xl mx-auto">
          <textarea
            ref={inputRef}
            rows={1}
            value={input}
            onChange={handleTextareaChange}
            onKeyDown={handleKeyDown}
            placeholder={connected ? t('agent.type_message') : t('agent.connecting')}
            disabled={!connected}
            className="input-electric flex-1 px-4 text-sm resize-none disabled:opacity-40"
            style={{ minHeight: '44px', maxHeight: '200px', paddingTop: '10px', paddingBottom: '10px' }}
          />
          <button
            type='button'
            onClick={handleSend}
            disabled={!connected || !input.trim()}
            className="btn-electric flex-shrink-0 rounded-2xl flex items-center justify-center"
            style={{ color: 'white', width: '40px', height: '40px' }}
          >
            <Send className="h-5 w-5" />
          </button>
        </div>
        <div className="flex items-center justify-center mt-2 gap-2">
          <span
            className="status-dot"
            style={connected
              ? { background: 'var(--color-status-success)', boxShadow: '0 0 6px var(--color-status-success)' }
              : { background: 'var(--color-status-error)', boxShadow: '0 0 6px var(--color-status-error)' }
            }
          />
          <span className="text-[10px]" style={{ color: 'var(--pc-text-faint)' }}>
            {connected ? t('agent.connected_status') : t('agent.disconnected_status')}
          </span>
        </div>
      </div>
    </div>
  );
}
