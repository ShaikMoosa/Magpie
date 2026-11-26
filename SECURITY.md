# Security policy

Magpie handles your bookmarks, API keys and links that may be malicious, so security reports are very welcome.

## Reporting a vulnerability

Please **don't open a public issue** for security problems. Instead, use GitHub's private reporting: go to the repository's **Security** tab and choose **Report a vulnerability**.

Include what you found, how to reproduce it, and the Magpie version (Settings → About). You'll get a reply as soon as possible, and a fix will be released before details are made public.

## Supported versions

Only the latest release receives security fixes.

## Scope

Particularly interesting areas:

- Preview fetching (SSRF, private-network access, redirect handling, oversized or malicious responses)
- URL security checks and the open/block flow
- API key handling (Windows Credential Manager, leaks into logs, errors or the UI)
- Backup restore and import parsing (malformed files)
- Tauri capabilities, Content Security Policy and IPC commands
