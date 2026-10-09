# Installer et exploiter Donka

[English](install.md)

Ce guide installe Donka sur un serveur avec Docker Compose et couvre son exploitation
courante : le test de fumée après chaque déploiement, les sauvegardes, les mises à jour et le
renouvellement des secrets.

Une installation sert une organisation. Elle fait tourner cinq conteneurs :

| Service | Rôle | Port (hôte) |
| --- | --- | --- |
| `app` | Donka Studio : l'application web et son API | `8080` |
| `runtime` | Donka Runtime, qui sert l'environnement **staging** | `8090` |
| `postgres` | La base de données de Studio (PostgreSQL 16) | `127.0.0.1:5432` |
| `minio` | Le stockage des versions publiées (compatible S3) | `127.0.0.1:9000`, console `127.0.0.1:9001` |
| `mailpit` | Retient les e-mails sortants tant qu'aucun serveur de messagerie n'est configuré | `127.0.0.1:8025` |

PostgreSQL, MinIO et Mailpit n'écoutent que sur le serveur lui-même. Exposez Studio et le
Runtime derrière votre proxy inverse (TLS), pas directement.

## 1. Installer

Il faut Docker Engine avec le plugin Compose (2.24 ou plus récent), git, environ 10 Go
d'espace disque libre pour la première construction, et un accès sortant à GitHub et aux
registres d'images.

```bash
git clone https://github.com/youmssi/donka.git && cd donka
docker compose --profile full up -d
```

Le premier démarrage construit Studio et le Runtime depuis les sources (15 à 30 minutes).
Ensuite :

1. Ouvrez http://localhost:8080 (ou l'adresse de votre serveur).
2. Récupérez le lien à usage unique pour choisir le mot de passe du premier administrateur :

   ```bash
   docker compose logs app | grep setup-password
   ```

   L'administrateur est `admin@donka.local`, sauf si vous définissez `DONKA_BOOTSTRAP_ADMIN_EMAIL`.
3. Lancez le test de fumée (section 2).

Chaque paramètre a une valeur par défaut qui fonctionne sur une seule machine. **Avant un
usage réel, définissez vos propres valeurs** dans un fichier `.env` à côté de
`docker-compose.yml` (gardez-le hors de git : il contient des secrets) :

```bash
# .env
DONKA_PUBLIC_URL=https://donka.banque.example   # l'adresse où l'on joint Studio
DONKA_COOKIE_SECURE=true                        # Studio est servi en HTTPS
DONKA_BOOTSTRAP_ADMIN_EMAIL=vous@banque.example # premier administrateur, sur une base vide
DONKA_POSTGRES_PASSWORD=<généré>                # openssl rand -hex 24
DONKA_STORAGE_ACCESS_KEY_ID=donka
DONKA_STORAGE_SECRET_ACCESS_KEY=<généré>        # openssl rand -hex 24
DONKA_DECISION_LOG_KEY=<généré>                 # openssl rand -base64 32
DONKA_SMTP_URL=smtp://user:pass@mail.banque.example:587?tls=required
DONKA_SMTP_FROM=Donka <donka@banque.example>
```

Derrière un proxy inverse, définissez aussi `DONKA_TRUSTED_PROXIES` avec son adresse (ou le
réseau Docker, par exemple `172.16.0.0/12`) : la connexion et la réinitialisation du mot de passe
sont limitées par adresse cliente, et sans ce paramètre tous les clients partagent celle du proxy.

Les administrateurs démarrent des projets à partir des packs de [packs/](../packs/README.md), que
l'image fournit dans `/srv/packs`. Pour proposer les vôtres, montez un dossier de packs et
indiquez-le dans `DONKA_PACKS_DIR` ; Studio ne démarre pas si l'un d'eux est invalide, et le nomme.

Définissez-les avant le premier démarrage : PostgreSQL et MinIO prennent leur mot de passe à la
création de leurs données (la section 5 explique comment les changer ensuite). Tous les autres
paramètres de Studio, listés dans [.env.example](../.env.example), peuvent aller dans le même
fichier.

Le Runtime lit des paramètres supplémentaires dans un fichier facultatif `runtime.env`, par
exemple pour envoyer ses décisions au journal des décisions de Studio (créez le jeton dans
**Runtimes** dans Studio) :

```bash
# runtime.env
DECISION_LOG__URL=http://app:8080/api/v1/decision-log/records
DECISION_LOG__TOKEN=dnk_log_...
```

