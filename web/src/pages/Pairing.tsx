// Admin pairing console — mission-control panel for issuing one-time
// pair codes, listing paired devices, and revoking / rotating their
// tokens. Mirrors the visual language of the orchestrator (PNL-X
// chrome, monospace tabular readouts, status lights).

import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Smartphone,
  Trash2,
  RefreshCw,
  Copy,
  CheckCircle2,
  Radio,
  KeyRound,
  AlertCircle,
  QrCode,
} from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import { getAdminPairCode } from '../lib/api';

interface Device {
  id: string;
  name: string | null;
  device_type: string | null;
  paired_at: string;
  last_seen: string;
  ip_address: string | null;
}

const PAIR_CODE_TTL_SEC = 300; // server-side TTL for pair codes

export default function Pairing() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(true);
  const [pairingCode, setPairingCode] = useState<string | null>(null);
  const [pairingIssuedAt, setPairingIssuedAt] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [, tick] = useState(0);
  const tickRef = useRef<number | null>(null);

  const token = localStorage.getItem('zeroclaw_token') || '';

  // 1Hz tick so the countdown / last-seen readouts decay smoothly.
  useEffect(() => {
    tickRef.current = window.setInterval(() => tick((n) => n + 1), 1000);
    return () => {
      if (tickRef.current !== null) window.clearInterval(tickRef.current);
    };
  }, []);

  const fetchDevices = useCallback(async () => {
    try {
      const res = await fetch('/api/devices', {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json();
        setDevices(data.devices || []);
      } else if (res.status === 401) {
        setError('Unauthorized — re-pair to continue');
      }
    } catch {
      setError('Failed to load devices');
    } finally {
      setLoading(false);
    }
  }, [token]);

  useEffect(() => {
    // If the server already has a code (e.g. CLI generated), show it.
    getAdminPairCode()
      .then((data) => {
        if (data.pairing_code) {
          setPairingCode(data.pairing_code);
          setPairingIssuedAt(Date.now());
        }
      })
      .catch(() => {
        // Endpoint is localhost-only, fine in production
      });
  }, []);

  useEffect(() => {
    fetchDevices();
  }, [fetchDevices]);

  const issuePairCode = async () => {
    setError(null);
    try {
      const res = await fetch('/api/pairing/initiate', {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json();
        setPairingCode(data.pairing_code);
        setPairingIssuedAt(Date.now());
        setCopied(false);
      } else {
        setError(`Failed to issue pair code (HTTP ${res.status})`);
      }
    } catch {
      setError('Failed to issue pair code');
    }
  };

  const revokeDevice = async (deviceId: string) => {
    try {
      const res = await fetch(`/api/devices/${deviceId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        setDevices((prev) => prev.filter((d) => d.id !== deviceId));
      } else {
        setError(`Revoke failed (HTTP ${res.status})`);
      }
    } catch {
      setError('Revoke failed');
    }
  };

  const rotateDeviceToken = async (deviceId: string) => {
    try {
      const res = await fetch(`/api/devices/${deviceId}/token/rotate`, {
        method: 'POST',
        headers: { Authorization: `Bearer ${token}` },
      });
      if (!res.ok) setError(`Rotate failed (HTTP ${res.status})`);
      else fetchDevices();
    } catch {
      setError('Rotate failed');
    }
  };

  const copyCode = async () => {
    if (!pairingCode) return;
    try {
      await navigator.clipboard.writeText(pairingCode);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      // clipboard blocked by permissions — silent
    }
  };

  // Derive countdown
  const codeAgeSec =
    pairingIssuedAt !== null
      ? Math.floor((Date.now() - pairingIssuedAt) / 1000)
      : null;
  const codeRemainingSec =
    codeAgeSec !== null ? Math.max(0, PAIR_CODE_TTL_SEC - codeAgeSec) : null;
  const codeExpired = codeRemainingSec !== null && codeRemainingSec === 0;

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div
          className="h-8 w-8 border-2 rounded-full animate-spin"
          style={{
            borderColor: 'rgba(125, 211, 252, 0.2)',
            borderTopColor: '#7DD3FC',
          }}
        />
      </div>
    );
  }

  return (
    <div
      className="p-5 space-y-3 animate-fade-in min-h-screen"
      style={{ background: '#03060c', fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace' }}
    >
      {/* ── Console header ────────────────────────────────────── */}
      <div
        className="rounded border overflow-hidden"
        style={{
          background:
            'linear-gradient(180deg, rgba(12, 16, 24, 0.98), rgba(8, 11, 18, 0.95))',
          borderColor: 'rgba(125, 211, 252, 0.25)',
        }}
      >
        <div
          className="flex items-center justify-between gap-4 px-4 py-2 border-b text-[10px] flex-wrap"
          style={{
            borderColor: 'rgba(125, 211, 252, 0.08)',
            background: 'rgba(3, 6, 12, 0.7)',
          }}
        >
          <div className="flex items-center gap-3">
            <KeyRound className="h-3.5 w-3.5" style={{ color: '#7DD3FC' }} />
            <span style={{ color: '#5BA8D9', letterSpacing: '0.35em' }}>
              OCTOPUS · PAIRING CONSOLE
            </span>
            <span style={{ color: 'rgba(125, 211, 252, 0.25)' }}>│</span>
            <span style={{ color: '#94A3B8', letterSpacing: '0.2em' }}>
              CON-PAIR
            </span>
          </div>
          <div className="flex items-center gap-3">
            <span
              className="px-2 py-0.5 inline-flex items-center gap-1.5 rounded-sm"
              style={{
                background: 'rgba(125, 211, 252, 0.08)',
                border: '1px solid rgba(125, 211, 252, 0.3)',
                color: '#BAE6FD',
                letterSpacing: '0.25em',
              }}
            >
              <Radio className="h-3 w-3" />
              {devices.length} DEVICES
            </span>
            <button
              onClick={issuePairCode}
              className="px-3 py-1.5 text-[10px] inline-flex items-center gap-1.5 rounded-sm border"
              style={{
                background: 'rgba(125, 211, 252, 0.12)',
                borderColor: '#7DD3FC',
                color: '#E0F2FE',
                letterSpacing: '0.25em',
                fontWeight: 600,
              }}
            >
              <Smartphone className="h-3 w-3" /> ISSUE NEW CODE
            </button>
          </div>
        </div>

        {/* Title row */}
        <div className="px-4 py-3">
          <p
            className="text-[10px]"
            style={{
              color: '#5BA8D9',
              letterSpacing: '0.4em',
            }}
          >
            DEVICE REGISTRY · TOKEN LIFECYCLE
          </p>
          <p
            className="text-xl mt-0.5"
            style={{
              color: '#E0F2FE',
              letterSpacing: '0.18em',
              fontWeight: 500,
            }}
          >
            PAIR CONTROL
          </p>
        </div>
      </div>

      {/* ── Error banner ──────────────────────────────────────── */}
      {error && (
        <div
          className="rounded border p-3 text-[11px] flex items-center justify-between gap-3"
          style={{
            background: 'rgba(248, 113, 113, 0.08)',
            borderColor: 'rgba(248, 113, 113, 0.4)',
            color: '#FCA5A5',
            letterSpacing: '0.06em',
          }}
        >
          <span className="flex items-center gap-2">
            <AlertCircle className="h-3.5 w-3.5" />
            ERR · {error}
          </span>
          <button
            onClick={() => setError(null)}
            className="font-bold"
            style={{ color: '#F87171' }}
          >
            ×
          </button>
        </div>
      )}

      {/* ── PNL-A · Pair code card (only when code issued) ───── */}
      {pairingCode && (
        <div
          className="rounded border overflow-hidden"
          style={{
            background: 'rgba(12, 16, 24, 0.92)',
            borderColor: codeExpired
              ? 'rgba(248, 113, 113, 0.4)'
              : 'rgba(134, 239, 172, 0.4)',
            borderLeft: `3px solid ${codeExpired ? '#F87171' : '#86EFAC'}`,
          }}
        >
          <div
            className="px-3 py-2 border-b text-[10px] flex items-center justify-between"
            style={{
              borderColor: 'rgba(125, 211, 252, 0.1)',
              letterSpacing: '0.3em',
              color: codeExpired ? '#F87171' : '#86EFAC',
              background: 'rgba(125, 211, 252, 0.04)',
            }}
          >
            <span>PNL-A · PAIR CODE</span>
            <span style={{ color: '#94A3B8' }}>
              {codeExpired ? 'EXPIRED · ISSUE A NEW ONE' : 'ONE-TIME · 6 DIGITS'}
            </span>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-[1fr_220px_260px] gap-0 items-stretch">
            {/* Big code */}
            <div className="p-6 flex flex-col items-center justify-center gap-3">
              <div
                className="text-5xl tabular-nums"
                style={{
                  color: codeExpired ? '#F87171' : '#E0F2FE',
                  letterSpacing: '0.45em',
                  fontWeight: 600,
                  textShadow: codeExpired
                    ? '0 0 12px rgba(248, 113, 113, 0.4)'
                    : '0 0 12px rgba(125, 211, 252, 0.35)',
                }}
              >
                {pairingCode}
              </div>
              <button
                onClick={copyCode}
                disabled={codeExpired}
                className="px-3 py-1.5 text-[10px] inline-flex items-center gap-1.5 rounded-sm border"
                style={{
                  background: copied
                    ? 'rgba(134, 239, 172, 0.12)'
                    : 'rgba(125, 211, 252, 0.06)',
                  borderColor: copied ? '#86EFAC' : 'rgba(125, 211, 252, 0.3)',
                  color: copied ? '#86EFAC' : '#BAE6FD',
                  letterSpacing: '0.25em',
                  opacity: codeExpired ? 0.4 : 1,
                  cursor: codeExpired ? 'not-allowed' : 'pointer',
                }}
              >
                {copied ? <CheckCircle2 className="h-3 w-3" /> : <Copy className="h-3 w-3" />}
                {copied ? 'COPIED' : 'COPY'}
              </button>
            </div>

            {/* QR for mobile pairing */}
            <div
              className="p-4 border-t md:border-t-0 md:border-l flex flex-col items-center justify-center gap-2"
              style={{
                borderColor: 'rgba(125, 211, 252, 0.1)',
                background: 'rgba(3, 6, 12, 0.4)',
                opacity: codeExpired ? 0.3 : 1,
              }}
            >
              <span
                className="text-[9px] inline-flex items-center gap-1.5"
                style={{
                  color: '#5BA8D9',
                  letterSpacing: '0.3em',
                }}
              >
                <QrCode className="h-3 w-3" /> SCAN TO PAIR
              </span>
              <div
                style={{
                  background: '#E0F2FE',
                  padding: 8,
                  borderRadius: 4,
                  boxShadow: codeExpired
                    ? 'none'
                    : '0 0 16px rgba(125, 211, 252, 0.25)',
                }}
              >
                <QRCodeSVG
                  value={`${window.location.origin}${window.location.pathname.replace(/\/pairing\/?$/, '/')}?pair=${pairingCode}`}
                  size={140}
                  level="M"
                  bgColor="#E0F2FE"
                  fgColor="#03060c"
                  includeMargin={false}
                />
              </div>
              <span
                className="text-[9px] text-center"
                style={{
                  color: '#94A3B8',
                  letterSpacing: '0.05em',
                  lineHeight: 1.3,
                }}
              >
                Open the camera on the new device
              </span>
            </div>

            {/* Telemetry side */}
            <div
              className="p-4 border-t md:border-t-0 md:border-l space-y-3 text-[10px]"
              style={{ borderColor: 'rgba(125, 211, 252, 0.1)' }}
            >
              <CodeReadout
                label="STATUS"
                value={codeExpired ? 'EXPIRED' : 'ARMED'}
                color={codeExpired ? '#F87171' : '#86EFAC'}
                pulse={!codeExpired}
              />
              <CodeReadout
                label="TTL"
                value={
                  codeRemainingSec !== null
                    ? `${codeRemainingSec.toString().padStart(3, '0')} S`
                    : '--- S'
                }
                color={
                  codeRemainingSec === null || codeRemainingSec === 0
                    ? '#F87171'
                    : codeRemainingSec < 30
                      ? '#FCD34D'
                      : '#86EFAC'
                }
              />
              <CodeReadout
                label="ISSUED"
                value={
                  codeAgeSec !== null
                    ? `${codeAgeSec.toString().padStart(3, '0')}S AGO`
                    : '— NIL —'
                }
                color="#BAE6FD"
              />
              <CodeReadout
                label="CHANNEL"
                value="POST /api/pair"
                color="#BAE6FD"
              />
            </div>
          </div>

          {/* Instructions row */}
          <div
            className="px-4 py-2 border-t text-[10px]"
            style={{
              borderColor: 'rgba(125, 211, 252, 0.08)',
              color: '#94A3B8',
              letterSpacing: '0.04em',
              lineHeight: 1.5,
            }}
          >
            ▸ On the new device: open this dashboard, enter{' '}
            <span style={{ color: '#BAE6FD', letterSpacing: '0.15em' }}>
              {pairingCode}
            </span>
            , click <em>Pair</em>. Or POST{' '}
            <code style={{ color: '#7DD3FC' }}>
              {`{"code":"${pairingCode}","device_name":"laptop"}`}
            </code>{' '}
            to <code style={{ color: '#7DD3FC' }}>/api/pair</code>.
          </div>
        </div>
      )}

      {/* ── PNL-B · Device registry table ─────────────────────── */}
      <div
        className="rounded border overflow-hidden"
        style={{
          background: 'rgba(12, 16, 24, 0.85)',
          borderColor: 'rgba(125, 211, 252, 0.18)',
        }}
      >
        <div
          className="px-3 py-2 border-b text-[10px] flex items-center justify-between"
          style={{
            borderColor: 'rgba(125, 211, 252, 0.1)',
            letterSpacing: '0.3em',
            background: 'rgba(125, 211, 252, 0.04)',
            color: '#5BA8D9',
          }}
        >
          <span>PNL-B · DEVICE REGISTRY · {devices.length} ACTIVE</span>
          <button
            onClick={fetchDevices}
            className="px-2 py-0.5 inline-flex items-center gap-1 rounded-sm border"
            style={{
              borderColor: 'rgba(125, 211, 252, 0.3)',
              color: '#7DD3FC',
              letterSpacing: '0.2em',
              fontSize: 9,
            }}
            title="Reload registry"
          >
            <RefreshCw className="h-2.5 w-2.5" /> SYNC
          </button>
        </div>

        {devices.length === 0 ? (
          <div
            className="px-4 py-8 text-center text-[11px]"
            style={{
              color: '#5BA8D9',
              letterSpacing: '0.2em',
            }}
          >
            ▸ NO DEVICES PAIRED · ISSUE A CODE TO START
          </div>
        ) : (
          <table className="w-full text-[10px] tabular-nums">
            <thead>
              <tr style={{ color: '#5BA8D9' }}>
                <th className="text-left px-3 py-1.5 font-normal" style={{ letterSpacing: '0.25em' }}>
                  STATE
                </th>
                <th className="text-left px-3 py-1.5 font-normal" style={{ letterSpacing: '0.25em' }}>
                  NAME
                </th>
                <th className="text-left px-3 py-1.5 font-normal" style={{ letterSpacing: '0.25em' }}>
                  TYPE
                </th>
                <th className="text-left px-3 py-1.5 font-normal" style={{ letterSpacing: '0.25em' }}>
                  PAIRED
                </th>
                <th className="text-left px-3 py-1.5 font-normal" style={{ letterSpacing: '0.25em' }}>
                  LAST SEEN
                </th>
                <th className="text-left px-3 py-1.5 font-normal" style={{ letterSpacing: '0.25em' }}>
                  IP
                </th>
                <th className="text-right px-3 py-1.5 font-normal" style={{ letterSpacing: '0.25em' }}>
                  ACT
                </th>
              </tr>
            </thead>
            <tbody>
              {devices.map((d) => (
                <DeviceRow
                  key={d.id}
                  device={d}
                  onRevoke={() => revokeDevice(d.id)}
                  onRotate={() => rotateDeviceToken(d.id)}
                />
              ))}
            </tbody>
          </table>
        )}
      </div>

      {/* ── Procedural footnote ───────────────────────────────── */}
      <div
        className="rounded border px-4 py-3 text-[10px] space-y-1.5"
        style={{
          background: 'rgba(12, 16, 24, 0.5)',
          borderColor: 'rgba(125, 211, 252, 0.1)',
          color: '#94A3B8',
          letterSpacing: '0.04em',
          lineHeight: 1.6,
        }}
      >
        <p style={{ color: '#5BA8D9', letterSpacing: '0.3em', fontSize: 9 }}>
          PROC · PAIR LIFECYCLE
        </p>
        <p>
          1. Issue a pair code (one-time, 5-min TTL). 2. Enter it on the new
          device, optionally with a `device_name`. 3. Server returns a bearer
          token; the device persists it in `localStorage`. 4. Revoke from this
          panel to invalidate the token; rotate to issue a fresh one for the
          same device entry.
        </p>
        <p style={{ color: '#5BA8D9' }}>
          Pair codes expire after 5 minutes. Tokens have no built-in TTL —
          revoke them when a device is no longer trusted.
        </p>
      </div>
    </div>
  );
}

function CodeReadout({
  label,
  value,
  color,
  pulse,
}: {
  label: string;
  value: string;
  color: string;
  pulse?: boolean;
}) {
  return (
    <div className="flex items-center justify-between">
      <span style={{ color: '#5BA8D9', letterSpacing: '0.3em' }}>{label}</span>
      <span className="inline-flex items-center gap-1.5">
        {pulse && (
          <span
            className="inline-block h-1.5 w-1.5 rounded-full"
            style={{
              background: color,
              boxShadow: `0 0 5px ${color}`,
              animation: 'pulse 1.6s infinite',
            }}
          />
        )}
        <span
          className="tabular-nums"
          style={{
            color,
            letterSpacing: '0.12em',
            fontWeight: 600,
          }}
        >
          {value}
        </span>
      </span>
    </div>
  );
}

function DeviceRow({
  device,
  onRevoke,
  onRotate,
}: {
  device: Device;
  onRevoke: () => void;
  onRotate: () => void;
}) {
  const lastSeen = new Date(device.last_seen);
  const ageSec = Math.max(0, (Date.now() - lastSeen.getTime()) / 1000);
  // active = last seen within 5 min, stale = within 24 h, dormant = older
  const status: 'active' | 'stale' | 'dormant' =
    ageSec < 300 ? 'active' : ageSec < 86400 ? 'stale' : 'dormant';
  const statusColor =
    status === 'active' ? '#86EFAC' : status === 'stale' ? '#FCD34D' : '#5BA8D9';
  const statusLabel =
    status === 'active' ? 'ONLINE' : status === 'stale' ? 'IDLE' : 'DORMANT';

  return (
    <tr style={{ borderTop: '1px solid rgba(125, 211, 252, 0.05)' }}>
      <td className="px-3 py-2">
        <span className="inline-flex items-center gap-1.5">
          <span
            className="inline-block h-1.5 w-1.5 rounded-full"
            style={{
              background: statusColor,
              boxShadow: status === 'active' ? `0 0 5px ${statusColor}` : 'none',
              animation: status === 'active' ? 'pulse 1.8s infinite' : undefined,
            }}
          />
          <span style={{ color: statusColor, letterSpacing: '0.2em' }}>
            {statusLabel}
          </span>
        </span>
      </td>
      <td
        className="px-3 py-2"
        style={{ color: '#E0F2FE', letterSpacing: '0.04em' }}
      >
        {device.name || 'Unnamed device'}
      </td>
      <td className="px-3 py-2" style={{ color: '#94A3B8' }}>
        {device.device_type || '—'}
      </td>
      <td className="px-3 py-2" style={{ color: '#94A3B8' }}>
        {new Date(device.paired_at).toISOString().slice(0, 10)}
      </td>
      <td className="px-3 py-2" style={{ color: '#94A3B8' }}>
        {formatAge(ageSec)}
      </td>
      <td className="px-3 py-2" style={{ color: '#94A3B8' }}>
        {device.ip_address || '—'}
      </td>
      <td className="px-3 py-2 text-right">
        <span className="inline-flex items-center gap-1">
          <button
            onClick={onRotate}
            className="px-1.5 py-0.5 inline-flex items-center gap-1 rounded-sm border"
            style={{
              borderColor: 'rgba(125, 211, 252, 0.3)',
              color: '#7DD3FC',
              letterSpacing: '0.18em',
              fontSize: 9,
            }}
            title="Rotate token (issues a fresh bearer for this device)"
          >
            <RefreshCw className="h-2.5 w-2.5" /> ROT
          </button>
          <button
            onClick={onRevoke}
            className="px-1.5 py-0.5 inline-flex items-center gap-1 rounded-sm border"
            style={{
              borderColor: 'rgba(248, 113, 113, 0.4)',
              color: '#F87171',
              letterSpacing: '0.18em',
              fontSize: 9,
            }}
            title="Revoke (invalidates the device's token)"
          >
            <Trash2 className="h-2.5 w-2.5" /> KILL
          </button>
        </span>
      </td>
    </tr>
  );
}

function formatAge(sec: number): string {
  if (sec < 60) return `${Math.floor(sec)}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m`;
  if (sec < 86400) return `${Math.floor(sec / 3600)}h`;
  return `${Math.floor(sec / 86400)}d`;
}
