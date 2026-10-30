# Install and operate Donka

[Français](install.fr.md)

This guide installs Donka on one server with Docker Compose and covers its day-to-day
operation: the smoke test after each deploy, backups, upgrades and the rotation of secrets.

One installation serves one organisation. It runs four containers from Donka's published
images:

| Service | What it does | Port (host) |
| --- | --- | --- |
| `app` | Donka Studio: the web app and its API | `8080` |
| `runtime` | Donka Runtime serving the **staging** environment | `8090` |
| `postgres` | Studio's database (PostgreSQL 16) | none |
| `mailpit` | Catches outgoing email until you set a mail server | `127.0.0.1:8025` |

Studio publishes releases to the `releases` volume, which the staging Runtime serves and reloads
when a deployment changes it; Studio wires the Runtime to its decision log at first start
(the token is in the `wiring` volume). Expose Studio and the Runtime through your reverse proxy
(TLS), not directly. To build Donka from source instead, see [CONTRIBUTING.md](../CONTRIBUTING.md).

## 1. Install

You need a server with Docker Engine and the Compose plugin (2.24 or later), and access to
GitHub Container Registry. Studio's image is private: log in with the read token Donka gives
you (see *Published images* below). Nothing else is installed on the server.

```bash
echo "$DONKA_REGISTRY_TOKEN" | docker login ghcr.io -u <your GitHub user> --password-stdin
mkdir donka && cd donka
docker run --rm -it -u "$(id -u):$(id -g)" -v "$PWD:/install" -w /install \
  ghcr.io/youmssi/donka-studio:latest donka-admin init
docker compose up -d
```

