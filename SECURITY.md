# Security policy

## Supported versions
| Version | Supported |
|---|---|
| 0.2.x | ✅ |
| 0.1.x | ⚠️ (security fixes only) |

## Reporting a vulnerability
**Do not open a public issue.** Use GitHub's private reporting:
**Security › Report a vulnerability** (https://github.com/kakashi3lite/firmgap/security/advisories/new).

If private reporting is unavailable, email <swanandtanavade100@gmail.com> with "firmgap security" in
the subject. Prefer the private advisory for anything sensitive; email is the fallback.

Please include affected version, reproduction steps or scenario file, and impact. You can expect:
- an acknowledgement within **3 business days**;
- an assessment within **10 business days**;
- a fix or mitigation for confirmed issues within **30 days** (critical: faster), followed by a
  coordinated advisory crediting you unless you prefer otherwise.

## Scope
In scope: the `firmgap` crate and CLI, `web/index.html`, CI workflows, and release artifacts.
That includes **result-integrity issues**: any input that silently produces wrong economics.
Out of scope: correctness of user-supplied tariffs or permits; social engineering of the maintainer.

## Design commitments
No `unsafe`, zero runtime dependencies, no network or process access, sanitized errors, SHA-pinned
CI. See `docs/THREAT_MODEL.md`.
