This release fixes model downloads stalling at the start or partway through (Mac and Android).

### More reliable model downloads

- Connections try IPv4 before IPv6 on dual-stack networks so unreachable IPv6 addresses do not burn through the connect timeout.
- Stalled transfers (averaging under 12 KB/s for 12 seconds) or dropped streams automatically reconnect and resume via HTTP Range; retries reset whenever real progress is made so large models survive flaky mobile networks.
- If `huggingface.co` cannot be reached or stalls, the downloader automatically falls back to `hf-mirror.com` while still verifying every file against its built-in SHA-256 checksum.

### Clearer download progress

- Fixed 0 bytes showing as `1 kB` before the first chunk arrives, and fixed resumed bundle downloads jumping straight to 100%.
- The download dialog and Android notification now show live speed along with connecting, reconnecting, and verifying states.
