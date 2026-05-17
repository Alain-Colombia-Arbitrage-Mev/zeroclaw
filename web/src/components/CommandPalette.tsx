// Command palette — Cmd+K / Ctrl+K quick navigation across the dashboard.
//
// Design: emulates the Linear / Cursor / VSCode pattern. Single keyboard
// shortcut opens a centered modal with a search input on top and a
// filtered list of commands underneath. Arrow keys move the cursor,
// Enter executes, Escape closes. Click also works for mouse users.
//
// Commands are static (navigation + a handful of global actions) — this
// is not a full command bus. The agent's tool surface is a separate
// concern handled by the AgentChat page.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import {
  Activity,
  Bot,
  Calendar,
  ClipboardList,
  CornerDownLeft,
  Database,
  DollarSign,
  Image as ImageIcon,
  Key,
  LayoutDashboard,
  LogOut,
  Plug,
  RefreshCw,
  Search,
  Settings,
  Stethoscope,
  Wrench,
  type LucideIcon,
} from 'lucide-react';

type CommandKind = 'navigate' | 'action';

interface CommandItem {
  id: string;
  label: string;
  hint?: string;
  icon: LucideIcon;
  kind: CommandKind;
  // For navigate: target route path. For action: undefined.
  to?: string;
  // For action: callback. For navigate: undefined.
  run?: () => void;
  // Free-text keywords appended to label for matching purposes.
  keywords?: string;
}

interface CommandPaletteProps {
  /** Called when the palette wants to log the user out. */
  onLogout: () => void;
}

/**
 * Global Command Palette. Mount once near the app root. It listens for
 * Cmd+K / Ctrl+K anywhere on the page, takes focus when opened, and
 * navigates / fires actions on selection.
 */
