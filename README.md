# QR Code Generator for Tezbar

Generate QR codes from text or URLs on Windows or macOS.

## Commands

- **Generate QR Code** opens a form for text/URL, color, format (PNG/SVG), UTM tracking and URL shortening, then saves, copies, or previews inline.
- **Generate QR Code from Clipboard** creates a QR from clipboard text.
- **Generate QR Code from Selection** creates a QR from selected text.

Output formats: PNG (transparent or with white background for preview) and SVG. Color presets and custom hex are supported. Files are saved to `~/Downloads` and copied via the system clipboard.

## Compatibility

- Windows: built and exercised in Tezbar on Windows 11, including file saves to Downloads and clipboard image copy.
- macOS: uses only cross-platform Node.js (`fs`, `os`, `path`) and `@raycast/api` and retains the upstream macOS-compatible design.
- Linux: not currently declared.

## Local verification

```powershell
pnpm install
pnpm typecheck
pnpm test
pnpm build
```

## Credits

Adapted for Tezbar from Raycast's MIT-licensed `qrcode-generator` extension at upstream commit `6734d85992290066fd31a7f5bee3cca3d3545a1a`. Original author `Melvynx` and contributors remain credited in `package.json`.

Native Rust V2: see [v2/README.md](v2/README.md).
