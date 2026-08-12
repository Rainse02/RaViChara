# Windows desktop packaging

RaViChara uses a native Winit window and the Windows WebView2 runtime. The same
process owns the Axum server, WebSocket streams, SQLite memory services and the
desktop window. Tauri CLI, Node.js and a separate browser process are not
required at runtime.

## Build

Rust's `minimal` profile is sufficient. `rustfmt` and `clippy` are not used.

```powershell
.\scripts\package_windows.ps1
```

To create a personal portable build containing the current SQLite databases and
runtime overrides:

```powershell
.\scripts\package_windows.ps1 -IncludeUserData
```

The result is written to:

```text
dist/RaViChara-0.6.0/RaViChara.exe
dist/RaViChara-0.6.0-Windows-x64-portable.zip
```

API keys and Blender tokens are removed from the packaged YAML in both modes.
The personal data option is intended only for the owner of those databases.

## Runtime directories

The packaged folder contains a `RaViChara.portable` marker, so configuration,
logs and SQLite data stay beside the executable:

```text
config/settings.yaml
data/overrides.json
data/memories/*.db
logs/ravichara.log
webview/
```

If the EXE is copied without the marker, it uses
`%LOCALAPPDATA%\RaViChara` and seeds a clean configuration and the bundled
character cards. `RAVICHARA_HOME` or `--data-dir <path>` can select another
location.

The Web UI assets, the original Lily example card, and its abstract SVG avatar
are embedded in the EXE. A normal public package copies only those two
redistributable character files. `-IncludeUserData` is a private-backup mode;
it also copies locally installed character assets so an active local card keeps
working. Do not publish an archive made with that option.
WebView2 keeps only browser profile data; Blender frames and synthesized audio
are never written to an application cache.

## Modes

- `RaViChara.exe`: desktop window plus the internal Axum service.
- `RaViChara.exe --server`: Axum service without a desktop window.
- `RaViChara.exe --data-dir D:\RaViCharaData`: explicit runtime data root.

Only one process can bind the configured local port. A second desktop launch
fails with an explicit message rather than silently attaching to an unrelated
service.

## System requirements

- Windows 10 or Windows 11, x64.
- Microsoft Edge WebView2 Runtime.
- External LLM, TTS and Blender services only when their corresponding features
  are enabled.
