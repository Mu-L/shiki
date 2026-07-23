# Security Policy

## Supported Versions

Only the latest released version of shiki is supported with security fixes.
There's no long-term-support branch — please upgrade to the latest release
before reporting an issue.

## Reporting a Vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Instead, use one of these private channels:

1. **Preferred**: [Report a vulnerability](https://github.com/sazardev/shiki/security/advisories/new)
   via GitHub's private vulnerability reporting (Security tab → "Report a
   vulnerability"). This lets us discuss and fix the issue privately before
   any public disclosure.
2. Email **cerberusprogrammer@gmail.com** with details and, if possible, steps
   to reproduce.

We'll acknowledge reports as soon as possible and keep you updated as the
issue is investigated and fixed. Once a fix is released, we'll credit the
reporter (unless you'd prefer to stay anonymous) and, where relevant, request
a CVE.

## Scope

shiki is a local-first TUI application — it reads/writes files on disk and
talks to git remotes (HTTPS/SSH) configured by the user. Relevant security
concerns include (but aren't limited to):

- Path traversal or unsafe file handling in note/notebook operations
- Credential handling in git remote operations (`shiki_core::git`)
- Unsafe deserialization of config/frontmatter (YAML/TOML parsing)
- Anything that could execute arbitrary code from note content or config

General usability bugs, crashes on malformed input that don't have a security
implication, and missing features should go through the normal
[issue tracker](https://github.com/sazardev/shiki/issues) instead.
