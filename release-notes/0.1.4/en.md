This release adds a mainland China mirror on ModelScope for every model file and picks the fastest mirror automatically (Mac and Android).

### Mainland China mirror for all models

- All 14 downloadable models — including the 857 MB Ultra model previously only on GitHub Releases — are now mirrored on ModelScope (`aiwren/FleetingGleam-models`) for fast direct downloads in mainland China without a VPN.
- Before starting each file, the downloader races a 1-byte Range probe across ModelScope, the upstream host (Hugging Face / GitHub), and `hf-mirror.com`, immediately picking the fastest valid response (filtering out HTML error pages).
- Every downloaded file is still verified against its built-in SHA-256 checksum before use.
