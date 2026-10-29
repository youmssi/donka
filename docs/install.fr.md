# Installer et exploiter Donka

[English](install.md)

Ce guide installe Donka sur un serveur avec Docker Compose et couvre son exploitation
courante : le test de fumée après chaque déploiement, les sauvegardes, les mises à jour et le
renouvellement des secrets.

Une installation sert une organisation. Elle fait tourner quatre conteneurs depuis les images
publiées de Donka :

| Service | Rôle | Port (hôte) |
| --- | --- | --- |
| `app` | Donka Studio : l'application web et son API | `8080` |
| `runtime` | Donka Runtime, qui sert l'environnement **staging** | `8090` |
| `postgres` | La base de données de Studio (PostgreSQL 16) | aucun |
| `mailpit` | Retient les e-mails sortants tant qu'aucun serveur de messagerie n'est configuré | `127.0.0.1:8025` |

Studio publie les versions dans le volume `releases`, que le Runtime de staging sert et
recharge quand un déploiement le modifie ; Studio relie le Runtime à son journal des décisions
au premier démarrage (le jeton est dans le volume `wiring`). Exposez Studio et le Runtime
derrière votre proxy inverse (TLS), pas directement. Pour construire Donka depuis les sources,
voir [CONTRIBUTING.md](../CONTRIBUTING.md).

## 1. Installer

Il faut un serveur avec Docker Engine et le plugin Compose (2.24 ou plus récent), et un accès à
GitHub Container Registry. L'image de Studio est privée : connectez-vous avec le jeton de lecture
que Donka vous remet (voir *Images publiées* plus bas). Rien d'autre n'est installé sur le
serveur.

```bash
echo "$DONKA_REGISTRY_TOKEN" | docker login ghcr.io -u <votre utilisateur GitHub> --password-stdin
mkdir donka && cd donka
docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/install" -w /install \
  ghcr.io/youmssi/donka-studio:latest donka-admin init \
  --public-url https://donka.banque.example --admin-email vous@banque.example
docker compose up -d
```

1. `donka-admin init` écrit `docker-compose.yml` et `.env` dans le dossier. `.env` contient les
   secrets de l'installation (mot de passe de la base, clés du journal des décisions, de la
   seconde étape et de l'index des personnes), tirés au hasard et lisibles par vous seul.
   Relancer init n'écrase jamais ces fichiers. Gardez `.env` hors du contrôle de version et
   sauvegardez-le (section 3).
2. Récupérez le lien à usage unique pour choisir le mot de passe du premier administrateur ;
   c'est le seul secret jamais affiché :

   ```bash
   docker compose logs app | grep setup-password
   ```

3. Ouvrez Studio (port 8080), choisissez le mot de passe, puis lancez le test de fumée
   (section 2).

Sans options, init prépare un premier essai sur cette machine : http://localhost:8080 et
`admin@donka.local`. `--studio-version` et `--runtime-version` fixent d'autres versions
d'images ; `--registry` prend les images dans votre propre miroir de `ghcr.io/youmssi`
(`DONKA_REGISTRY` dans `.env`).

**La démo.** `donka-admin init --demo` prépare la même installation avec un projet prêt à être
appelé. Au premier démarrage, Studio importe le pack *Crédit aux particuliers*, le publie et le
déploie en staging, et un faux bureau de crédit répond à son connecteur (service `demo-bureau`,
clé dans `runtime.env`). Une fois le mot de passe du premier administrateur choisi, la liste
*Pour commencer* montre les étapes déjà faites ; créez un jeton Runtime dans **Environnements**
et appelez la décision :

```bash
curl -X POST http://localhost:8090/api/projects/retail-credit/evaluate/credit-application \
  -H 'content-type: application/json' -H "X-Access-Token: $TOKEN" \
  -d '{"context": {"applicant": {"nationalId": "CM-109", "age": 38, "employment": "public",
       "monthsInJob": 96, "monthlyIncome": 450000, "monthlyDebtPayments": 40000,
       "salaryDomiciled": true}, "loan": {"amount": 2500000, "termMonths": 36}}}'
```

