# Desktop build — roadmap

A self-contained Mac / Windows / Linux app that bundles the daemon,
the web dashboard, and a tray icon so the operator never needs to
touch a terminal. Same Rust binary, same React UI, packaged as a
native installer.

This is the design + execution plan. **No code yet** — execute when
the hosted version is stable.

---

## 1. Why Tauri

ZeroClaw is already Rust + a bundled SPA. The two viable wrappers:

| Wrapper | Verdict |
|---|---|
| **Tauri 2** | ✅ Native fit. The daemon's runtime can live inside the Tauri Rust process or be spawned as a sidecar. Webview = system WebView2/WebKit/WebKitGTK, so installer is small (~10 MB on Linux, ~30 MB on Windows, ~5 MB on Mac). Built-in updater, code-signing helpers, tray API, secure IPC. |
| Electron | Works but adds 80–120 MB to every install for Chromium it doesn't need. The dashboard is already framework-agnostic enough to ship on the system webview. Reject. |
| Pure Rust + winit + wry | Possible but reinvents Tauri's installer / updater / signing for no benefit. Reject. |

**Decision: Tauri 2.x.** Single source of truth for both desktop and
hosted — same `web/dist`, same `zeroclaw` binary semantics.

---

## 2. Architecture

```
┌─────────────────────────────────────────────────┐
│  ZeroClaw.app  (Tauri 2 process)                │
│                                                 │
│  ┌──────────────────────────────────────────┐   │
│  │  Tauri main (Rust)                       │   │
│  │  - launches the daemon as an in-process  │   │
│  │    task, NOT a separate process          │   │
│  │  - serves web/dist via Tauri's asset     │   │
│  │    protocol (no HTTP at all to webview)  │   │
│  │  - tray icon + global shortcut           │   │
│  │  - native menu (File/Edit/View/Window)   │   │
│  │  - autostart on login                    │   │
│  │  - updater (Tauri JSON manifest)         │   │
│  └─────────────┬────────────────────────────┘   │
│                │                                │
│  ┌─────────────▼────────────────────────────┐   │
│  │  WebView (React dashboard)               │   │
│  │  - same web/dist as hosted version       │   │
│  │  - talks to localhost:42617 OR via       │   │
│  │    Tauri commands (preferred)            │   │
│  └──────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
                 │
                 │  (still listens on 127.0.0.1:42617
                 │   for compatibility with channels +
                 │   external integrations)
                 ▼
            Channels / MCP / Cron / Memory
```

**Two integration modes for the webview ↔ daemon path:**

1. **HTTP loopback** (zero-changes mode). The dashboard hits
   `http://127.0.0.1:42617/api/...` exactly like the hosted version.
   Pros: no UI changes, tested code path. Cons: opens a local port
   that other apps could probe, requires the same pairing flow even
   though we trust the local user.
2. **Tauri commands** (native mode). New `#[tauri::command]` shims
   in Rust call the same backend functions; the webview uses
   `invoke('command_name', payload)`. Pros: no port, no pairing,
   tighter security model. Cons: needs a `lib/api-tauri.ts`
   that mirrors the HTTP one, and a feature flag so the SPA picks
   the right transport.

**Recommendation: ship both, default to native, fall back to HTTP.**
The transport selection is a single bool at SPA boot:

```ts
// web/src/lib/transport.ts
export const usingTauri = '__TAURI__' in window;
export const apiFetch = usingTauri ? tauriInvoke : httpFetch;
```

This way the dashboard is one codebase that works in both worlds.

---

## 3. What changes per platform

### macOS

- App bundle: `ZeroClaw.app` with `Contents/MacOS/zeroclaw-tauri`
- Workspace: `~/Library/Application Support/ZeroClaw/`
- Config:    `~/Library/Application Support/ZeroClaw/config.toml`
- Logs:      `~/Library/Logs/ZeroClaw/zeroclaw.log`
- Code signing: Developer ID Application cert + notarisation (mandatory
  for distribution outside Mac App Store)
- Universal binary: `cargo build --target x86_64-apple-darwin && cargo
  build --target aarch64-apple-darwin && lipo` (Tauri does this in CI)
- Distribution: `.dmg` + `.app.tar.gz` for the Tauri updater

### Windows

