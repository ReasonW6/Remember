# Code signing policy

## Status and distribution

Remember is distributed from the project's [GitHub Releases](https://github.com/ReasonW6/Remember/releases) page.

The SignPath Foundation application is currently pending. Existing releases are unsigned. If the application is accepted, future Windows release executables will be signed under this policy.

Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

Official release binaries must be produced by the repository's GitHub Actions workflow from the corresponding source revision. Each signing request requires manual approval. Every distributed artifact, including an unsigned release candidate, receives a SHA-256 checksum. When an executable is signed, its published checksum must be regenerated from the final signed bytes.

## Team roles

- Committers and reviewers: [ReasonW6](https://github.com/ReasonW6)
- Approvers: [ReasonW6](https://github.com/ReasonW6)

## Privacy

Remember stores recordings locally. This program will not transfer any information to other networked systems unless specifically requested by the user or the person installing or operating it.

## Security

The maintainer must use multi-factor authentication for both GitHub and SignPath access. Signed artifacts must be built from the source code and build scripts in this repository and must satisfy the SignPath Foundation Code of Conduct.
