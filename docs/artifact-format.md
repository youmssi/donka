# Release artifact format

The release artifact is the contract between Donka Studio and Donka Runtime (ADR-001). Studio
writes it; the Runtime reads it. Changes are **additive only**, and the Runtime ships first when
the format changes.

## Layout

One zip per project and environment, stored at `<environment>/<project-key>` in the bucket (no
extension):

```
.config/project.json       release metadata and the tokens it accepts
person-score               a JDM decision graph, named by its key (no extension)
bureau/normalize           decisions in folders keep their path
```

An entry is named exactly by the decision key, so a decision node that calls `bureau/normalize`
and a client that evaluates `/api/projects/<project>/evaluate/bureau/normalize` both find it.

## `.config/project.json`

Current version: **2**.

```json
{
  "version": "2",
  "project": { "id": "…", "key": "credit-pme", "name": "Crédit PME" },
  "release": { "id": "…", "version": "1.4.0", "name": "Q3 limits", "status": "published" },
  "environment": { "id": "…", "key": "production", "name": "Production" },
  "accessTokenHashes": [
    {
      "id": "…",
      "environment": "production",
      "algorithm": "sha256",
      "hash": "d4d813b79f07c455e68458c955824329d902dd5f8e7b0c250fb3f05b3f68c840"
    }
  ]
}
```

| Field | Since | Meaning |
|---|---|---|
| `version` | 1 | Format version, a string. |
| `project` | 1 | `id`, `key`, `name` of the project. |
| `release` | 1 | `id`, `version` (semantic), `name`, `status` of the release. |
| `environment` | 1 | `id`, `key` (`staging` or `production`), `name` of the environment deployed to. |
| `accessTokens` | 1 | Plain tokens. **Studio never writes them**; the Runtime still accepts them so older artifacts keep working. |
| `accessTokenHashes` | 2 | The tokens issued for this environment, as hashes. |

Each `accessTokenHashes` entry:

| Field | Meaning |
|---|---|
| `id` | The token's id in Studio, to tell tokens apart without revealing them. |
| `environment` | The environment the token was issued for. |
| `algorithm` | `sha256`, the only one today. An entry naming another algorithm never matches. |
| `hash` | Lowercase hex SHA-256 of the token's UTF-8 bytes. |

## Token rules

- A token is a long random secret: at least 32 random bytes, shown once when Studio issues it
  and never stored or written anywhere in plain form. Because it is random, an unsalted SHA-256
  is enough to keep it out of the artifact; the Runtime compares digests in constant time.
- A client sends the token in the `X-Access-Token` header.
- An artifact deployed to an environment accepts only entries whose `environment` equals its
  `environment.key`: a token issued for staging is refused by the production Runtime, even if
  its hash were listed there.
- A malformed entry is skipped (and logged) by the Runtime; it never invalidates the rest of the
  file.

Test vector: the token `dnk_test_token` hashes to
`d4d813b79f07c455e68458c955824329d902dd5f8e7b0c250fb3f05b3f68c840`. The Runtime checks it in its
tests; Studio checks it too once it issues tokens (DNK-14).

## Connector nodes

A decision may call an outside service through a connector node (DNK-17): a JDM `customNode`
whose `content.kind` is `donka.connector`. The Runtime POSTs `body` to `url` and adds the JSON
answer to the node's output under `outputKey`; Studio answers with `mock` instead.

```json
{
  "id": "bureau", "name": "bureau", "type": "customNode", "position": { "x": 300, "y": 40 },
  "content": {
    "kind": "donka.connector",
    "config": {
      "preset": "bureau-score",
      "url": "https://api.bureau.example/v2/score",
      "auth": { "type": "header", "header": "X-Api-Key", "secret": "BUREAU_API_KEY" },
      "body": { "nationalId": "{{ applicant.nationalId }}" },
      "outputKey": "bureau",
      "timeoutMs": 3000,
      "retries": 1,
      "onError": "fallback",
      "fallback": { "score": null, "available": false },
      "mock": { "score": 712, "available": true }
    }
  }
}
```

| Field | Meaning |
|---|---|
| `preset` | `http` or `bureau-score`; informational. |
| `url` | `http://` or `https://`. |
| `auth` | `{ "type": "none" }`, `{ "type": "bearer", "secret" }` or `{ "type": "header", "header", "secret" }`. `secret` is a **name** (capital letters, digits, `_`, at most 64); the Runtime reads the value from `DONKA_SECRET_<NAME>`. |
| `body` | Any JSON; string values may be templates over the node's input. |
| `outputKey` | Letters, digits and `_`. |
| `timeoutMs`, `retries` | Optional; capped by the Runtime (`CONNECTORS__*`). |
| `onError` | `fail` (default) or `fallback`, which answers `fallback` when the service fails. |
| `mock` | The answer in Studio's simulator and test scenarios; required to simulate. |

Unknown fields are refused, so a secret value cannot ride along in the node. The node's trace
holds the mode, outcome, attempts, status and an error code, never a secret or the answer.

## History

| Version | Change |
|---|---|
| 1 | Format inherited from the GoRules agent: plain `accessTokens`. |
| 2 | `accessTokenHashes`, per environment (DNK-13). `accessTokens` no longer written. |
