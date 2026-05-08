# Building the Octopus Labs desktop app (Tauri)

The desktop client lives in [`apps/tauri/`](https://github.com/zeroclaw-labs/zeroclaw/tree/master/apps/tauri).
It's a Tauri 2.x system-tray app that hosts the React dashboard and
talks to a local `zeroclaw daemon` over the gateway.

This guide covers:

1. Local dev (`cargo tauri dev`)
2. Production bundles (Windows `.msi`, macOS `.dmg`, Linux `.AppImage`)
3. Code signing (Windows + macOS)
4. CI release pipeline (GitHub Actions)

## 1. Prerequisites

| OS | Toolchain |
|---|---|
| **All** | Rust stable (1.86+), Node 20+, [Tauri prereqs](https://tauri.app/start/prerequisites/) |
| **Windows** | MSVC + WebView2 Runtime (preinstalled on Win11) |
| **macOS** | Xcode CLT (`xcode-select --install`) |
| **Linux** | `webkit2gtk-4.1`, `libsoup-3.0`, `libayatana-appindicator3-1` |

Install the Tauri CLI globally:

```bash
cargo install tauri-cli --version "^2.0"
# or, project-pinned via Cargo.toml workspace tools (preferred):
cargo install --path tools/tauri-cli  # if present
```

## 2. Dev loop

The Tauri shell expects the gateway to be up at `127.0.0.1:42617`.
Start it in a separate terminal:

```bash
cargo run --bin zeroclaw -- daemon
```

Build the React bundle once (Tauri serves the bundled `dist/`):

```bash
cd web && npm install && npm run build
```

Then run Tauri in dev:

```bash
cargo tauri dev --config apps/tauri/tauri.conf.json
```

This compiles the desktop binary and opens a window pointing at
`http://127.0.0.1:42617/_app/`. Hot reload on the React side requires
running `npm run dev` (vite at :5173) and editing `tauri.conf.json` to
point `devUrl` at `http://localhost:5173/`.

## 3. Production bundles

```bash
# 1. Static web bundle for the embedded webview
cd web && npm ci && npm run build && cd ..

# 2. Desktop binary + installer
cargo tauri build --config apps/tauri/tauri.conf.json
```

Output (relative to repo root):

| Platform | Path |
|---|---|
| Windows | `target/release/bundle/msi/Octopus Labs_<version>_x64_en-US.msi` |
| Windows (portable) | `target/release/bundle/nsis/Octopus Labs_<version>_x64-setup.exe` |
| macOS | `target/release/bundle/dmg/Octopus Labs_<version>_aarch64.dmg` |
| macOS app | `target/release/bundle/macos/Octopus Labs.app` |
| Linux AppImage | `target/release/bundle/appimage/octopus-labs_<version>_amd64.AppImage` |
| Linux deb | `target/release/bundle/deb/octopus-labs_<version>_amd64.deb` |

### Targeting both architectures on macOS

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo tauri build --target universal-apple-darwin
```

This produces a fat binary that runs on Intel and Apple Silicon.

### Stripping debuginfo

Add to the workspace `Cargo.toml`:

```toml
[profile.release]
strip = "debuginfo"
lto = "thin"
codegen-units = 1
```

Cuts ~30% off the binary size at the cost of ~2× build time.

## 4. Code signing

Unsigned bundles trigger SmartScreen / Gatekeeper warnings. For
distribution outside `cargo install`, sign:

### Windows — Authenticode

1. Get a code-signing cert (Sectigo, DigiCert, etc. — EV recommended for
   instant SmartScreen reputation).
2. Export the PFX:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY="C:\certs\octopus.pfx"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD="..."
```

3. Add to `tauri.conf.json`:

```json
"bundle": {
  "windows": {
    "certificateThumbprint": "ABCDEF...",
    "digestAlgorithm": "sha256",
    "timestampUrl": "http://timestamp.digicert.com"
  }
}
```

### macOS — Developer ID + notarization

1. Enrol in the Apple Developer Program (US$99/yr).
2. Generate a "Developer ID Application" certificate via Xcode.
3. Create an [app-specific password](https://appleid.apple.com/account/manage)
   for `notarytool`.
4. Export envs:

```bash
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="abcd-efgh-ijkl-mnop"
export APPLE_TEAM_ID="ABCDE12345"
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (ABCDE12345)"
```

5. `cargo tauri build` will sign and notarize automatically when those
   env vars are set. Verify:

```bash
spctl -a -v "target/release/bundle/macos/Octopus Labs.app"
# → "accepted" → "source=Notarized Developer ID"
```

## 5. Auto-update endpoint (optional)

`tauri.conf.json`:

```json
"plugins": {
  "updater": {
    "active": true,
    "endpoints": ["https://updates.octopus.example.com/{{target}}/{{current_version}}"],
    "dialog": true,
    "pubkey": "RWQyM2QzYjAtMmQ3Ni00YjI3LTllZmEtMzAyZjE..."
  }
}
```

Generate the keypair once with `cargo tauri signer generate`, host
the public side, keep the private side in `TAURI_SIGNING_PRIVATE_KEY`.

The release pipeline below uploads the `latest.json` manifest +
signed bundles to that endpoint on every tag.

## 6. CI release with GitHub Actions

`.github/workflows/desktop-release.yml`:

```yaml
name: Desktop release

on:
  push:
    tags: ["desktop-v*"]

permissions:
  contents: write

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - os: macos-14
            target: aarch64-apple-darwin
          - os: macos-14
            target: x86_64-apple-darwin
          - os: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: npm, cache-dependency-path: web/package-lock.json }

      - name: Linux deps
        if: runner.os == 'Linux'
        run: |
          sudo apt-get update
          sudo apt-get install -y libwebkit2gtk-4.1-dev libsoup-3.0-dev \
            libappindicator3-dev librsvg2-dev patchelf

      - name: Build web bundle
        run: cd web && npm ci && npm run build

      - uses: tauri-apps/tauri-action@v0
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
        with:
          configPath: apps/tauri/tauri.conf.json
          args: --target ${{ matrix.target }}
          tagName: ${{ github.ref_name }}
          releaseName: "Octopus Labs Desktop ${{ github.ref_name }}"
          releaseDraft: true
          prerelease: false
```

Tag and push:

```bash
git tag desktop-v0.1.0
git push origin desktop-v0.1.0
```

Three runners build in parallel; `tauri-action` uploads the signed
bundles + the auto-update manifest to a draft GitHub Release.

## 7. Troubleshooting

| Symptom | Fix |
|---|---|
| `error: failed to run custom build command for tauri-build` | Update Tauri CLI to 2.0+ and ensure `webkit2gtk-4.1` (not 4.0) on Linux |
| Window opens blank | The gateway isn't up — `cargo run --bin zeroclaw -- daemon` first |
| macOS "app is damaged" | The bundle was downloaded but not notarized; either sign or `xattr -cr "Octopus Labs.app"` for local use |
| Windows: WebView2 missing | Ship the [Evergreen Bootstrapper](https://developer.microsoft.com/microsoft-edge/webview2/) inside your installer (Tauri does this by default in MSI) |
| `cargo tauri dev` rebuilds the daemon constantly | The Tauri crate depends on workspace deps that change; use `cargo tauri dev` with `--no-dev-server` and a pre-built daemon |
| AppImage segfaults on boot (Linux) | Older glibc; build on Ubuntu 22.04 (matches workflow above) — never on rolling distros |

## 8. Mobile (Android / iOS)

Tauri 2 supports mobile via `cargo tauri android` / `cargo tauri ios`.
The current `apps/tauri/` config targets desktop only. To start
mobile:

```bash
cargo tauri android init
cargo tauri ios init      # macOS only
```

This generates `gen/android/` and `gen/apple/` Gradle / Xcode
projects. Mobile capabilities live in
`apps/tauri/capabilities/mobile.json`. Build:

```bash
cargo tauri android dev      # connected device or emulator
cargo tauri android build    # → APK in gen/android/app/build/
```

A full mobile guide will land once we ship CI for it.