1. `donka-admin init` asks its questions, in English or French, each with a default: the
   language, the address people reach Studio at, the first administrator, what the
   installation is (production, staging or demo), who signs in with a second step, where
   releases are stored (this server's disk or an S3 store), the mail server, the language
   model that explains decisions, the licence file, internet access and, for staging and demo,
   unattended upgrades. It shows a summary before writing anything. Every answer can also be
   given as an option (`--public-url`, `--admin-email`, `--role`…, see `donka-admin --help`);
   with `--yes`, or without a terminal (no `-it`), init asks nothing and takes the defaults.
2. It writes, in the folder:

   | File | What it holds |
   | --- | --- |
   | `donka.yaml` | Your answers. No secret: it can go in version control |
   | `secrets/` | One file per secret, mounted read-only at `/run/secrets` in the containers |
   | `.env`, `runtime.env` | Studio's and the staging Runtime's settings, written from `donka.yaml`; they name the secret files (`*_FILE`), never a value |
   | `docker-compose.yml` | The services |

   Init generates the database password and the decision-log, second-step and person-index
   keys at random. The secrets you hold (the mail server's URL, the storage keys, the language
   model's key, the licence) are asked for once and written to `secrets/` without being shown.
   The folder is readable by you only. Running init again changes nothing while `donka.yaml`
   exists, and a secret file is never overwritten. Back up `donka.yaml` and `secrets/`
   together (section 3).
3. Find the one-time link to choose the first administrator's password; it is the only
   secret ever printed:

   ```bash
   docker compose logs app | grep setup-password
   ```

4. Open the link (port 8080) and choose the password. Signing in then opens the first-run
   assistant, in three steps: turn on two-step sign-in with an authenticator app (required:
   this account holds every right), name the organisation (Studio shows it in its header, its
   emails, the access reports and the audit exports; administrators rename it later in
   **Settings**), and choose the packs to start from, each becoming a project you own. The Get
   started checklist follows. Until an administrator finishes the assistant, the projects page
   asks them to. Then run the smoke test (section 2).

The defaults set up a first look on this machine: http://localhost:8080 and
`admin@donka.local`. `--studio-version` and `--runtime-version` pin other image tags;
`--registry` takes the images from your own mirror of `ghcr.io/youmssi`.

**The demo.** `donka-admin init --demo` prepares the same installation with a project ready to
call. At the first start, Studio imports the *Retail credit* pack, releases it and deploys it to
staging, and a fake credit bureau answers its connector (`demo-bureau` service, key in
`secrets/connector_bureau_api_key`). Once the first administrator has a password, the Get started checklist shows the
steps already done; create a Runtime token under **Environments** and call the decision:

```bash
curl -X POST http://localhost:8090/api/projects/retail-credit/evaluate/credit-application \
  -H 'content-type: application/json' -H "X-Access-Token: $TOKEN" \
  -d '{"context": {"applicant": {"nationalId": "CM-109", "age": 38, "employment": "public",
       "monthsInJob": 96, "monthlyIncome": 450000, "monthlyDebtPayments": 40000,
       "salaryDomiciled": true}, "loan": {"amount": 2500000, "termMonths": 36}}}'
```

The fake bureau's answers are synthetic and depend on the last digit of `nationalId`: most are
clean, `7` and `8` have arrears and `9` serious ones (a decline). The decision appears in
Studio's decision log. A demo is for a first look: install again without `--demo` for real use.

**Change a setting later** in `donka.yaml`, then write the files again and restart:

```yaml
# donka.yaml (excerpt)
public_url: https://donka.bank.example   # behind HTTPS, sessions use secure cookies
administrator: you@bank.example
role: production
second_step: privileged                  # none, privileged or all
storage:
  kind: disk                             # or s3, with bucket, region and endpoint
mail:
  kind: smtp                             # mailpit catches every email instead
  from: Donka <donka@bank.example>       # the URL, with the password, in secrets/smtp_url
```

```bash
docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/install" -w /install \
  ghcr.io/youmssi/donka-studio:<installed version> donka-admin init --config donka.yaml
docker compose up -d
```

`init --config` asks nothing. When an answer needs a secret that is not in `secrets/` yet, it
stops and names the file to write, for example `./secrets/smtp_url is missing: write the SMTP
URL there`. Write it (`printf '%s' 'smtp://user:pass@mail.bank.example:587?tls=required' >
secrets/smtp_url`), then run it again. **Before real use**, set at least the address people
reach Studio at and your mail server.

Do not edit `.env` or `runtime.env`: init writes them again. A setting `donka.yaml` does not
hold (any of [.env.example](../.env.example), or the Runtime's) goes in
`docker-compose.override.yml`, which Compose reads after `docker-compose.yml` and init never
touches:

```yaml
# docker-compose.override.yml
services:
  app:
    environment:
      DONKA_TRUSTED_PROXIES: 172.16.0.0/12
```

**Secrets as files.** Every secret setting `X` (database URL, mail URL, storage keys, the
decision-log, person-index and second-step keys and their previous keys, the LLM key) is given
as `X_FILE`, the path of a file holding the value: init points them at `/run/secrets/`.
Trailing newlines are removed. Setting both forms, or a file Studio cannot read, stops Studio
at start and names the setting. Vault agents (OpenBao, Vault, Infisical) can write the files in
`secrets/` instead. On Kubernetes, mount the secret as a volume and point `X_FILE` at the file.
The Runtime takes the same form for its secrets
([configuration guide](https://github.com/youmssi/donka-runtime/blob/develop/docs/configuration.md)).

Behind a reverse proxy, also set `DONKA_TRUSTED_PROXIES` to its address (or the Docker network,
e.g. `172.16.0.0/12`): sign-in and password reset are limited per client address, and without
it every client shares the proxy's.

Administrators start projects from the packs in [packs/](../packs/README.md), which the image
ships in `/srv/packs`. To offer your own, mount a folder of packs and point `DONKA_PACKS_DIR` at
it; Studio does not start if one of them is invalid, and names it.

**Licence.** Donka sends each installation a licence file naming the organisation, the days it
is valid and the modules it includes (explanations, connectors, analysis). Studio verifies it
without network access, at start and every day. Give its path when init asks, or copy it to
`secrets/licence.json`, set `licence: true` in `donka.yaml` and run `init --config donka.yaml`.

- **Without a licence file**, Studio runs in evaluation: everything works, but releases cannot
  reach production.
- **From thirty days before the end**, administrators see a notice.
- **After the last day**, or if the file was changed or was issued for another installation,
  Studio stays readable and exportable but refuses new versions and releases, and says why.
  The log names the file and the reason.
- **Decisions are never affected:** the Runtime does not read licences and keeps serving
  released rules.

To install a new licence, replace `secrets/licence.json` and reload it without a restart:

```bash
docker compose kill -s HUP app
```

A module the licence leaves out is hidden in Studio, and its actions are refused with a message
that names it.

PostgreSQL takes its password, `secrets/postgres_password`, when its data is first created
(section 5 explains how to change it later).

The staging Runtime sends its decisions to Studio's decision log with a token Studio issues at
its first start, listed as *Staging Runtime (installed)* among the decision-log tokens. To
replace it, revoke it in Studio, then remove the file and restart; Studio issues a new one:

```bash
docker run --rm -v donka_wiring:/wiring alpine rm /wiring/staging-decision-log-token
docker compose restart app runtime
```

The staging Runtime reads its settings from `runtime.env`, which init writes; add others in
`docker-compose.override.yml` (under `runtime:`). They are documented in its
[configuration guide](https://github.com/youmssi/donka-runtime/blob/develop/docs/configuration.md).
A production Runtime usually runs on another machine: Studio and its Runtimes then share an S3
store (your own, or [SILO](https://github.com/pgsty/silo)) instead of the `releases` volume.

**Connector secrets.** A decision's connector nodes name their secrets; the Runtime reads each
value from the file `DONKA_SECRET_<NAME>_FILE` names (or from `DONKA_SECRET_<NAME>`). A service
that wants an OAuth2 token needs its client secret; one that requires mutual TLS needs a client
certificate, its key and, for a private CA, the CA bundle. List the names in `donka.yaml`, write
each value to `secrets/connector_<name>` (lowercase), and run `init --config donka.yaml`:

```yaml
# donka.yaml
connector_secrets:
  - BUREAU_API_KEY          # secrets/connector_bureau_api_key: an API key in a header
  - CBS_CLIENT_SECRET       # OAuth2 client credentials
  - CBS_CLIENT_CERT         # mutual TLS, PEM
  - CBS_CLIENT_KEY
  - CBS_CA
```

The settings each method takes, and how failures are reported, are in the Runtime's
[connectors guide](https://github.com/youmssi/donka-runtime/blob/develop/docs/connectors.md).
Restart the Runtime after replacing a certificate.

### Published images

Each release publishes signed images to GitHub Container Registry, with an SBOM and a
provenance attestation: `ghcr.io/youmssi/donka-studio` (private: Donka gives each customer a
read token) and `ghcr.io/youmssi/donka-runtime` (public). `edge` follows development, for
staging and demos only. Verify an image before running it, with
[cosign](https://docs.sigstore.dev/cosign/system_config/installation/) and the public key
`cosign.pub` from this repository; no network access to a transparency log is needed:

```bash
echo "$DONKA_REGISTRY_TOKEN" | docker login ghcr.io -u <your GitHub user> --password-stdin
cosign verify --key cosign.pub --insecure-ignore-tlog ghcr.io/youmssi/donka-studio:1.5.0
cosign verify --key cosign.pub --insecure-ignore-tlog ghcr.io/youmssi/donka-runtime:1.5.0
docker buildx imagetools inspect ghcr.io/youmssi/donka-studio:1.5.0 --format '{{ json .SBOM }}'
```

### Without internet access

Each release also comes as an offline bundle, `donka-<version>-offline.tar`, attached to the
release: Studio, the latest Runtime release and every other image the installation runs (the
demo's included), for linux/amd64 servers. Its `SHA256SUMS` is signed with the same key as the
images. Copy the bundle to the server, then check it with `cosign.pub` obtained from Donka, never
from the bundle itself; `openssl` does instead of cosign:

```bash
tar -xf donka-1.5.0-offline.tar && cd donka-1.5.0-offline
openssl dgst -sha256 -verify cosign.pub -signature <(base64 -d SHA256SUMS.sig) SHA256SUMS
sha256sum -c SHA256SUMS
docker load -i images.tar.gz
```

Then install as in section 1 without logging in to the registry, with the versions the
bundle's `README.txt` gives, `donka-admin init --offline` (Studio then never looks for new
releases) and `docker compose up -d --pull never`. `images.txt` lists each
image with the registry digest its own signature covers. CI installs from a bundle with every
image removed beforehand, on each change.

**Five minutes.** On GitHub's `ubuntu-latest` runner (4 vCPU, 16 GB), the reference machine,
pulling the images, `donka-admin init --demo` and `docker compose up` reach a Runtime decision
recorded in Studio in under five minutes. CI measures it after each published image and fails
above that.

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

Back up three things, together: the database, the release volume and the installation folder
(`donka.yaml` and `secrets/`).

```bash
mkdir -p backups
stamp=$(date +%Y%m%d-%H%M)

# The database: projects, decisions, releases, approvals, audit and decision log (its
# monthly partitions included).
docker compose exec -T postgres pg_dump -U donka -Fc donka > "backups/donka-$stamp.dump"

# The release volume: what the Runtimes serve.
docker run --rm -v donka_releases:/releases:ro -v "$PWD/backups:/backups" alpine \
  tar -czf "/backups/releases-$stamp.tar.gz" -C /releases .
```

Keep a copy of `secrets/` in your secret store. **Without the decision-log key
(`secrets/decision_log_key` and, after a rotation, its previous keys) the decision log cannot be read**, even from a
backup. **Without the second-step key (and its previous keys) nobody with a second sign-in step
can sign in** after a restore: keep it with the backups too. **Without the person index key
nobody can be found by their identifiers** in a restored
decision log until it is indexed again with a new key (below).

Copy the backups off the server and test a restore regularly. To restore, onto a stopped Studio:

```bash
docker compose stop app runtime
docker compose exec -T postgres pg_restore -U donka -d donka --clean --if-exists < backups/donka-<stamp>.dump
docker run --rm -v donka_releases:/releases -v "$PWD/backups:/backups" alpine \
  sh -c "rm -rf /releases/* && tar -xzf /backups/releases-<stamp>.tar.gz -C /releases"
docker compose up -d app runtime
```

## 4. Upgrade

Studio tells administrators when a newer release is published: it asks GitHub once a day, and
never with `DONKA_UPDATE_CHECK=false` (`donka-admin init --offline` sets it). Read the release
notes ([releases](https://github.com/youmssi/donka/releases)) first, then let
`donka-admin upgrade` do the rest. Run it with the image of the version **installed now**, in the
installation folder, which must hold Donka's public key as `cosign.pub`:

```bash
docker run --rm -u "$(id -u):$(id -g)" --group-add "$(stat -c %g /var/run/docker.sock)" \
  -v /var/run/docker.sock:/var/run/docker.sock -v "$PWD:/install" -w /install \
  -e DONKA_REGISTRY_USER -e DONKA_REGISTRY_TOKEN \
  ghcr.io/youmssi/donka-studio:1.5.0 donka-admin upgrade 1.6.0 --runtime-version 1.4.0
```

1. **Verify.** The new images' signatures are checked with `cosign.pub` before anything is
   pulled for use; an image that does not verify is refused. `DONKA_REGISTRY_USER` and
   `DONKA_REGISTRY_TOKEN` hold the read token Donka gives you for Studio's image.
2. **Check compatibility.** Each release states which installed versions it upgrades from and
   which Runtime it needs, in a file its image carries. An installation it refuses stays as it
   is: upgrade through the release the notes name first, or add `--runtime-version`.
3. **Back up** to `backups/<time>/`: the database (`donka.dump`), the release and wiring volumes
   (`volumes.tar.gz`), `donka.yaml`, `secrets/`, `.env`, `runtime.env` and `docker-compose.yml`.
4. **Upgrade the Runtime, then Studio**: the new versions go in `donka.yaml`, and the files are
   written again from it with the new release's `docker-compose.yml` (keep your own changes in
   `docker-compose.override.yml`). After each, the smoke test: Studio
   ready (database reachable, migrations applied), both services running the verified images,
   the Runtime healthy.
5. **If a check fails**, the previous versions, the database, the volumes and the files come
   back from the backup, and are checked the same way.

Every run writes a report to `upgrades/<time>-<from>-to-<to>.txt` (and `.json`), for your change
record: versions, image digests, each step and what failed. The exit status says the outcome:
0 upgraded, 3 refused (nothing changed), 4 rolled back, 5 the restore failed too (the report and
the backup say where things stand). Then run the full smoke test (section 2).

**By hand.** Without `donka-admin upgrade`:

1. When a release changes the artifact format or the Runtime's API, upgrade the Runtime first.
2. Back up (section 3). Database migrations only go forward: the way back is the backup.
3. Set the new versions in `donka.yaml` (`images.studio`, and `images.runtime` when the
   release notes name one), run `donka-admin init --config donka.yaml` with the **new**
   version's image, verify the images (*Published images*), then pull and restart:

   ```bash
   docker compose pull && docker compose up -d
   ```

   Studio applies pending migrations when it starts (`DONKA_DB_MIGRATE=true`).
4. Check it: `curl -s localhost:8080/api/v1/version`, then the smoke test (section 2).
5. If Studio's log says decision records are not indexed yet (after the upgrade that lets
   owners find a person's records, DNK-75), index them; it runs alongside Studio, and can be
   stopped and run again:

   ```bash
   docker compose exec app donka-app decision-log index-people
   ```

**Unattended upgrades** (staging and demo installations only, off by default). Prepare the
installation with its role and a weekly window, in the server's time:

```yaml
# donka.yaml, then: donka-admin init --config donka.yaml
role: staging
auto_upgrade: Sun 02:00-04:00
```

The assistant asks for the window too (`--role staging --auto-upgrade "Sun 02:00-04:00"` as
options); it turns on the `auto-upgrade` service.
Keep Donka's `cosign.pub` in the folder. The `auto-upgrade` service then looks every five
minutes. Once per window it upgrades to the latest release of the same major version, exactly
as `donka-admin upgrade` does: verified, backed up, checked, rolled back on failure, reported in
`upgrades/`. A new major version is only announced; upgrade it by hand. A production
installation (the default role) never upgrades by itself: init refuses the window, and the
service does nothing. `donka-admin auto-upgrade --once` looks once, now. The service drives
Docker, so it runs as root, and its backups and reports belong to root.

## 5. Rotate secrets

Plan a short maintenance window for the passwords: the services restart.

**Database password**

```bash
new=$(openssl rand -hex 24)
docker compose exec postgres psql -U donka -c "ALTER USER donka PASSWORD '$new'"
printf '%s' "$new" > secrets/postgres_password
printf 'postgres://donka:%s@postgres:5432/donka' "$new" > secrets/database_url
docker compose up -d --force-recreate app
```

**Storage keys**, when releases go to an S3 store: change the key in the store, write the new
values to `secrets/storage_access_key_id` and `secrets/storage_secret_access_key` (the staging
Runtime reads the same files; set the other Runtimes' `AWS_*` keys), then restart them. The `releases` volume has no keys.

**Mail server credentials**: write the new URL to `secrets/smtp_url`, then
`docker compose restart app`.

**Runtime, CI and decision-log tokens** are issued in Studio. Issue a new token, give it to the
system that uses it, check that system works, then revoke the old token. Each step is in the
audit log.

Studio reads its keys from `secrets/`; their previous keys go in files there too. Before the
first rotation, create them empty (`touch secrets/decision_log_previous_keys
secrets/second_step_previous_keys`) and name them in `docker-compose.override.yml`:

```yaml
# docker-compose.override.yml
services:
  app:
    environment:
      DONKA_DECISION_LOG_PREVIOUS_KEYS_FILE: /run/secrets/decision_log_previous_keys
      DONKA_SECOND_STEP_PREVIOUS_KEYS_FILE: /run/secrets/second_step_previous_keys
```

**Decision-log key**: records are encrypted with the key in `secrets/decision_log_key`. Replace it when it
may have leaked, when someone who knew it leaves, or if you started with a public default key (the
source build's `docker-compose.yml` has them). Each record names the key it was sealed with, so nothing is lost:

1. Generate a new key: `openssl rand -base64 32`.
2. Move the current value to `secrets/decision_log_previous_keys` (several keys are separated
   by commas) and write the new key to `secrets/decision_log_key`. Do not drop the old value:
   records sealed with it could no longer be read.
3. `docker compose up -d --force-recreate app`. Studio seals new records with the new key and still reads the
   old ones with the previous key; its log lists the keys still in use.
4. Re-seal the old records with the new key:

   ```bash
   docker compose exec app donka-app decision-log reseal
   ```

   It works in batches of 500, each recorded in the project's audit log (*Decision records
   re-sealed*). It can be stopped and run again: it carries on with the records left. When it
   names a key that is missing, add that key to `secrets/decision_log_previous_keys` first.
5. When it says no record uses the old key any more, remove it from
   `secrets/decision_log_previous_keys` and restart Studio again.

Backups taken before the re-seal still hold records sealed with the old key: keep the old key
with those backups, in your secret store, until they expire. If the key leaked, also restrict
access to the database and its backups: the key alone does not give access to records.

**Second-step key**: authenticator secrets are sealed with `secrets/second_step_key`. To replace
it, move the current value to `secrets/second_step_previous_keys`, write the new key and run
`docker compose up -d --force-recreate app`. Each secret is re-sealed with the new key the next time its owner
signs in; keep the previous key until everyone with a second step has signed in once (or an
administrator has reset the steps of those who have not).

**Person index key**: identifiers (national ID, phone…) are found through hashes made with
`secrets/person_index_key`; there is no previous key. To replace it, write the new key, empty the
index and index the log again, Studio stopped:

```bash
docker compose stop app
docker compose exec postgres psql -U donka -c "TRUNCATE decision_record_people; \
  UPDATE decision_record_people_backfill SET received_before = now(), after_evaluated_at = NULL, \
  after_id = NULL, finished_at = NULL"
docker compose up -d app
docker compose exec app donka-app decision-log index-people
```

**User passwords**: people change theirs with *Forgot your password?* on the sign-in page.

**Two-step sign-in**: anyone can turn it on in *Account*, with any authenticator app (codes of
six digits that change every 30 seconds). `DONKA_MFA_REQUIRED=privileged` requires it of
instance administrators and project owners, `all` of everyone; they set it up right after their
password at their next sign-in. A person who lost their phone signs in with one of their
recovery codes; if they have none left, another administrator resets their second step in
*People*, and they set it up again. Each change is audited and the person is told by email.

## Troubleshooting

Start with the doctor. It checks, with Studio's own settings and from Studio's network, what
Studio depends on: the database (and its migrations), the release storage (it writes a test
file and removes it), the mail server (it connects and signs in, without sending anything),
sign-in, the language model (it asks for one word), the licence, the packs, and each service
the live decisions' connectors call (its name resolves and its port answers; nothing is sent).
It works even when Studio does not start:

```bash
docker compose run --rm --no-deps app donka-admin doctor
```

```
  ok    settings        Studio accepts them
  ok    database        reachable, every migration applied
  ok    storage         file:///var/lib/donka/releases is writable
  FAIL  mail            cannot talk to the server: Connection refused (os error 111)
                        → check the SMTP URL (secrets/smtp_url, DONKA_SMTP_URL): host, port, …
  ok    sign-in         local accounts, a second step for administrators and project owners
  off   language model  explanations are off
  ok    licence         Microfinance SA, valid until 2027-10-31 (366 days left)
  ok    packs           3 offered
  ok    connectors      https://bureau.example/score is reachable
doctor: 1 check failed
```

Each failing line says why, and under it what to change. The exit status is 0 when everything
works and 1 otherwise, so it can gate a deployment pipeline. A production Runtime on another
network may reach connectors differently: its logs name each failed call.

| Symptom | Look at |
| --- | --- |
| Studio does not start | `docker compose logs app`: a missing or invalid setting is named, with an example |
| `/api/v1/ready` answers 503 | PostgreSQL: `docker compose ps postgres`, then `docker compose logs postgres` |
| A deployment stays *pending* | `docker compose logs app`: the publisher retries until the release storage answers |
| The Runtime answers 404 for a project | It serves one environment (staging here); check the release is live there, then `docker compose logs runtime` |
| The Runtime keeps restarting at the first start | It waits for Studio to write its decision-log token: `docker compose logs app \| grep decision-log` |
| Studio stops with `DONKA_PACKS_DIR` | A pack in that folder is invalid: the message names the pack and why |
| "The licence file was refused" | `docker compose logs app \| grep licence` gives the reason: the file was changed, was issued for another installation, or cannot be read at `DONKA_LICENCE_FILE` |
| No emails | Mailpit (http://localhost:8025) until `DONKA_SMTP_URL` points to your mail server |
