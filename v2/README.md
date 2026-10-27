# QR Code Generator — external Rust V2

Uncommitted implementation of **all three own command entrypoints**, including the Round 2 review fixes. Production is an independent executable using `tezbar-extension-sdk` **0.1.1**, immutable HTTPS Git revision `e955911f0bf5b558f9d3eb5183df0adc95c22af2`, protocol 1, schema 3, `execution: process`. The independent SDK is linked into the executable; production has no app/Tauri, sibling-path, Node or host-embedded execution dependency. `Cargo.lock` is included. MSRV is Rust 1.85.

This is **not yet a publishable replacement** for V1. The selected committed host needs the unapplied generic renderer/resource patches below; SVG clipboard has an explicit limitation; live desktop acceptance and Windows MSVC execution remain gates. Implementation QA did not change the host catalog, install the package or publish a release. Platform acceptance and the prerequisites below remain outstanding.

## References and scope

- Own source: `6e983922022d54f38b114fe837ea1232a143cabb` (fresh `origin/main`, unchanged from parent inspection).
- Selected host: `ea100cee3f69711de12485ba061950c5fc65258f`. Only committed `git show` objects were read; no checkout, host edits/build or dirty-file dependency.
- SDK: `e955911f0bf5b558f9d3eb5183df0adc95c22af2`. Its newer default branch was deliberately not used.
- Upstream Raycast source/assets: `6734d85992290066fd31a7f5bee3cca3d3545a1a`.
- Branch: `feat/native-v2-rust`, from fetched `origin/main`. One clone/worktree only. Changes are confined to `v2/` and the one root README discovery link. V1 files otherwise retain their exact source bytes.

Git blob IDs/hashes and read-only host/SDK snapshots are in [PROVENANCE.json](PROVENANCE.json) and `integration/baseline/`. Those snapshots are **QA evidence**, not production dependencies. [INDEX.sha256](INDEX.sha256) inventories every Git-visible file except itself (self-hashing is impossible); ignored build/QA outputs are excluded explicitly.

## Implemented behavior

`index` presents URL/content, an initially unselected format, six source presets plus Custom (selected initially), normalized preference color, custom validation, a nonblocking luminance warning (`>0.4`, source WCAG formula), shortener disclosure/hint, and conditional UTM fields. Actions are source-ordered: save → save/show/copy; copy → copy/save/show; otherwise show/save/copy. Back/Edit retains drafts. Generation starts only on the selected explicit action; field changes, refresh and search perform no network/save/open/copy.

`clipboard` and `selection` acquire text once on launch, reject empty JS-trimmed input, retain the exact raw text, and prepare a white-background PNG preview. Their actions are Save to Downloads followed by Copy to Clipboard. Their Save action does not open the file. Index Save opens the actual saved path with `host.shell.open` and requires its structured acknowledgement.

Colors accept optional `#`, 3/6 hex digits and source trimming; case is preserved while expanding 3 digits. Invalid preferences become black. Invalid custom values block submission; valid light colors remain generatable. Text is encoded without truncation. UTM modifies HTTP(S) URLs only, applies trimmed nonempty parameters in source/medium/campaign/term/content order, and replaces duplicate keys with `URLSearchParams.set` semantics. Plain text/other schemes remain unchanged. Merely enabling UTM still normalizes an HTTP URL, as in V1.

Shortening happens only for an explicit submission with Shorten selected and HTTP(S) input, after UTM. Fixed endpoints: is.gd → da.gd → TinyURL, 8 seconds each, 4-second connect deadline, 4096 response bytes, no redirects/proxies. No live shortener request was made during QA. Local HTTP and mock fallback tests cover rejection/cancellation. All-provider failure aborts generation and offers an explicit, single-use Retry Generation action with unchanged fields; the retry preserves the original Show/Save/Copy intent. Error text never includes source URLs, endpoints with query values or remote response bodies. Redirect refusal is a bounded V2 adaptation to V1 fetch's implicit redirects.

## Deliberate differences and source quirks

