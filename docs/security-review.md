# Security review — November 2026

Review of Magpie against PRD §11.7 (security requirements) and §16.3 (security testing), done before the 1.0 hardening work. Findings marked **Fixed** are addressed in the same pull request as this document.

## Findings

| # | Severity | Finding | Status |
|---|---|---|---|
| 1 | **High** | Opening a bookmark passed the stored URL straight to the Windows shell. Every way of *adding* a link only accepts http(s), but the database itself can come from outside the app: a backup restored from a file. A crafted backup with `file:///…/calc.exe` or an `ms-msdt:` link would run that handler when the bookmark was opened. | **Fixed.** `bookmarks::openable_url` re-validates every URL (http/https only, re-serialised by the URL parser) before Open and Open in private window. Restoring a backup that contains non-web links is refused. |
| 2 | Medium | `export_library` wrote to any path the page supplied. With script injection in the WebView, this could overwrite arbitrary files, including Magpie's own database. | **Fixed.** Exports must be absolute file paths with the format's extension (`.json`, `.html`/`.htm`, `.csv`), in an existing folder outside Magpie's data folder. Restore-from-file only accepts `.db` files. |
| 3 | Low | The page was granted `opener:default` (open URLs or reveal files from page script), but the UI never uses it; Rust does all opening after the security checks. | **Fixed.** Permission removed, so page script can't bypass the warning/block flow. |
| 4 | Low | The Content Security Policy didn't restrict `base-uri`, `form-action`, `object-src` or `frame-ancestors`. | **Fixed.** Added `object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'`. |
| 5 | Info | Dependencies weren't audited automatically. npm production dependencies: 0 known vulnerabilities. | **Fixed.** CI now runs `cargo audit` (both lock files) and `npm audit --omit=dev` on every pull request. |
| 6 | Info | Installers aren't code-signed (PRD §11.7). | Open. Needs a code-signing certificate; documented in the README and release notes. |

## What was checked and is fine

- **Input URLs (§11.7, §16.3 malformed / credentials / IP / punycode / look-alikes):** `urls::parse` accepts only http(s) with a host. Local heuristics flag embedded credentials, IP hosts, look-alike and punycode domains, program downloads and HTTP. Covered by `security::tests` and `urls::tests`.
- **SSRF through previews (§16.3):** public addresses only, checked after DNS resolution with the connection pinned to the checked address (DNS rebinding). Redirects are followed manually and re-checked, response sizes are capped, and only raster images are cached under SHA-256 names (SVG refused). Tests cover private, link-local, CGNAT, cloud-metadata and IPv4-mapped addresses, plus decimal/hex/octal loopback encodings.
- **Malicious HTML metadata:** titles and descriptions are decoded and cleaned as text, and React renders them as text. There's no `innerHTML`, `eval` or dynamic code in the UI.
- **Untrusted images:** served only from the preview cache through the asset protocol (scope `$APPDATA/previews/*`). The UI also checks file names against `^[0-9a-f]{64}\.(png|jpg|gif|webp|ico|avif)$`.
- **API keys (§11.7, §16.3 key leakage):** stored only in Windows Credential Manager, never returned to the UI or written to SQLite, and scrubbed from error messages (`ai::tests::errors_never_contain_the_key`, `scanners::tests::errors_never_contain_request_details`).
- **Privacy (§11.4):** AI requests contain URL (without query string or fragment), title, description and category names only. Scanner submissions need consent and drop credentials, fragments and (optionally) tracking or token parameters. Trusted domains are never sent.
- **Command injection:** no shell is used. The private-window launcher passes the browser path from the registry and the URL as separate process arguments.
- **CSV export:** cells that would run as spreadsheet formulas are prefixed with `'`.
- **Database:** all multi-row writes use transactions; restores are validated (`quick_check`, schema version) and preceded by a snapshot.

## How to re-run

```powershell
cd crates/magpie-core; cargo test            # includes all security tests
cd ../../src-tauri; cargo test
npm audit --omit=dev
```
