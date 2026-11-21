# Licence keys (DNK-77)

| File | What it is |
| --- | --- |
| `licence.pub` | Donka's public key, built into every delivered Studio. Its signing key never leaves Donka. Until Donka's key pair exists, it holds a placeholder nobody can sign for, and `scripts/check-licence-key.sh` refuses to release with it. |
| `development.pub` | The development public key, trusted only by builds with the `development-licence-key` feature (local development and CI). |
| `development.key` | The development signing key, committed on purpose so CI and developers can issue test licences (`scripts/dev-licence.sh`). A delivered Studio never trusts it. |

To set Donka's key: run `donka-licence keygen <dir>` on the machine that will issue licences,
keep `licence.key` offline, and commit `licence.pub` here.