Les réponses du faux bureau sont fictives et dépendent du dernier chiffre de `nationalId` : la
plupart sont sans incident, `7` et `8` ont des retards et `9` des retards graves (un refus). La
décision apparaît dans le journal des décisions de Studio. Une démo sert à un premier essai :
réinstallez sans `--demo` pour un usage réel.

Pour changer un paramètre ensuite, modifiez `.env` puis lancez `docker compose up -d`. **Avant
un usage réel**, définissez au moins l'adresse où l'on joint Studio et votre serveur de
messagerie :

```bash
# .env
DONKA_PUBLIC_URL=https://donka.banque.example   # l'adresse où l'on joint Studio
DONKA_COOKIE_SECURE=true                        # Studio est servi en HTTPS
DONKA_SMTP_URL=smtp://user:pass@mail.banque.example:587?tls=required
DONKA_SMTP_FROM=Donka <donka@banque.example>
```

**Secrets sous forme de fichiers.** Chaque paramètre secret `X` (URL de la base, URL de
messagerie, clés du stockage, clés du journal des décisions, de l'index des personnes et de la
seconde étape et leurs clés précédentes, clé du LLM) peut être donné à la place par `X_FILE`, le
chemin d'un fichier qui contient la valeur, comme les fournissent les secrets Docker et
Kubernetes et les agents de coffre-fort (OpenBao, Vault, Infisical). Les sauts de ligne finaux
sont retirés. Définir les deux formes, ou un fichier que Studio ne peut pas lire, arrête Studio
au démarrage en nommant le paramètre. Avec les secrets Docker, par exemple :

```yaml
# docker-compose.override.yml
services:
  app:
    environment:
      DONKA_DECISION_LOG_KEY: ""                # le fichier compose a une valeur par défaut : videz-la
      DONKA_DECISION_LOG_KEY_FILE: /run/secrets/decision_log_key
    secrets: [decision_log_key]
secrets:
  decision_log_key:
    file: ./secrets/decision_log_key           # openssl rand -base64 32 > secrets/decision_log_key
```