export function CommandPalette({ onLogout }: CommandPaletteProps) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [cursor, setCursor] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const listRef = useRef<HTMLDivElement | null>(null);
  const navigate = useNavigate();
  const location = useLocation();

  // ── Command catalog ────────────────────────────────────────────────
  // Defined inside the component because action callbacks (`run`) close
  // over `onLogout`, `navigate`, etc. Memoized so the list reference is
  // stable across renders unless those deps change.
  const commands = useMemo<CommandItem[]>(
    () => [
      { id: 'nav-dashboard', label: 'Dashboard', hint: 'Home', icon: LayoutDashboard, kind: 'navigate', to: '/' },
      { id: 'nav-agent', label: 'Agent chat', hint: 'Talk to the orchestrator', icon: Bot, kind: 'navigate', to: '/agent', keywords: 'jarvis chat assistant' },
      { id: 'nav-tools', label: 'Tools', hint: 'Tool surface', icon: Wrench, kind: 'navigate', to: '/tools' },
      { id: 'nav-cron', label: 'Cron jobs', hint: 'Scheduled tasks', icon: Calendar, kind: 'navigate', to: '/cron', keywords: 'schedule jobs' },
      { id: 'nav-integrations', label: 'Integrations', hint: 'Connected services', icon: Plug, kind: 'navigate', to: '/integrations' },
      { id: 'nav-memory', label: 'Memory', hint: 'Stored memories', icon: Database, kind: 'navigate', to: '/memory' },
      { id: 'nav-config', label: 'Config', hint: 'Settings & providers', icon: Settings, kind: 'navigate', to: '/config', keywords: 'settings preferences' },
      { id: 'nav-cost', label: 'Cost', hint: 'Token spend', icon: DollarSign, kind: 'navigate', to: '/cost', keywords: 'billing tokens' },
      { id: 'nav-logs', label: 'Logs', hint: 'Activity log', icon: ClipboardList, kind: 'navigate', to: '/logs', keywords: 'history audit' },
      { id: 'nav-doctor', label: 'Doctor', hint: 'Diagnostics', icon: Stethoscope, kind: 'navigate', to: '/doctor', keywords: 'health diagnostics' },
      { id: 'nav-pairing', label: 'Pairing', hint: 'Device pairing', icon: Key, kind: 'navigate', to: '/pairing', keywords: 'auth login' },
      { id: 'nav-canvas', label: 'Canvas', hint: 'Generated artefacts', icon: ImageIcon, kind: 'navigate', to: '/canvas', keywords: 'images gallery' },
      // Actions
      { id: 'act-reload', label: 'Reload dashboard', hint: 'Refresh the page', icon: RefreshCw, kind: 'action', run: () => window.location.reload(), keywords: 'refresh reset' },
      { id: 'act-logout', label: 'Sign out', hint: 'Clear pairing token', icon: LogOut, kind: 'action', run: onLogout, keywords: 'logout disconnect' },
    ],
    [onLogout],
  );

  // ── Filtering ─────────────────────────────────────────────────────
  // Lowercased substring match against `label + hint + keywords`. Cheap
  // and predictable; good enough for ~20 commands. If the catalog grows
  // past ~50, swap for a small fuzzy library (fzy / fuzzysort).
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands.filter((c) => {
      const haystack = `${c.label} ${c.hint ?? ''} ${c.keywords ?? ''}`.toLowerCase();
      return haystack.includes(q);
    });
  }, [commands, query]);

  // ── Global hotkey + open/close lifecycle ──────────────────────────
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      const isMac = navigator.platform.toUpperCase().includes('MAC');
      const modOk = isMac ? e.metaKey : e.ctrlKey;
      if (modOk && (e.key === 'k' || e.key === 'K')) {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // Reset state every time the palette opens. Auto-focus the input on
  // the next tick (the DOM element may not be rendered yet on the same
  // tick the state flips).
  useEffect(() => {
    if (open) {
      setQuery('');
      setCursor(0);
      const id = window.setTimeout(() => inputRef.current?.focus(), 0);
      return () => window.clearTimeout(id);
    }
    return undefined;
  }, [open]);

  // Clamp cursor when the filtered list shrinks below the current index.
  useEffect(() => {
    if (cursor >= filtered.length) setCursor(Math.max(0, filtered.length - 1));
  }, [cursor, filtered.length]);

  // Scroll the active row into view when cursor moves via keyboard.
  useEffect(() => {
    const node = listRef.current?.querySelector<HTMLElement>(`[data-cmd-index="${cursor}"]`);
    node?.scrollIntoView({ block: 'nearest' });
  }, [cursor]);

  // ── Selection ────────────────────────────────────────────────────
  const execute = useCallback(
    (item: CommandItem) => {
      setOpen(false);
      if (item.kind === 'navigate' && item.to) {
        if (location.pathname !== item.to) navigate(item.to);
      } else if (item.kind === 'action' && item.run) {
        item.run();
      }
    },
    [location.pathname, navigate],
  );

  const onInputKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        setOpen(false);
        return;
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        setCursor((c) => Math.min(c + 1, Math.max(0, filtered.length - 1)));
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        setCursor((c) => Math.max(0, c - 1));
        return;
      }
      if (e.key === 'Enter') {
        e.preventDefault();
        const item = filtered[cursor];
        if (item) execute(item);
      }
    },
    [cursor, execute, filtered],
  );

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      className="fixed inset-0 z-[100] flex items-start justify-center pt-[12vh] animate-fade-in"
      style={{ background: 'rgba(0, 0, 0, 0.55)' }}
      onMouseDown={(e) => {
        // Click on the backdrop closes; click on the inner panel does not.
        if (e.target === e.currentTarget) setOpen(false);
      }}
    >
      <div
        className="surface-panel w-full max-w-xl mx-4 flex flex-col overflow-hidden"
        style={{ maxHeight: '70vh' }}
      >
        {/* Search input */}
        <div className="flex items-center gap-3 px-4 py-3 border-b" style={{ borderColor: 'var(--pc-border)' }}>
          <Search size={16} style={{ color: 'var(--pc-text-muted)' }} />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setCursor(0);
            }}
            onKeyDown={onInputKeyDown}
            placeholder="Type a command or page…"
            className="flex-1 bg-transparent outline-none text-sm"
            style={{ color: 'var(--pc-text-primary)' }}
            aria-label="Command search"
            aria-autocomplete="list"
          />
          <kbd
            className="text-xs px-1.5 py-0.5 rounded font-mono"
            style={{ background: 'var(--pc-bg-base)', color: 'var(--pc-text-muted)' }}
          >
            esc
          </kbd>
        </div>

        {/* Results list */}
        <div ref={listRef} className="overflow-y-auto py-1" role="listbox">
          {filtered.length === 0 && (
            <div className="px-4 py-6 text-center text-sm" style={{ color: 'var(--pc-text-muted)' }}>
              No matches for &ldquo;{query}&rdquo;
            </div>
          )}
          {filtered.map((item, idx) => {
            const Icon = item.icon;
            const active = idx === cursor;
            return (
              <button
                key={item.id}
                data-cmd-index={idx}
                type="button"
                role="option"
                aria-selected={active}
                onMouseEnter={() => setCursor(idx)}
                onClick={() => execute(item)}
                className="w-full flex items-center gap-3 px-4 py-2.5 text-left text-sm transition-colors"
                style={{
                  background: active ? 'var(--pc-accent-glow)' : 'transparent',
                  color: 'var(--pc-text-primary)',
                }}
              >
                <Icon size={16} style={{ color: active ? 'var(--pc-accent)' : 'var(--pc-text-muted)' }} />
                <span className="flex-1">{item.label}</span>
                {item.hint && (
                  <span className="text-xs" style={{ color: 'var(--pc-text-muted)' }}>
                    {item.hint}
                  </span>
                )}
                {active && (
                  <CornerDownLeft size={14} style={{ color: 'var(--pc-text-muted)' }} aria-hidden="true" />
                )}
              </button>
            );
          })}
        </div>

        {/* Footer with key hints */}
        <div
          className="flex items-center gap-4 px-4 py-2 text-xs border-t"
          style={{ borderColor: 'var(--pc-border)', color: 'var(--pc-text-muted)' }}
        >
          <span className="flex items-center gap-1">
            <Activity size={12} aria-hidden="true" />
            {filtered.length} {filtered.length === 1 ? 'command' : 'commands'}
          </span>
          <span className="ml-auto flex items-center gap-2">
            <kbd className="px-1.5 py-0.5 rounded font-mono" style={{ background: 'var(--pc-bg-base)' }}>↑↓</kbd>
            navigate
            <kbd className="px-1.5 py-0.5 rounded font-mono" style={{ background: 'var(--pc-bg-base)' }}>⏎</kbd>
            select
          </span>
        </div>
      </div>
    </div>
  );
}