- Installer: `.msi` (WiX) + `.exe` (NSIS); ship both
- Workspace: `%APPDATA%\ZeroClaw\`
- Config:    `%APPDATA%\ZeroClaw\config.toml`
- Logs:      `%LOCALAPPDATA%\ZeroClaw\Logs\`
- Code signing: EV cert (or standard cert + SmartScreen reputation
  building over time)
- WebView: WebView2 (auto-installed if missing on Windows 10; built
  in on Windows 11)
- Autostart: registry `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
  (Tauri's autostart plugin handles this)

### Linux

- Bundles: `.deb`, `.rpm`, `.AppImage`, optionally Flatpak/Snap
- Workspace: `~/.local/share/zeroclaw/`
- Config:    `~/.config/zeroclaw/config.toml`
- Logs:      `~/.local/state/zeroclaw/`
- WebView: WebKitGTK 4.1 (Tauri 2 requires it; standard on Debian 12+,
  Ubuntu 24.04+, Fedora 40+)
- Autostart: XDG autostart entry in `~/.config/autostart/`
- Distribution: GitHub Releases + `apt` repo via `deb-s3` if you
  outgrow Releases

---

## 4. Repository layout

Add `crates/zeroclaw-tauri/` alongside the existing crates:

```
crates/
  zeroclaw-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      main.rs              # Tauri builder + setup hook
      commands.rs          # #[tauri::command] shims
      tray.rs              # Tray icon + menu
      autostart.rs         # Plugin wiring
      updater.rs           # Updater wiring
    icons/                 # 32x32 / 128x128 / 256x256 / .icns / .ico
    capabilities/
      default.json         # Tauri permissions whitelist

web/                       # No changes needed — same SPA
  src/
    lib/
      transport.ts         # NEW — HTTP vs Tauri detection
```

`web/dist` is consumed by Tauri at build time as the frontend
bundle. The same `npm run build` that generates the hosted bundle
generates the desktop bundle. No fork, no diverging build.

---

## 5. Implementation plan (sequenced)

Each step is a self-contained PR.

### Step 1 — Tauri scaffolding

```bash
cd crates && cargo new --lib zeroclaw-tauri
cd zeroclaw-tauri
cargo add tauri --features 'protocol-asset,system-tray,updater'
cargo add tauri-plugin-autostart
cargo add tauri-plugin-window-state
cargo add tauri-plugin-updater
```

`tauri.conf.json` skeleton (verify against current Tauri 2 docs via
Context7 before committing — schema moves):

```jsonc
{
  "productName": "ZeroClaw",
  "version": "0.1.0",
  "identifier": "com.octopuslabs.zeroclaw",
  "build": {
    "frontendDist": "../../web/dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "cd ../../web && npm run dev",
    "beforeBuildCommand": "cd ../../web && npm run build"
  },
  "app": {
    "windows": [
      {
        "label": "main",
        "title": "ZeroClaw",
        "width": 1400,
        "height": 900,
        "minWidth": 1100,
        "minHeight": 720,
        "decorations": true,
        "transparent": false
      }
    ],
    "trayIcon": {
      "iconPath": "icons/tray.png",
      "menuOnLeftClick": false
    }
  },
  "bundle": {
    "active": true,
    "targets": ["app", "dmg", "deb", "rpm", "appimage", "msi", "nsis"],
    "icon": ["icons/icon.png"]
  }
}
```

**Acceptance:** `cargo tauri dev` opens a window showing the dashboard.

### Step 2 — Embed the daemon

Add a setup hook that starts the daemon's tokio runtime as a task,
not a sidecar. Reuses `zeroclaw-runtime` + `zeroclaw-gateway` libs.

```rust
// crates/zeroclaw-tauri/src/main.rs (sketch)
fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = zeroclaw_gateway::run_embedded(...).await {
                    tracing::error!(?e, "daemon exited");
                    let _ = app_handle.emit("daemon-died", e.to_string());
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::status])
        .run(tauri::generate_context!())
        .expect("error launching ZeroClaw");
}
```

Pre-req: extract a `pub async fn run_embedded()` from
`zeroclaw-gateway::lib` that takes an `AppState` and spins the
gateway without `main()`. Today the entrypoint is in the `zeroclaw`
binary crate; the lib functions are mostly there but `run_embedded`
needs to be the public one.

**Acceptance:** the desktop app talks to its own daemon on localhost,
no separate process.

### Step 3 — Native transport (`#[tauri::command]`)

Mirror the most-hit endpoints as Tauri commands so the SPA can skip
HTTP for those:

- `get_status()` → `/api/status`
- `list_agents()` → `/api/agents`
- `list_tenants()` → `/api/tenants`
- `create_tenant(body)` → `POST /api/tenants`
- `send_message(body)` → `/webhook`

`web/src/lib/transport.ts` picks Tauri vs HTTP at boot. Pairing is
**bypassed entirely** in Tauri mode — the local user is trusted.

**Acceptance:** `/pairing` page is hidden, no pair code needed.
Network inspector shows zero HTTP traffic to localhost:42617.

### Step 4 — Tray icon + lifecycle

- Click tray → show window
- Right-click tray → menu: Show / Hide / Open Logs / Open Workspace
  Folder / Quit
- Window close button → hide (don't quit)
- Cmd/Ctrl+Q → quit
- macOS: dock icon visibility configurable

### Step 5 — Autostart + updater

`tauri-plugin-autostart` for "launch at login". `tauri-plugin-updater`
hits a JSON manifest hosted on GitHub Pages or the same Hetzner
instance as `https://octopus.example.com/desktop/manifest.json`:

```json
{
  "version": "0.2.0",
  "notes": "Bug fixes",
  "pub_date": "2026-05-15T00:00:00Z",
  "platforms": {
    "darwin-aarch64": { "signature": "...", "url": "https://.../ZeroClaw_0.2.0_aarch64.app.tar.gz" },
    "darwin-x86_64":  { "signature": "...", "url": "https://.../ZeroClaw_0.2.0_x64.app.tar.gz" },
    "windows-x86_64": { "signature": "...", "url": "https://.../ZeroClaw_0.2.0_x64-setup.nsis.zip" },
    "linux-x86_64":   { "signature": "...", "url": "https://.../zeroclaw_0.2.0_amd64.AppImage.tar.gz" }
  }
}
```

Tauri verifies the signature with a public key burned into the
binary at build time. Generate the keypair once, keep the private
key in your CI secrets.

### Step 6 — CI for releases

GitHub Actions matrix on macos-14 (arm64), macos-13 (x86_64),
windows-2022, ubuntu-22.04. Each job:

1. `npm ci && npm run build` in `web/`
2. `cargo tauri build --target <triple>`
3. Sign artefacts (Developer ID for Mac, EV cert for Windows)
4. Upload to GitHub Releases
5. Update `manifest.json` + push to release branch

A `tauri-apps/tauri-action` GitHub Action wraps most of this.

### Step 7 — Notarisation + signing

- **macOS**: enrol in Apple Developer Program ($99/yr). Generate
  Developer ID Application cert in Xcode → Keychain. Notarise via
  `xcrun notarytool` in CI; staple the ticket post-notarisation.
- **Windows**: cheapest credible cert is OV from Sectigo / SSL.com
  (~$200/yr). EV cert ($400+/yr) skips SmartScreen reputation
  building but isn't strictly necessary at first.
- **Linux**: AppImage is unsigned by convention; `.deb` and `.rpm`
  can be signed with a GPG key — useful only if you set up a custom
  apt repo.

---

## 6. What stays the same

- Hosted version — unchanged. `cargo build --release && systemctl
  restart zeroclaw` continues to work.
- Web dashboard — unchanged. Same components, same routes; only
  `lib/transport.ts` differs at runtime.
- All agent presets, channels, MCP, memory backends — unchanged.
- The pairing flow — only matters in hosted mode. Desktop trusts
  the local user implicitly.

---

## 7. What needs minor refactor first

Before Step 2 ships, surface these as `pub`:

- `zeroclaw_gateway::run_embedded(config, workspace_dir, …)` — today
  the gateway boot is wrapped in the `zeroclaw daemon` CLI; needs
  to be callable from Tauri's setup hook.
- `zeroclaw_runtime::observability` — Tauri can subscribe to the
  same event channel the SSE endpoint emits, surfacing it as Tauri
  events for the dashboard.
- A way to point `XDG_CONFIG_HOME` / Tauri's app-data dirs at the
  workspace location — Tauri's `path` API gives the right per-OS
  directories; the config loader needs to accept an explicit base
  path argument (probably already does — verify).

Estimated effort: 1–2 days for the refactor, 1–2 days for Steps 1–4,
1 day for Steps 5–6, ½ day for Step 7 plumbing. Plus signing-cert
provisioning calendar time.

---

## 8. Open questions

- **Auto-launch the agent on login by default, or opt-in?** Lean
  opt-in — surprise daemons running in the background are bad UX.
- **System notifications for agent events?** Tauri has notification
  API; could surface `tool_call_start` for long-running operations,
  agent errors, channel events.
- **Multi-window?** Could put the orchestrator in one window, the
  agent chat in another, the logs in a third. Tauri supports it
  cleanly. Probably wait for user demand.
- **Local-LLM mode?** The desktop is the natural place to bundle a
  small Ollama-compatible model for offline use. Out of scope for
  v1 but worth keeping the door open.

---

## 9. When to start

After:
- The hosted version on Hetzner has run continuously for ≥2 weeks
  with no operator intervention
- Pairing flow has been used by ≥3 different devices
- Channel webhooks have processed real traffic
- Backups have been restored once successfully

Until then, every desktop bug surfaces as "is it the daemon, the
wrapper, or both?" and the diagnosis cost is high. Get the daemon
boring first; then wrap it.