Sur Kubernetes, montez le secret en volume et faites pointer `X_FILE` vers le fichier. Le Runtime
accepte la même forme pour ses secrets
([guide de configuration](https://github.com/youmssi/donka-runtime/blob/develop/docs/configuration.md)).

Derrière un proxy inverse, définissez aussi `DONKA_TRUSTED_PROXIES` avec son adresse (ou le
réseau Docker, par exemple `172.16.0.0/12`) : la connexion et la réinitialisation du mot de passe
sont limitées par adresse cliente, et sans ce paramètre tous les clients partagent celle du proxy.

Les administrateurs démarrent des projets à partir des packs de [packs/](../packs/README.md), que
l'image fournit dans `/srv/packs`. Pour proposer les vôtres, montez un dossier de packs et
indiquez-le dans `DONKA_PACKS_DIR` ; Studio ne démarre pas si l'un d'eux est invalide, et le nomme.

**Licence.** Donka envoie à chaque installation un fichier de licence qui nomme l'organisation,
les jours de validité et les modules inclus (explications, connecteurs, analyse). Studio le
vérifie sans accès réseau, au démarrage puis chaque jour. Montez-le en lecture seule et indiquez
son chemin dans `DONKA_LICENCE_FILE` :

```yaml
# docker-compose.override.yml
services:
  app:
    environment:
      DONKA_LICENCE_FILE: /etc/donka/licence.json
    volumes:
      - ./licence.json:/etc/donka/licence.json:ro
```

- **Sans fichier de licence**, Studio fonctionne en évaluation : tout marche, mais les releases
  ne peuvent pas aller en production.
- **À partir de trente jours avant la fin**, les administrateurs voient un avis.
- **Après le dernier jour**, ou si le fichier a été modifié ou émis pour une autre
  installation, Studio reste consultable et exportable mais refuse les nouvelles versions et
  releases, et dit pourquoi. Le journal nomme le fichier et la raison.
- **Les décisions ne sont jamais touchées :** le Runtime ne lit pas les licences et continue de
  servir les règles publiées.

Pour installer une nouvelle licence, remplacez le fichier et rechargez-le sans redémarrage :

```bash
docker compose kill -s HUP app
```

Un module que la licence n'inclut pas est masqué dans Studio, et ses actions sont refusées avec
un message qui le nomme.

PostgreSQL prend son mot de passe à la création de ses données (la section 5 explique comment
le changer ensuite). Tous les autres paramètres de Studio, listés dans
[.env.example](../.env.example), peuvent aller dans le même fichier.

Le Runtime de staging envoie ses décisions au journal des décisions de Studio avec un jeton que
Studio crée à son premier démarrage, listé comme *Staging Runtime (installed)* parmi les jetons
du journal. Pour le remplacer, révoquez-le dans Studio, puis supprimez le fichier et
redémarrez ; Studio en crée un nouveau :

```bash
docker run --rm -v donka_wiring:/wiring alpine rm /wiring/staging-decision-log-token
docker compose restart app runtime
```

Le Runtime lit des paramètres supplémentaires dans un fichier facultatif `runtime.env` à côté
de `docker-compose.yml` ; ils sont décrits dans son
[guide de configuration](https://github.com/youmssi/donka-runtime/blob/develop/docs/configuration.md).
Un Runtime de production tourne en général sur une autre machine : Studio et ses Runtimes
partagent alors un stockage S3 (le vôtre, ou [SILO](https://github.com/pgsty/silo)) au lieu du
volume `releases`.

**Secrets des connecteurs.** Les nœuds connecteurs d'une décision nomment leurs secrets ; le
Runtime lit chaque valeur dans `DONKA_SECRET_<NOM>`, ou dans le fichier désigné par
`DONKA_SECRET_<NOM>_FILE`. Un service qui demande un jeton OAuth2 a besoin de son secret client ;
un service qui exige le TLS mutuel a besoin d'un certificat client, de sa clé et, pour une AC
privée, du bundle de l'AC, de préférence sous forme de fichiers :

```bash
# runtime.env
DONKA_SECRET_BUREAU_API_KEY=...                                  # une clé d'API dans un en-tête
DONKA_SECRET_CBS_CLIENT_SECRET=...                               # identifiants client OAuth2
DONKA_SECRET_CBS_CLIENT_CERT_FILE=/run/secrets/cbs-client.crt    # TLS mutuel, PEM
DONKA_SECRET_CBS_CLIENT_KEY_FILE=/run/secrets/cbs-client.key
DONKA_SECRET_CBS_CA_FILE=/run/secrets/cbs-ca.pem
```

```yaml
# docker-compose.override.yml : les fichiers de certificats, en lecture seule
services:
  runtime:
    volumes:
      - ./secrets:/run/secrets:ro
```

Les paramètres de chaque méthode, et la façon dont les échecs sont signalés, sont dans le
[guide des connecteurs](https://github.com/youmssi/donka-runtime/blob/develop/docs/connectors.md)
du Runtime. Redémarrez le Runtime après avoir remplacé un certificat.

### Images publiées

Chaque version publie des images signées sur GitHub Container Registry, avec un SBOM et une
attestation de provenance : `ghcr.io/youmssi/donka-studio` (privée : Donka remet à chaque client
un jeton de lecture) et `ghcr.io/youmssi/donka-runtime` (publique). `edge` suit le développement,
pour la préproduction et les démonstrations seulement. Vérifiez une image avant de la lancer, avec
[cosign](https://docs.sigstore.dev/cosign/system_config/installation/) et la clé publique
`cosign.pub` de ce dépôt ; aucun accès réseau à un journal de transparence n'est nécessaire :

```bash
echo "$DONKA_REGISTRY_TOKEN" | docker login ghcr.io -u <votre utilisateur GitHub> --password-stdin
cosign verify --key cosign.pub --insecure-ignore-tlog ghcr.io/youmssi/donka-studio:1.5.0
cosign verify --key cosign.pub --insecure-ignore-tlog ghcr.io/youmssi/donka-runtime:1.5.0
docker buildx imagetools inspect ghcr.io/youmssi/donka-studio:1.5.0 --format '{{ json .SBOM }}'
```

### Sans accès à Internet

Chaque version est aussi livrée en paquet hors ligne, `donka-<version>-offline.tar`, joint à la
version : Studio, la dernière version du Runtime et toutes les autres images que l'installation
lance (celles de la démonstration comprises), pour des serveurs linux/amd64. Son `SHA256SUMS` est
signé avec la même clé que les images. Copiez le paquet sur le serveur, puis vérifiez-le avec un
`cosign.pub` obtenu auprès de Donka, jamais celui du paquet ; `openssl` remplace cosign :

```bash
tar -xf donka-1.5.0-offline.tar && cd donka-1.5.0-offline
openssl dgst -sha256 -verify cosign.pub -signature <(base64 -d SHA256SUMS.sig) SHA256SUMS
sha256sum -c SHA256SUMS
docker load -i images.tar.gz
```

Installez ensuite comme à la section 1, sans connexion au registre, avec les versions qu'indique
le `README.txt` du paquet et `docker compose up -d --pull never`. `images.txt` donne chaque image
avec le condensat du registre que couvre sa propre signature. La CI installe depuis un paquet,
toutes les images supprimées au préalable, à chaque changement.

**Cinq minutes.** Sur le runner `ubuntu-latest` de GitHub (4 vCPU, 16 Go), la machine de
référence, le téléchargement des images, `donka-admin init --demo` et `docker compose up`
mènent à une décision du Runtime enregistrée dans Studio en moins de cinq minutes. La CI le
mesure après chaque image publiée et échoue au-delà.

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

Sauvegardez trois choses, ensemble : la base de données, le volume des versions et les secrets.

```bash
mkdir -p backups
stamp=$(date +%Y%m%d-%H%M)

# La base : projets, décisions, versions, approbations, audit et journal des décisions
# (ses partitions mensuelles comprises).
docker compose exec -T postgres pg_dump -U donka -Fc donka > "backups/donka-$stamp.dump"

# Le volume des versions : ce que servent les Runtimes.
docker run --rm -v donka_releases:/releases:ro -v "$PWD/backups:/backups" alpine \
  tar -czf "/backups/releases-$stamp.tar.gz" -C /releases .
```

Gardez `.env` et `runtime.env` dans votre coffre à secrets. **Sans `DONKA_DECISION_LOG_KEY` (et,
après un renouvellement, les clés de `DONKA_DECISION_LOG_PREVIOUS_KEYS`), le journal des
décisions est illisible**, même depuis une sauvegarde. **Sans `DONKA_SECOND_STEP_KEY` (et ses
clés précédentes), personne ayant une seconde étape de connexion ne peut se connecter** après une
restauration : conservez-la aussi avec les sauvegardes. **Sans `DONKA_PERSON_INDEX_KEY`,
personne ne peut être retrouvé par ses identifiants** dans un journal restauré, tant qu'il n'a
pas été indexé de nouveau avec une nouvelle clé (ci-dessous).

Copiez les sauvegardes hors du serveur et testez une restauration régulièrement. Pour
restaurer, Studio arrêté :

```bash
docker compose stop app runtime
docker compose exec -T postgres pg_restore -U donka -d donka --clean --if-exists < backups/donka-<stamp>.dump
docker run --rm -v donka_releases:/releases -v "$PWD/backups:/backups" alpine \
  sh -c "rm -rf /releases/* && tar -xzf /backups/releases-<stamp>.tar.gz -C /releases"
docker compose up -d app runtime
```

## 4. Mettre à jour

Lisez d'abord les notes de version ([releases](https://github.com/youmssi/donka/releases)), puis
laissez `donka-admin upgrade` faire le reste. Lancez-le avec l'image de la version **installée
aujourd'hui**, dans le dossier de l'installation, qui doit contenir la clé publique de Donka sous
le nom `cosign.pub` :

```bash
docker run --rm -u "$(id -u):$(id -g)" --group-add "$(stat -c %g /var/run/docker.sock)" \
  -v /var/run/docker.sock:/var/run/docker.sock -v "$PWD:/install" -w /install \
  -e DONKA_REGISTRY_USER -e DONKA_REGISTRY_TOKEN \
  ghcr.io/youmssi/donka-studio:1.5.0 donka-admin upgrade 1.6.0 --runtime-version 1.4.0
```

1. **Vérifier.** Les signatures des nouvelles images sont contrôlées avec `cosign.pub` avant
   toute utilisation ; une image qui ne se vérifie pas est refusée. `DONKA_REGISTRY_USER` et
   `DONKA_REGISTRY_TOKEN` contiennent le jeton de lecture que Donka vous remet pour l'image de
   Studio.
2. **Contrôler la compatibilité.** Chaque version indique depuis quelles versions installées
   elle met à jour et quel Runtime elle demande, dans un fichier que porte son image. Une
   installation qu'elle refuse reste telle quelle : passez d'abord par la version que nomment les
   notes, ou ajoutez `--runtime-version`.
3. **Sauvegarder** dans `backups/<heure>/` : la base (`donka.dump`), les volumes des versions
   publiées et du raccordement (`volumes.tar.gz`), `.env`, `runtime.env` et `docker-compose.yml`.
4. **Mettre à jour le Runtime, puis Studio**, en installant le `docker-compose.yml` de la
   nouvelle version (gardez vos propres changements dans `docker-compose.override.yml`). Après
   chacun, le test de fumée : Studio prêt (base joignable, migrations appliquées), les deux
   services sur les images vérifiées, le Runtime en bonne santé.
5. **Si un contrôle échoue**, les versions précédentes, la base, les volumes et les fichiers
   reviennent de la sauvegarde, et sont contrôlés de la même façon.

Chaque exécution écrit un rapport dans `upgrades/<heure>-<de>-to-<vers>.txt` (et `.json`), pour
votre registre des changements : versions, condensats des images, chaque étape et ce qui a
échoué. Le code de sortie donne le résultat : 0 mis à jour, 3 refusé (rien n'a changé), 4 revenu
en arrière, 5 la restauration a échoué aussi (le rapport et la sauvegarde disent où en sont les
choses). Lancez ensuite le test de fumée complet (section 2).

**À la main.** Sans `donka-admin upgrade` :

1. Quand une version change le format des artefacts ou l'API du Runtime, mettez d'abord le
   Runtime à jour.
2. Sauvegardez (section 3). Les migrations de la base ne vont que vers l'avant : le retour
   arrière, c'est la sauvegarde.
3. Indiquez les nouvelles versions dans `.env` (`DONKA_VERSION`, et `DONKA_RUNTIME_VERSION`
   quand les notes de version en nomment une), vérifiez les images (*Images publiées*), puis
   récupérez-les et redémarrez :

   ```bash
   docker compose pull && docker compose up -d
   ```

   Studio applique les migrations en attente au démarrage (`DONKA_DB_MIGRATE=true`).
4. Vérifiez : `curl -s localhost:8080/api/v1/version`, puis le test de fumée (section 2).
5. Si le journal de Studio indique que des enregistrements ne sont pas encore indexés (après la
   mise à jour qui permet aux propriétaires de retrouver les décisions d'une personne, DNK-75),
   indexez-les ; la commande tourne à côté de Studio et peut être arrêtée puis relancée :

   ```bash
   docker compose exec app donka-app decision-log index-people
   ```

## 5. Renouveler les secrets

Prévoyez une courte fenêtre de maintenance pour les mots de passe : les services redémarrent.

**Mot de passe de la base**

```bash
docker compose exec postgres psql -U donka -c "ALTER USER donka PASSWORD 'nouveau-mot-de-passe'"
# mettez DONKA_POSTGRES_PASSWORD=nouveau-mot-de-passe dans .env, puis :
docker compose up -d app
```

**Clés du stockage**, quand les versions vont dans un stockage S3 : changez la clé dans le
stockage, mettez les nouvelles valeurs de `DONKA_STORAGE_ACCESS_KEY_ID` et
`DONKA_STORAGE_SECRET_ACCESS_KEY` pour Studio et les clés `AWS_*` des Runtimes, puis
redémarrez-les. Le volume `releases` n'a pas de clés.

**Identifiants du serveur de messagerie** : changez `DONKA_SMTP_URL` dans `.env`, puis
`docker compose up -d app`.

**Jetons Runtime, CI et journal des décisions** : ils se créent dans Studio. Créez un nouveau
jeton, donnez-le au système qui l'utilise, vérifiez que ce système fonctionne, puis révoquez
l'ancien. Chaque étape est dans le journal d'audit.

**Clé du journal des décisions** : les enregistrements sont chiffrés avec
`DONKA_DECISION_LOG_KEY`. Remplacez-la si elle a pu fuiter, si une personne qui la connaissait
part, ou si vous avez démarré avec une clé publique par défaut (le `docker-compose.yml` de la
construction depuis les sources en contient). Chaque
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

**Clé de la seconde étape** : les secrets des applications d'authentification sont scellés avec
`DONKA_SECOND_STEP_KEY`. Pour la remplacer, déplacez la valeur actuelle dans
`DONKA_SECOND_STEP_PREVIOUS_KEYS`, renseignez la nouvelle clé et lancez `docker compose up -d app`.
Chaque secret est rescellé avec la nouvelle clé à la prochaine connexion de son titulaire ; gardez
l'ancienne clé jusqu'à ce que chacun se soit connecté une fois (ou qu'un administrateur ait
réinitialisé la seconde étape des autres).

**Clé de l'index des personnes** : les identifiants (numéro national, téléphone…) sont retrouvés
grâce à des empreintes calculées avec `DONKA_PERSON_INDEX_KEY` ; il n'y a pas de clé précédente.
Pour la remplacer, renseignez la nouvelle clé, videz l'index et indexez de nouveau le journal,
Studio arrêté :

```bash
docker compose stop app
docker compose exec postgres psql -U donka -c "TRUNCATE decision_record_people; \
  UPDATE decision_record_people_backfill SET received_before = now(), after_evaluated_at = NULL, \
  after_id = NULL, finished_at = NULL"
docker compose up -d app
docker compose exec app donka-app decision-log index-people
```

**Mots de passe des personnes** : chacun change le sien avec *Mot de passe oublié ?* sur la
page de connexion.

**Connexion en deux étapes** : chacun peut l'activer dans *Compte*, avec n'importe quelle
application d'authentification (codes de six chiffres renouvelés toutes les 30 secondes).
`DONKA_MFA_REQUIRED=privileged` l'impose aux administrateurs et aux propriétaires de projet, `all`
à tout le monde ; ils la configurent juste après leur mot de passe à leur prochaine connexion. Une
personne qui a perdu son téléphone se connecte avec un de ses codes de secours ; s'il ne lui en
reste plus, un autre administrateur réinitialise sa seconde étape dans *Personnes*, puis elle la
configure à nouveau. Chaque changement est audité et la personne est prévenue par e-mail.

## Dépannage

| Symptôme | Où regarder |
| --- | --- |
| Studio ne démarre pas | `docker compose logs app` : un paramètre manquant ou invalide est nommé, avec un exemple |
| `/api/v1/ready` répond 503 | PostgreSQL : `docker compose ps postgres`, puis `docker compose logs postgres` |
| Un déploiement reste *en attente* | `docker compose logs app` : le publicateur réessaie jusqu'à ce que le stockage des versions réponde |
| Le Runtime répond 404 pour un projet | Il sert un seul environnement (ici staging) ; vérifiez que la version y est en ligne, puis `docker compose logs runtime` |
| Le Runtime redémarre en boucle au premier démarrage | Il attend que Studio écrive son jeton du journal des décisions : `docker compose logs app \| grep decision-log` |
| Studio s'arrête sur `DONKA_PACKS_DIR` | Un pack de ce dossier est invalide : le message nomme le pack et la raison |
| « Le fichier de licence a été refusé » | `docker compose logs app \| grep licence` donne la raison : le fichier a été modifié, a été émis pour une autre installation, ou ne peut pas être lu à `DONKA_LICENCE_FILE` |
| Aucun e-mail | Mailpit (http://localhost:8025) tant que `DONKA_SMTP_URL` ne pointe pas vers votre serveur |
