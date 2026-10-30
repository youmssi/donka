# Performance

[English](performance.md)

Ce que le Runtime et le flux du journal des décisions tiennent, comment c'est mesuré, et comment
le mesurer sur vos propres serveurs. Les objectifs et la référence sont dans
[`performance.json`](performance.json) ; la CI vérifie chaque pull request par rapport à eux.

## Objectifs

| Mesure | Objectif |
| --- | --- |
| Charge de pointe | 100 décisions par seconde, 100 000 par jour |
| Latence du Runtime à 100/s, décision sans connecteur | p95 ≤ 20 ms, p99 ≤ 50 ms, aucune erreur |
| Flux du journal des décisions | chaque décision rendue à 100/s est dans le journal de Studio en moins de 5 s |
| Studio arrêté, dans la file du Runtime | aucune décision en échec, aucun enregistrement perdu |
| Studio arrêté, au-delà de la file | aucune décision en échec ; les enregistrements abandonnés sont comptés dans le journal du Runtime |
| Recherche et rejeu du journal dans Studio | p95 1 s et 2 s (mesurés dans un changement ultérieur) |
| Régression | un débit maximal inférieur de plus de 20 % à sa référence fait échouer la CI |

La latence est mesurée à partir du moment où chaque requête était prévue, pas de celui où elle
est partie : un Runtime lent ne peut pas masquer sa propre file d'attente.

## Résultats

Machine de référence : un runner GitHub `ubuntu-latest` (4 vCPU, 16 Go), avec Studio, le Runtime,
PostgreSQL et le générateur de charge sur la même machine, installés depuis les images publiées.
Exécution courte de la CI du 30 octobre 2026 : 15 s à 100/s après 3 s de chauffe, puis 10 s au
débit maximal avec 32 appelants simultanés. Les entrées sont les scénarios de test de chaque pack.

| Pack / décision | p50 à 100/s | p95 | p99 | Débit maximal | p95 au maximum |
| --- | --- | --- | --- | --- | --- |
| kyc-risk-rating / risk-rating | 2,2 ms | 2,8 ms | 3,1 ms | 3 747/s | 14,5 ms |
| mobile-money-limits / transaction-limits | 1,6 ms | 11,1 ms | 19,1 ms | 6 145/s | 9,2 ms |
| retail-credit / scorecard | 2,5 ms | 14,0 ms | 22,5 ms | 2 861/s | 18,9 ms |
| sme-treasury / evaluation | 2,3 ms | 12,7 ms | 21,1 ms | 2 932/s | 18,8 ms |
| retail-credit / credit-application, avec un bureau simulé | 2,6 ms | 4,5 ms | 16,1 ms | 2 042/s | 30,0 ms |

Aucune requête n'a échoué. Sur un runner partagé, la latence à 100/s varie d'une exécution à
l'autre (p99 de 3 à 20 ms pour la même décision), toujours bien en deçà de l'objectif : la CI la
compare donc aux objectifs, et ne compare à la référence que le débit maximal.

**Flux.** 1 500 décisions à 100/s : la dernière était dans le journal de Studio 1,1 s après la
fin de la charge.

**Studio arrêté.** 30 s à 100/s, Studio arrêté pendant 15 s au milieu :

| File du Runtime | Décisions en échec | p99 | Enregistrements perdus |
| --- | --- | --- | --- |
| 10 000 (par défaut) | 0 | 2,8 ms | 0 |
| 500 | 0 | 2,8 ms | 2 200 sur 3 000, comptés dans le journal du Runtime |

Le Runtime n'attend jamais Studio : quand sa file est pleine, il abandonne les nouveaux
enregistrements et journalise leur nombre (`"dropped"`, à chaque puissance de deux), de sorte que
les décisions gardent leur latence. Dimensionnez `DECISION_LOG__QUEUE_CAPACITY` pour la plus
longue indisponibilité de Studio que vous prévoyez : à 100/s, les 10 000 par défaut couvrent
100 secondes.

## Le mesurer vous-même

Sur un serveur de test, depuis un clone de ce dépôt : `scripts/check-install.sh` installe Donka
depuis les images avec la démo (son faux bureau compris), puis `scripts/check-performance.sh` la
charge. Utilisez un serveur dédié, pas votre installation de production.

```bash
DONKA_INSTALL_DIR=/opt/donka scripts/check-install.sh <tag studio> <tag runtime> demo
scripts/check-performance.sh /opt/donka full     # 60 s à 100/s, 30 s au maximum
```

Les résultats vont dans `performance.json` à la racine du dépôt et sont vérifiés par rapport aux
objectifs. Pour charger une de vos propres décisions, `donka-admin load-test` dans l'image de
Studio fait la même mesure :

```bash
docker compose run --rm -e DONKA_RUNTIME_TOKEN=<jeton> -v "$PWD/inputs.json:/inputs.json:ro" app \
  donka-admin load-test --url http://runtime:8080/api/projects/<projet>/evaluate/<décision> \
  --inputs /inputs.json --rate 100 --seconds 60
```

`--rate` envoie selon un rythme fixe et donne la latence à cette charge ; `--concurrency <n>`
envoie aussi vite que le Runtime répond et donne le débit maximal. `inputs.json` est un tableau
JSON d'entrées, envoyées tour à tour.
