# Install and operate Donka

[Français](install.fr.md)

This guide installs Donka on one server with Docker Compose and covers its day-to-day
operation: the smoke test after each deploy, backups, upgrades and the rotation of secrets.

One installation serves one organisation. It runs five containers:

| Service | What it does | Port (host) |
| --- | --- | --- |
| `app` | Donka Studio: the web app and its API | `8080` |
| `runtime` | Donka Runtime serving the **staging** environment | `8090` |
| `postgres` | Studio's database (PostgreSQL 16) | `127.0.0.1:5432` |
| `minio` | Release storage (S3-compatible) | `127.0.0.1:9000`, console `127.0.0.1:9001` |
| `mailpit` | Catches outgoing email until you set a mail server | `127.0.0.1:8025` |

PostgreSQL, MinIO and Mailpit listen on the server itself only. Expose Studio and the Runtime
through your reverse proxy (TLS), not directly.

## 1. Install

You need Docker Engine with the Compose plugin (2.24 or later), git, about 10 GB of free disk
for the first build, and outbound access to GitHub and the container registries.

```bash
git clone https://github.com/youmssi/donka.git && cd donka
docker compose --profile full up -d
```

The first start builds Studio and the Runtime from source (15 to 30 minutes). Then:

1. Open http://localhost:8080 (or your server's address).
2. Find the one-time link to choose the first administrator's password:

   ```bash
   docker compose logs app | grep setup-password
   ```

   The administrator is `admin@donka.local` unless you set `DONKA_BOOTSTRAP_ADMIN_EMAIL`.
3. Run the smoke test (section 2).

Every setting has a default that works on one machine. **Before real use, set your own values**
in a `.env` file next to `docker-compose.yml` (keep it out of git; it holds secrets):

```bash
# .env
DONKA_PUBLIC_URL=https://donka.bank.example     # where people reach Studio
DONKA_COOKIE_SECURE=true                        # Studio is served over HTTPS
DONKA_BOOTSTRAP_ADMIN_EMAIL=you@bank.example    # first administrator, on an empty database
DONKA_POSTGRES_PASSWORD=<generated>             # openssl rand -hex 24
DONKA_STORAGE_ACCESS_KEY_ID=donka
DONKA_STORAGE_SECRET_ACCESS_KEY=<generated>     # openssl rand -hex 24
DONKA_DECISION_LOG_KEY=<generated>              # openssl rand -base64 32
DONKA_SMTP_URL=smtp://user:pass@mail.bank.example:587?tls=required
DONKA_SMTP_FROM=Donka <donka@bank.example>
```

Behind a reverse proxy, also set `DONKA_TRUSTED_PROXIES` to its address (or the Docker network,
e.g. `172.16.0.0/12`): sign-in and password reset are limited per client address, and without
it every client shares the proxy's.

Set these before the first start: PostgreSQL and MinIO take their passwords when their data
is first created (section 5 explains how to change them later). Every other Studio setting in
[.env.example](../.env.example) can be added to the same file.

The Runtime reads extra settings from an optional `runtime.env` file, for example to send its
decisions to Studio's decision log (issue the token under **Runtimes** in Studio):

```bash
# runtime.env
DECISION_LOG__URL=http://app:8080/api/v1/decision-log/records
DECISION_LOG__TOKEN=dnk_log_...
```