- The `defaultFormat` command preference exists in metadata but **is not used by own `index.tsx`**. The real locked `@raycast/utils` 2.3.0 `useForm` returns `null` for the initial format. The selected host has no `storeValue` implementation. V2 therefore opens with no selected format and persists no invented format store or V1 migration. It keeps the session draft. Raycast's native UI persistence is outside this own-host evidence.
- Source `getQRCodePath` stringifies the **full regex match array**, including commas and later schemes on multiline content; V2 preserves that finite behavior before Windows sanitization and 80 UTF-16 units. Cutting a surrogate pair drops its final high surrogate; trailing dots/spaces in the derived stem are removed. Filesystem/invalid path failures are visible. Unpaired JSON surrogates are rejected by the pinned SDK/serde transport; scalar Unicode, including supplementary characters, is UTF-8 encoded. No lossy payload conversion is performed.
- V1 overwrites existing filenames. V2 atomically publishes with **no overwrite** using a same-directory hard link from a flushed private staging file. Existing files/symlinks/collisions are preserved and reported. Filesystems that cannot hard-link fail visibly; no non-atomic fallback is attempted. `dirs::download_dir` resolves platform Downloads (Windows Known Folder, standard macOS Downloads). Directory symlink components are refused. Tests inject disposable directories, never choose a Downloads path through renderer payloads. Staging cleanup checks content plus file identity and only unlinks its original staging name; published or replaced names are not owned by that guard.
- PNG is 512×512 RGBA, four-module quiet zone, ECC M, automatic normal version/segmentation/mask, validated foreground, transparent black light pixels or white for `png-bg`. SVG is 1536×1536, four-module quiet zone, transparent light modules. Rust `qrcode` 0.14.1 and real `png` replace JS `qrcode` 1.5.4. Seven of eight recorded vectors match source matrices **and decoded pixels**; repeated alphanumeric vector 6 has a different valid matrix. This is finite representation evidence, not universal PNG/SVG parity. PNG compression and SVG path serialization differ. Independent `rqrr` decoding verifies payload/ECC, including capacity boundaries; resvg independently rasterizes SVG before another decoder roundtrip.
- V1 appends `?raycast-height=350` **after base64**. The actual host Detail parser passes that suffix through to image `src`, corrupting data. V2 emits a query-free generated data URL; the actual Detail renderer applies its normal responsive height (`70vh`). No untrusted input becomes Markdown or an image URL.
- V1's own host `Clipboard.copy({file})` loads raster pixels but writes a **path string for SVG**. V2 never reports a copied SVG on that basis. The selected process `host.clipboard.writeImage {file}` genuinely consumes PNG pixels and returns `{ok:true}`. **SVG Copy is visibly unsupported**; Save SVG is supported. The generic `writeFiles` service is not used: it returns boolean success and keeps OS file references without a durable lifetime contract. A future file-clipboard prerequisite would need a structured receipt and host-owned file lifetime before enabling SVG Copy. No invented RPC or copied path masquerades as an image/file.

## Effects, cancellation and resources

SDK stages adapter state on every operation; filesystem/network/clipboard effects cannot be rolled back by discarding it. Bounded session-owned checkpoints in `Adapter` are therefore authoritative, independent of staged `Value`. Before every awaited acquisition/network/clipboard/open effect the attempt/outcome is recorded. A canceled generation cannot silently restart on refresh, repeated submission or process restart. A fresh process starts with fresh sessions and never imports/replays effects. If no output was prepared before interruption, edit the input or reopen to explicitly request generation again. A completed ordinary generation failure is distinct: it issues a new opaque Retry Generation token. Refresh, queued ordinary Generate submissions and consumed/expired retry tokens never restart that failed request. The recovery token is consumed before awaiting; canceling recovery does not issue another token. Actual draft changes clear the obsolete attempt and retry token, so an explicit A → edit B (including invalid/empty B) → edit A → Generate can prepare A again without a blank/stuck form. Field changes alone never generate.

Prepared output has an opaque UUID. Old/discarded UUID actions fail; editing fields discards old prepared actions. Repeated acknowledged Save/Copy is a no-op. Explicit Retry Copy/Save uses the **same prepared bytes**, never another shortener request or QR generation. Retry Open uses the acknowledged saved path and never rewrites the file. A failed/unacknowledged open is reported separately from a successful save. Only the exact structured `{ok:true}` / `{ok:true,message:string}` platform responses count as success; null, booleans, errors and contradictory/unknown acknowledgement keys do not.

No detached tasks/helpers/polling are created by production code. Work runs in the SDK operation scope; cancellation drops network futures. Sessions and owned acknowledged files are cleaned on dispose, shutdown, EOF, and adapter drop. Clipboard PNGs are removed after the host positively acknowledges consuming pixels. An uncertain/canceled clipboard call may still be reading its path in the selected host's blocking platform code, so its file is conservatively retained in the **private data-dir `clipboard-staging`**, including after dispose/EOF. This is **not a durable lease** or an automatically purged cache. For each profile staging directory, a process-global mutex serializes the directory scan and file creation across all adapters/sessions. Admission counts existing files (including leftovers from prior runs) plus the incoming file, and refuses creation beyond 128 files / 256 MiB. Saturation requires manual review. Unknown/unacknowledged files are never removed for capacity. This single-process contract does not coordinate independent processes or hostile external directory mutations. No arbitrary directory deletion or reference-lifetime claim is made. A crashed writer can also leave an owned staging artifact for manual review; no global sweep runs.

Limits: 128 sessions; 16,384 UTF-8 bytes per input, 32 KiB aggregate form, 2048 bytes per UTM field, QR version 1..40 at ECC M (finite tests reach 5596 numeric / 3391 alphanumeric / 2331 byte characters), max 2 MiB per file, 512 raster dimensions, at most 177² modules and roughly 16k SVG module paths. Capacity/resource failures are explicit. Manifest/SDK concurrency agree on `serialize`; views use `refreshMs:null`. Native grants are not an OS sandbox. No profile/config/ledger access or migration is requested.

