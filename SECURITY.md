# Security Policy

## Supported versions

lanner is in early development. Security fixes target the latest `master`.

## Reporting a vulnerability

Please report security issues privately rather than opening a public issue.

- GitHub: use the repository's private vulnerability reporting (the Security tab,
  "Report a vulnerability").
- Or contact the maintainer through the address listed on the GitHub profile.

Include steps to reproduce, the affected version or commit, and the impact. You
can expect an acknowledgement within a few days.

## Scope notes

lanner spawns external processes (`wf-recorder`, and later `ffmpeg` and
`gifski`) using argument vectors rather than a shell, so selection geometry and
file paths are never interpreted by a shell. Reports about command construction,
file path handling, or the overlay input region are in scope.