Its settings are documented in the Runtime's
[configuration guide](https://github.com/youmssi/donka-runtime/blob/develop/docs/configuration.md).
For a production Runtime, run a second one with `PROVIDER__PREFIX: production/`.

## 2. Smoke test after each deploy

`scripts/smoke.sh` checks that Studio answers and reaches its database, that the decision
engine simulates, and that the Runtime evaluates a release published by this Studio.

```bash
STUDIO_URL=http://localhost:8080 RUNTIME_URL=http://localhost:8090 \
DONKA_SMOKE_EMAIL=admin@donka.local DONKA_SMOKE_PASSWORD='...' \
scripts/smoke.sh
```

```
Studio http://localhost:8080
  ok    health
  ok    ready (database reachable)
  ok    version 1.4.0
  ok    simulate (95µs)
Runtime http://localhost:8090
  ok    health
  ok    evaluate donka-smoke/smoke/double on staging
smoke: Studio and the Runtime are working
```

The account must be allowed to create projects. The evaluation uses a project of its own,
`donka-smoke`: the first run creates it with one small decision and deploys it to staging;
later runs reuse it. Each run issues a Runtime token and revokes it before it exits. A failed
check prints `FAIL` with the reason and exits with a non-zero status, so the script can gate a
deployment pipeline.

| Variable | Default | Meaning |
| --- | --- | --- |
| `STUDIO_URL`, `RUNTIME_URL` | required | Where Studio and the Runtime answer |
| `DONKA_SMOKE_EMAIL`, `DONKA_SMOKE_PASSWORD` | required | The account the script signs in with |
| `DONKA_SMOKE_PROJECT` | `donka-smoke` | Key of the smoke project |
| `DONKA_SMOKE_ENVIRONMENT` | `staging` | The environment the Runtime at `RUNTIME_URL` serves |
| `DONKA_SMOKE_WAIT_SECONDS` | `90` | How long to wait for the Runtime to pick up a deployment |

To check a production Runtime, set `DONKA_SMOKE_ENVIRONMENT=production`. Production
deployments need an approval: approve the smoke project's release once in Studio.

## 3. Back up

Back up three things, together: the database, the release bucket and the secrets.

```bash
mkdir -p backups
stamp=$(date +%Y%m%d-%H%M)

# The database: projects, decisions, releases, approvals, audit and decision log.
docker compose exec -T postgres pg_dump -U donka -Fc donka > "backups/donka-$stamp.dump"

# The release bucket: what the Runtimes serve.
docker compose run --rm -v "$PWD/backups:/backups" --entrypoint sh minio-init -c \
  "mc alias set local http://minio:9000 \$AWS_ACCESS_KEY_ID \$AWS_SECRET_ACCESS_KEY >/dev/null &&
   mc mirror --overwrite local/donka-releases /backups/releases-$stamp"
```

Keep `.env` and `runtime.env` in your secret store. **Without `DONKA_DECISION_LOG_KEY` (and,
after a rotation, the keys in `DONKA_DECISION_LOG_PREVIOUS_KEYS`) the decision log cannot be
read**, even from a backup.

Copy the backups off the server and test a restore regularly. To restore, onto a stopped Studio:

```bash
docker compose stop app runtime
docker compose exec -T postgres pg_restore -U donka -d donka --clean --if-exists < backups/donka-<stamp>.dump
docker compose run --rm -v "$PWD/backups:/backups" --entrypoint sh minio-init -c \
  "mc alias set local http://minio:9000 \$AWS_ACCESS_KEY_ID \$AWS_SECRET_ACCESS_KEY >/dev/null &&
   mc mirror --overwrite /backups/releases-<stamp> local/donka-releases"
docker compose up -d app runtime
```

## 4. Upgrade

1. Read the release notes ([releases](https://github.com/youmssi/donka/releases)). When a release changes the artifact
   format or the Runtime's API, upgrade the Runtime first.
2. Back up (section 3). Database migrations only go forward: the way back is the backup.
3. Fetch the new version and rebuild:

   ```bash
   git fetch --tags && git checkout v1.5.0
   DONKA_VERSION=1.5.0 docker compose --profile full up -d --build
   ```

   To pin the Runtime to a release, set
   `DONKA_RUNTIME_SOURCE=https://github.com/youmssi/donka-runtime.git#v1.5.0` in `.env`.
   Studio applies pending migrations when it starts (`DONKA_DB_MIGRATE=true`).
4. Check it: `curl -s localhost:8080/api/v1/version`, then the smoke test (section 2).

## 5. Rotate secrets

Plan a short maintenance window for the passwords: the services restart.

**Database password**

```bash
docker compose exec postgres psql -U donka -c "ALTER USER donka PASSWORD 'new-password'"
# set DONKA_POSTGRES_PASSWORD=new-password in .env, then:
docker compose up -d app
```

**Storage keys (MinIO)**: MinIO reads its root credentials when it starts. Set the new
`DONKA_STORAGE_ACCESS_KEY_ID` and `DONKA_STORAGE_SECRET_ACCESS_KEY` in `.env`, then restart
every service that uses them:

```bash
docker compose --profile full up -d minio minio-init app runtime
```

**Mail server credentials**: change `DONKA_SMTP_URL` in `.env`, then `docker compose up -d app`.

**Runtime, CI and decision-log tokens** are issued in Studio. Issue a new token, give it to the
system that uses it, check that system works, then revoke the old token. Each step is in the
audit log.

**Decision-log key**: records are encrypted with `DONKA_DECISION_LOG_KEY`. Replace it when it
may have leaked, when someone who knew it leaves, or if you started with the default key from
`docker-compose.yml`. Each record names the key it was sealed with, so nothing is lost:

1. Generate a new key: `openssl rand -base64 32`.
2. In `.env`, move the current value to `DONKA_DECISION_LOG_PREVIOUS_KEYS` (several keys are
   separated by commas) and set `DONKA_DECISION_LOG_KEY` to the new key. Do not drop the old
   value: records sealed with it could no longer be read.
3. `docker compose up -d app`. Studio seals new records with the new key and still reads the
   old ones with the previous key; its log lists the keys still in use.
4. Re-seal the old records with the new key:

   ```bash
   docker compose exec app donka-app decision-log reseal
   ```

   It works in batches of 500, each recorded in the project's audit log (*Decision records
   re-sealed*). It can be stopped and run again: it carries on with the records left. When it
   names a key that is missing, add that key to `DONKA_DECISION_LOG_PREVIOUS_KEYS` first.
5. When it says no record uses the old key any more, remove it from
   `DONKA_DECISION_LOG_PREVIOUS_KEYS` and run `docker compose up -d app` again.

Backups taken before the re-seal still hold records sealed with the old key: keep the old key
with those backups, in your secret store, until they expire. If the key leaked, also restrict
access to the database and its backups: the key alone does not give access to records.

**User passwords**: people change theirs with *Forgot your password?* on the sign-in page.

## Troubleshooting

| Symptom | Look at |
| --- | --- |
| Studio does not start | `docker compose logs app`: a missing or invalid setting is named, with an example |
| `/api/v1/ready` answers 503 | PostgreSQL: `docker compose ps postgres`, then `docker compose logs postgres` |
| A deployment stays *pending* | `docker compose logs app`: the publisher retries until the bucket answers |
| The Runtime answers 404 for a project | It serves one environment (`PROVIDER__PREFIX`); check the release is live there |
| No emails | Mailpit (http://localhost:8025) until `DONKA_SMTP_URL` points to your mail server |