Ses paramètres sont décrits dans le
[guide de configuration](https://github.com/youmssi/donka-runtime/blob/develop/docs/configuration.md)
du Runtime. Pour la production, faites tourner un second Runtime avec `PROVIDER__PREFIX: production/`.

## 2. Test de fumée après chaque déploiement

`scripts/smoke.sh` vérifie que Studio répond et joint sa base de données, que le moteur de
décision simule, et que le Runtime évalue une version publiée par ce Studio.

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

Le compte doit pouvoir créer des projets. L'évaluation utilise son propre projet,
`donka-smoke` : le premier passage le crée avec une petite décision et le déploie en staging ;
les passages suivants le réutilisent. Chaque passage crée un jeton Runtime et le révoque avant
de se terminer. Une vérification qui échoue affiche `FAIL` avec la raison et termine avec un
code non nul : le script peut donc conditionner un pipeline de déploiement.

| Variable | Défaut | Signification |
| --- | --- | --- |
| `STUDIO_URL`, `RUNTIME_URL` | obligatoire | Les adresses de Studio et du Runtime |
| `DONKA_SMOKE_EMAIL`, `DONKA_SMOKE_PASSWORD` | obligatoire | Le compte avec lequel le script se connecte |
| `DONKA_SMOKE_PROJECT` | `donka-smoke` | La clé du projet de test |
| `DONKA_SMOKE_ENVIRONMENT` | `staging` | L'environnement que sert le Runtime à `RUNTIME_URL` |
| `DONKA_SMOKE_WAIT_SECONDS` | `90` | Combien de temps attendre que le Runtime prenne un déploiement |

Pour vérifier un Runtime de production, définissez `DONKA_SMOKE_ENVIRONMENT=production`. Les
déploiements en production demandent une approbation : approuvez une fois la version du projet
de test dans Studio.

## 3. Sauvegarder

Sauvegardez trois choses, ensemble : la base de données, le bucket des versions et les secrets.

```bash
mkdir -p backups
stamp=$(date +%Y%m%d-%H%M)

# La base : projets, décisions, versions, approbations, audit et journal des décisions.
docker compose exec -T postgres pg_dump -U donka -Fc donka > "backups/donka-$stamp.dump"

# Le bucket des versions : ce que servent les Runtimes.
docker compose run --rm -v "$PWD/backups:/backups" --entrypoint sh minio-init -c \
  "mc alias set local http://minio:9000 \$AWS_ACCESS_KEY_ID \$AWS_SECRET_ACCESS_KEY >/dev/null &&
   mc mirror --overwrite local/donka-releases /backups/releases-$stamp"
```

Gardez `.env` et `runtime.env` dans votre coffre à secrets. **Sans `DONKA_DECISION_LOG_KEY` (et,
après un renouvellement, les clés de `DONKA_DECISION_LOG_PREVIOUS_KEYS`), le journal des
décisions est illisible**, même depuis une sauvegarde.

Copiez les sauvegardes hors du serveur et testez une restauration régulièrement. Pour
restaurer, Studio arrêté :

```bash
docker compose stop app runtime
docker compose exec -T postgres pg_restore -U donka -d donka --clean --if-exists < backups/donka-<stamp>.dump
docker compose run --rm -v "$PWD/backups:/backups" --entrypoint sh minio-init -c \
  "mc alias set local http://minio:9000 \$AWS_ACCESS_KEY_ID \$AWS_SECRET_ACCESS_KEY >/dev/null &&
   mc mirror --overwrite /backups/releases-<stamp> local/donka-releases"
docker compose up -d app runtime
```

## 4. Mettre à jour

1. Lisez les notes de version ([releases](https://github.com/youmssi/donka/releases)). Quand
   une version change le format des artefacts ou l'API du Runtime, mettez d'abord le Runtime à
   jour.
2. Sauvegardez (section 3). Les migrations de la base ne vont que vers l'avant : le retour
   arrière, c'est la sauvegarde.
3. Récupérez la nouvelle version et reconstruisez :

   ```bash
   git fetch --tags && git checkout v1.5.0
   DONKA_VERSION=1.5.0 docker compose --profile full up -d --build
   ```

   Pour fixer le Runtime sur une version, définissez
   `DONKA_RUNTIME_SOURCE=https://github.com/youmssi/donka-runtime.git#v1.5.0` dans `.env`.
   Studio applique les migrations en attente au démarrage (`DONKA_DB_MIGRATE=true`).
4. Vérifiez : `curl -s localhost:8080/api/v1/version`, puis le test de fumée (section 2).

## 5. Renouveler les secrets

Prévoyez une courte fenêtre de maintenance pour les mots de passe : les services redémarrent.

**Mot de passe de la base**

```bash
docker compose exec postgres psql -U donka -c "ALTER USER donka PASSWORD 'nouveau-mot-de-passe'"
# mettez DONKA_POSTGRES_PASSWORD=nouveau-mot-de-passe dans .env, puis :
docker compose up -d app
```

**Clés du stockage (MinIO)** : MinIO lit ses identifiants racine au démarrage. Mettez les
nouvelles valeurs de `DONKA_STORAGE_ACCESS_KEY_ID` et `DONKA_STORAGE_SECRET_ACCESS_KEY` dans
`.env`, puis redémarrez tous les services qui les utilisent :

```bash
docker compose --profile full up -d minio minio-init app runtime
```

**Identifiants du serveur de messagerie** : changez `DONKA_SMTP_URL` dans `.env`, puis
`docker compose up -d app`.

**Jetons Runtime, CI et journal des décisions** : ils se créent dans Studio. Créez un nouveau
jeton, donnez-le au système qui l'utilise, vérifiez que ce système fonctionne, puis révoquez
l'ancien. Chaque étape est dans le journal d'audit.

**Clé du journal des décisions** : les enregistrements sont chiffrés avec
`DONKA_DECISION_LOG_KEY`. Remplacez-la si elle a pu fuiter, si une personne qui la connaissait
part, ou si vous avez démarré avec la clé par défaut de `docker-compose.yml`. Chaque
enregistrement indique la clé qui l'a chiffré, donc rien n'est perdu :

1. Générez une nouvelle clé : `openssl rand -base64 32`.
2. Dans `.env`, déplacez la valeur actuelle dans `DONKA_DECISION_LOG_PREVIOUS_KEYS` (plusieurs
   clés sont séparées par des virgules) et mettez la nouvelle clé dans `DONKA_DECISION_LOG_KEY`.
   Ne supprimez pas l'ancienne valeur : les enregistrements chiffrés avec elle ne seraient plus
   lisibles.
3. `docker compose up -d app`. Studio chiffre les nouveaux enregistrements avec la nouvelle clé
   et lit toujours les anciens avec la précédente ; son journal liste les clés encore utilisées.
4. Rechiffrez les anciens enregistrements avec la nouvelle clé :

   ```bash
   docker compose exec app donka-app decision-log reseal
   ```

   La commande avance par lots de 500, chacun inscrit au journal d'audit du projet
   (*Décisions enregistrées rechiffrées*). Elle peut être arrêtée puis relancée : elle reprend
   avec les enregistrements restants. Si elle nomme une clé manquante, ajoutez d'abord cette clé
   à `DONKA_DECISION_LOG_PREVIOUS_KEYS`.
5. Quand elle indique qu'aucun enregistrement n'utilise plus l'ancienne clé, retirez-la de
   `DONKA_DECISION_LOG_PREVIOUS_KEYS` et relancez `docker compose up -d app`.

Les sauvegardes faites avant le rechiffrement contiennent encore des enregistrements chiffrés
avec l'ancienne clé : gardez l'ancienne clé avec ces sauvegardes, dans votre coffre à secrets,
jusqu'à leur expiration. Si la clé a fuité, restreignez aussi l'accès à la base et à ses
sauvegardes : la clé seule ne donne pas accès aux enregistrements.

**Mots de passe des personnes** : chacun change le sien avec *Mot de passe oublié ?* sur la
page de connexion.

## Dépannage

| Symptôme | Où regarder |
| --- | --- |
| Studio ne démarre pas | `docker compose logs app` : un paramètre manquant ou invalide est nommé, avec un exemple |
| `/api/v1/ready` répond 503 | PostgreSQL : `docker compose ps postgres`, puis `docker compose logs postgres` |
| Un déploiement reste *en attente* | `docker compose logs app` : le publicateur réessaie jusqu'à ce que le bucket réponde |
| Le Runtime répond 404 pour un projet | Il sert un seul environnement (`PROVIDER__PREFIX`) ; vérifiez que la version y est en ligne |
| Studio s'arrête sur `DONKA_PACKS_DIR` | Un pack de ce dossier est invalide : le message nomme le pack et la raison |
| Aucun e-mail | Mailpit (http://localhost:8025) tant que `DONKA_SMTP_URL` ne pointe pas vers votre serveur |