## Unapplied host prerequisites

[host-prerequisites.patch](integration/host-prerequisites.patch) applies only against the selected committed baseline; it has **not** been applied to Tezbar. Proposed complete files and actual before/after tests are included.

1. Every submit-form action needs the current form map (including secondary/tertiary actions).
2. Detail's primary button needs a wrapper so a MouseEvent cannot become the action ID.
3. Opt-in `__tezbarDraftRevision` plus full maps and per-field local revisions plus a response gate preserve newer edits when old queued responses arrive. The outer renderer rejects older Form/Detail views; form instances are keyed by session to isolate drafts. Unversioned forms retain their existing behavior. QR rejects missing/incomplete/stale submission maps rather than running an effect from stale values.
4. Registry icon/resource paths need the resolved version directory. Preference paths remain at the canonical package root. Actual extracted before/after Rust registry tests exercise resource lookup and unchanged preference root.

SVG clipboard is a separate explicit format limitation, not covered by these patches. Full host integration, actual GUI focus, platform clipboard, system Downloads permission/redirect behavior and two-OS acceptance still need parent-owned verification.

## Build, test and private package

See [VALIDATION.md](VALIDATION.md) for recorded results and [ci/reproduce.sh](ci/reproduce.sh) for isolated source reproduction. `ci/native.yml` is a reviewable inactive template for native ARM64/macOS and Windows MSVC execution plus Rust 1.85 gates. It is intentionally not a root workflow. Windows native pipe smoke is `qa/windows-smoke.ps1`.

```sh
cargo test --locked --all-targets --manifest-path v2/Cargo.toml
cargo clippy --locked --all-targets --manifest-path v2/Cargo.toml -- -D warnings
cargo build --locked --release --manifest-path v2/Cargo.toml
python3 v2/qa/process-smoke.py v2/target/release/tezbar-qrcode-generator
python3 v2/tools/package.py v2/target/release/tezbar-qrcode-generator --target aarch64-apple-darwin
```

`--protocol-info` / `--self-test` are SDK metadata entrypoints, **not domain QA**. `tools/package.py` validates the native header, produces deterministic ZIP member order/timestamps/permissions and exact file inventory, validates/extracts it privately, and records digest metadata from built bytes. Source `artifacts:{}` is accepted by the actual manifest validator; acquisition correctly fails without verified target pins. No fake release URLs, artifact pins or host catalog edits exist. Ignored `out/` contains private QA ZIPs, extracted packages and source archives; `qa/artifact-evidence.json` records their actual digests. Packages contain only native executable, schema-3 manifest, exact resources, grants/licenses and NOTICE.

## Original icons and licenses

| Resource | Bytes | SHA256 |
|---|---:|---|
| command-icon.png | 184611 | `528b0186ce805553de1040a7fbaf39e9ed340d10a80bac89f53b309417491936` |
| generate-qr-icon.png | 13260 | `5c6dd2cd886df20d0f2d9127af4b8d0d35647552670acbbf17138f01b2355be9` |
| clipboard-icon.png | 11103 | `6088d80fcc0029dbd836581fcc53717e3db6a4a496c3c58e0b6d198f7c3d9946` |
| selection-icon.png | 9322 | `57e4dc96dbfd272e1ed989b36309ec19423c0914a9857eded97e5210d36c0c5c` |

Original author Melvynx and actual contributors are retained in NOTICE and the immutable upstream manifest. Real upstream, own and SDK grants and resolved dependency licenses are included without fabricated notices.

## QA filesystem incident

The **first** source-oracle run incorrectly allowed the locked QR library's `toFile` past the TSX filesystem capture. It wrote `~/Downloads/qrcode-hello.png` and three temporary PNGs. The preimage of that Downloads name was not captured, so previous absence cannot be proven. The boundary was corrected to buffer-only capture before later runs. Round 2 adds fake home/temp paths in the source require seam, a fail-closed dependency allowlist, immutable guards on the real QR file APIs, and an execution-time fs producer tripwire. A regression deliberately misroutes real `toFile`/`toFileStream`, checks callback/stream refusal before effects, and verifies captured Promise/callback `toFile` calls and SVG writes. Genuine `toBuffer` remains the output oracle; only the final fixture write under this clone reaches fs. This is a finite QA boundary, not an OS sandbox. No live desktop clipboard/open or external shortener was invoked. [qa/INCIDENT.json](qa/INCIDENT.json) lists exact paths, hashes and timestamps. They remain untouched under the no-deletion instruction, as does the parent-owned outside-clone incident record. No rollback or absence of earlier real writes is claimed. This exception prevents claiming that all QA writes were confined to disposable directories.
