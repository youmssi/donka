# Rules sync

How a CI pipeline pulls a project's release artifact from Studio (DNK-20). The Donka CLI
(`donka pull`, [youmssi/donka-cli](https://github.com/youmssi/donka-cli), DNK-21) speaks this API
and ships GitHub, GitLab and Azure templates around it; any pipeline can also call it directly.
Changes are **additive only**, and donka-cli's tests follow this page.

## Token

A **CI token** is issued by a project owner in the project's **Settings** (`dnk_ci_…`). It is
shown once, kept only as a SHA-256, read-only, and reaches only its own project. Studio records
when a pipeline last used it; revoking it refuses the next request.

## Resolve

```
POST /api/v1/rules-sync
Authorization: Bearer dnk_ci_…
Content-Type: application/json

{ "deployments": [ { "project": "credit-pme", "target": "env:production",
                     "alias": "prod", "current": { "commitId": "…", "releaseId": "…" } } ] }
```

- `project`: the project's key or id.
- `target` (default `main`):

  | Target | Resolves to |
  |---|---|
  | `main` | The project's newest release. |
  | `commit:<id>` | The release with that id (releases are Donka's immutable snapshots). |
  | `release:<version>` | The release with that semantic version, `1.4.0` or `v1.4.0`. |
  | `env:<staging\|production>` | What is live in that environment, tokens included. |
  | `branch:<…>` | Not supported: Donka has no branches (`UNSUPPORTED_TARGET`). |

- `current`: ids the pipeline already holds. A match answers `no_change`. Send back `commit.id`
  from the previous answer: for a release it is the release id; for an environment it is the
  deployment id, which changes when the environment's Runtime tokens change.
- At most 50 deployments per request; each is answered on its own, in order.

Answer `200`:

```json
{
  "nextPollAt": null,
  "deployments": [{
    "project": { "id": "…", "key": "credit-pme" },
    "target": "env:production",
    "alias": "prod",
    "action": "load",
    "commit": { "id": "<deployment id>", "branchId": null, "branchName": null },
    "release": { "id": "…", "name": "<notes>", "version": "1.4.0", "semanticVersion": "1.4.0" },
    "environment": { "id": "…", "key": "production", "name": "Production" },
    "artifact": { "url": "/rules-sync/artifacts/<project>/deployments/<id>", "sha256": "…" }
  }]
}
```

| `action` | Meaning |
|---|---|
| `load` | Download `artifact.url` (relative to `/api/v1`) with the same token and check `sha256`. |
| `no_change` | The pipeline already holds it; no `artifact`. |
| `no_release` | No release yet (`main`), or nothing live in the environment (`env:`). |
| `no_access` | The token cannot reach this project, or it does not exist; `project` is `null`. |
| `error` | `code` says why: `INVALID_TARGET`, `UNSUPPORTED_TARGET` or `RELEASE_NOT_FOUND`. |

Other answers: `401 INVALID_TOKEN` (missing, unknown or revoked token), `400 INVALID_REQUEST`
(not a request, or more than 50 deployments).

## Download

```
GET /api/v1/rules-sync/artifacts/<project id>/releases/<release id>
GET /api/v1/rules-sync/artifacts/<project id>/deployments/<deployment id>
Authorization: Bearer dnk_ci_…
```

`200` with the zip (`application/zip`), or `404 RELEASE_NOT_FOUND` for another project's
artifact or one never published. Artifacts are rebuilt from Studio's database and are
byte-for-byte reproducible, so the download matches the `sha256` given when resolving.

- An **environment** artifact is what that environment's Runtime reads
  ([`artifact-format.md`](artifact-format.md)), with the hashes of its Runtime tokens.
- A **release** artifact has no `environment` and an empty `accessTokenHashes`: a Runtime given
  it refuses every request. It is for pipelines that test or embed the rules themselves.
