# Contributing

Thank you for contributing to HORCRUX. This repository is public and maintained
with a production-style workflow: **nothing reaches `main` unless CI verifies it.**

## Contribution flow

```
Fork Repository
       ↓
Create Feature Branch
       ↓
Write Tests
       ↓
Run Checks Locally
       ↓
Submit Pull Request (targeting main)
       ↓
CI Verifies (required before merge)
       ↓
Maintainer Reviews & Merges
```

## Before you open a pull request

Verify your changes locally. All commands below must pass:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

## Continuous integration

Every pull request targeting `main` and every push to `main` runs the `CI`
workflow. It checks:

| Check            | Purpose                                            |
| ---------------- | -------------------------------------------------- |
| `fmt`            | Code formatting (`cargo fmt --check`)              |
| `clippy`         | Lints, warnings denied                             |
| `test`           | Unit + integration tests                           |
| `build-release`  | Release build succeeds                             |
| `security-audit` | Dependency vulnerability audit (`cargo audit`)     |
| `gitleaks`       | Secret-leak scan                                   |
| `verify`         | Aggregate gate — must be green to merge            |

`CI / verify` is the single required status check. If any check fails, the pull
request cannot be merged. Keep your branch up to date with `main` so the
required checks run against the latest code.

## Branch protection on `main`

`main` is protected. Direct pushes are blocked; all changes must land through a
pull request that passes required checks. Repo admins configure this in the
GitHub web UI under **Settings → Branches → Add rule** for `main`:

- **Require a pull request before merging** — require 1 approving review, and
  check *Dismiss stale pull request approvals when new commits are pushed*.
- **Require status checks to pass before merging** — select `CI / verify` and
  check *Require branches to be up to date before merging*.
- **Require linear history** — keep history clean (squash/rebase only).
- **Require conversation resolution** — all comment threads must be resolved.
- **Do not allow bypassing the above settings** — even admins must follow the
  rules.
- **Block force pushes** and **block deletions**.

Keep the rules in sync with the required checks listed in this file so the
`CI / verify` gate stays the single source of truth for merging.

## Release process

Maintainers create a release by pushing a version tag (`v1.0.0`). The `Release`
workflow builds the binary, attaches it to a GitHub release, and publishes
generated release notes.

## Guidelines

- Code must be formatted and lint-clean.
- Tests must pass — add tests for new functionality.
- Keep the documentation updated (README, module docs).
- Explain the security considerations of any change.
- This is cryptographic software: prefer conservative, audited primitives and
  do not introduce secrets, keys, or test data that resembles real ones.
