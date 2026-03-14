# Security Policy

## Supported Versions

Only the latest release receives security updates.

| Version | Supported          |
| ------- | ------------------ |
| 0.8.x   | :white_check_mark: |
| < 0.8   | :x:                |

## Scope

ExphoraDB is a local-first desktop application. The following areas
are considered in scope for security reports:

- **P2P transport layer** — encryption handshake, key exchange,
  or data integrity issues in `p2pShare` / `p2pFetch`.
- **File parsers** — malicious JSON, CSV, SQLite, XML, or NDJSON
  files that cause crashes, panics, or unintended code execution.
- **Expression engine** — `expr.rs` inputs that escape the
  sandboxed evaluation context or cause denial of service.
- **View files (.exh)** — crafted `.exh` files that exploit
  deserialization to cause unintended behavior.

The following are **out of scope**:
- Issues requiring physical access to the user's machine.
- Bugs in third-party dependencies (report those upstream).
- UI/UX issues that do not have a security impact.

## Reporting a Vulnerability

**Do not open a public GitHub Issue for security vulnerabilities.**

To report a vulnerability, please open a
[GitHub Security Advisory](https://github.com/Nuulz/exphora_db/security/advisories/new)
in this repository. This keeps the report private until a fix is released.

Include in your report:
- A clear description of the vulnerability.
- Steps to reproduce (minimal example if possible).
- The version of ExphoraDB affected.
- Potential impact in your assessment.

## Response Timeline

| Stage | Timeframe |
| :--- | :--- |
| Initial acknowledgement | Within 72 hours |
| Status update | Within 7 days |
| Patch release (if confirmed) | Within 30 days |

## Disclosure Policy

Once a fix is released, the vulnerability will be disclosed publicly
via a GitHub Security Advisory. Credit will be given to the reporter
unless they prefer to remain anonymous.
