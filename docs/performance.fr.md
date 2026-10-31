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
| Recherche et rejeu du journal dans Studio | p95 1 s et 2 s sur l'échantillon documenté |
| Régression | débit maximal Runtime inférieur de plus de 20 % à sa référence, ou p95 global Studio supérieur de plus de 20 %, fait échouer la CI |

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

## Recherche et rejeu dans Studio

La mesure de Studio passe **avant** la charge du Runtime et les tests de panne, sur la même
installation isolée construite à partir des images. Elle envoie des décisions synthétiques au
vrai flux `/api/v1/decision-log/records`, par lots de 1 000 au maximum : Studio les valide,
chiffre leurs données et crée les partitions mensuelles. Aucune insertion SQL directe ne
contourne ce chemin. Chaque contrôle crée un projet synthétique séparé : les décisions de
calibrage du Runtime arrivant en différé ne changent pas son volume. La sortie vient d'une évaluation réelle de la décision d'entrée du pack
KYC. Une décision sur dix est un échec synthétique pour vérifier le filtre par statut.

La CI courte insère **100 000 décisions sur 60 mois civils** ; `full` en insère **1 000 000**.
Les lignes sont réparties uniformément, le reste dans les mois récents. Les dates historiques
sont au milieu du mois pour rester dans la rétention de 1 825 jours, même en fin de mois ou
après un 29 février. Ce sont des
**échantillons**, pas une certification de capacité sur cinq ans et 180 millions de décisions :
ils ne reproduisent ni 100 000 décisions par jour ni la variété des tailles de données clients.
Une mesure sur tout le volume exige une machine et un jeu de données adaptés avant de pouvoir
annoncer cette capacité. `sample.studio` dans `performance.json` fixe les tailles, la période,
le nombre de requêtes et l'échauffement.

Chaque cas envoie désormais **trois séries de 1 000 requêtes mesurées**, après 50 échauffements
par série, dans les deux modes. Les 3 000 mesures sont réunies pour calculer p50/p95/p99 ;
aucune série n'est écartée ou choisie parce qu'elle est plus rapide. Les cas sont : première
page récente, recherche par référence, décision avec période récente, statut en échec et rejeu
de décisions réussies réparties entre partitions anciennes et récentes. La recherche comprend
la page et son **nombre total exact**, la transaction, le JSON et le transfert réseau ; ce
n'est pas seulement la durée d'un accès à l'index. Le rejeu comprend ouverture, déchiffrement,
chargement de la release enregistrée, évaluation et audit. Un statut HTTP réussi ne suffit pas :
les filtres doivent renvoyer les bonnes décisions et le rejeu doit rendre la même sortie.

`studio.search` (un rapport par filtre) et `studio.replay` contiennent p50/p95/p99, maximum,
nombre de requêtes et erreurs dans le même artefact et dans un avis Studio performance du contrôle CI. `studio.corpus` donne les totaux réels
avant et après insertion et le nombre de lignes par mois ; `studio.machine` précise le modèle
de processeur. L'artefact conserve chaque partition ; l'avis garde leur nombre et tous les
résumés de durée pour rester sous la limite GitHub de 4 096 caractères. Un avis Runtime performance
séparé conserve latence, débit, flux et pannes ; un contrôle refusé indique sa cause dans une annotation.
Chaque cas conserve aussi les résumés de ses séries dans `rounds`. Toute erreur, un p95 de
recherche supérieur à 1 000 ms ou de rejeu supérieur à 2 000 ms, globalement ou dans une série,
fait échouer le contrôle.

`baseline.studio` décrit l'échantillon et le volume réel, avec une entrée `profiles` pour
chaque modèle de processeur mesuré. Une comparaison exige le même modèle de processeur,
nombre de cœurs virtuels, architecture et version majeure de Node, ainsi que des réglages
d'échantillon et volumes identiques. La casse et les espaces supplémentaires du nom de
processeur sont ignorés ; les mises à jour mineures de Node restent comparables.
La **limite de 20 % sur p95 reste inchangée** pour chaque recherche et le rejeu.
Le choix matériel est consigné dans [ADR-008](adr/008-hardware-matched-performance-baselines.md).

Chaque exécution CI exige un profil comparable : processeur inconnu, volume ou nombre de
requêtes différent, mesure manquante ou profil ambigu font échouer le contrôle après
l'enregistrement du résultat. Sans profil comparable, une exécution locale vérifie toujours
tous les objectifs absolus et indique explicitement l'absence de comparaison à une référence.
Le mode local `full` peut donc mesurer son volume supérieur sans le comparer à la référence
sur 100 000 décisions.

### Références par processeur, 30 octobre 2026

Trois installations indépendantes depuis les images, sur GitHub `ubuntu-latest` (4 vCPU,
16 Go, x64, Node 22.23.4), contiennent chacune exactement **100 000 décisions dans 60 partitions
mensuelles**, dans un projet séparé initialement vide. Chaque cas mesure 3 000 requêtes en trois
séries de 1 000, après 50 échauffements par série. Les **45 000 requêtes mesurées réussissent**,
et chaque rejeu rend la sortie enregistrée. Les contrôles Runtime, flux et pannes passent aussi
sur les trois installations.

| Cas, p95 global | AMD EPYC 7763 | AMD EPYC 9V74 | Intel Xeon Platinum 8573C |
| --- | --- | --- | --- |
| Page récente | 21,78 ms | 19,07 ms | 15,08 ms |
| Référence | 5,57 ms | 4,28 ms | 3,96 ms |
| Décision et période récente | 5,00 ms | 3,10 ms | 3,40 ms |
| Statut en échec | 39,64 ms | 35,54 ms | 23,30 ms |
| Rejeu | 6,06 ms | 4,36 ms | 4,24 ms |

Sources au commit `dbcf8f1` :

- [Exécution 1, EPYC 7763](https://github.com/youmssi/donka/actions/runs/41121411427/attempts/1).
- [Exécution 2, EPYC 9V74](https://github.com/youmssi/donka/actions/runs/41121411427/attempts/2).
- [Exécution 3, Xeon 8573C](https://github.com/youmssi/donka/actions/runs/41121411427/attempts/3).

Chaque colonne fournit son propre profil dans `performance.json`, avec le p95 de l'ensemble
des trois séries de ce processeur. Les modèles de processeur ne sont pas moyennés ensemble.
L'insertion est exclue des durées de requête ; p50/p95/p99, maxima, séries, volumes et métadonnées
machine restent dans l'artefact et l'avis compact du contrôle. Les exécutions CI suivantes
comparent au profil enregistré correspondant au matériel attribué par GitHub.

Le premier essai sur 50 requêtes a vu son p95 de rejeu passer de
[8,47 ms](https://github.com/youmssi/donka/actions/runs/41084840154) à
[13,40 ms](https://github.com/youmssi/donka/actions/runs/41088061492/attempts/1), refusé par la
limite de 20 % malgré le respect de l'objectif absolu. Cela a conduit aux séries plus longues
et répétées. Même avec ces séries, une moyenne des trois processeurs ci-dessus ferait dépasser
la moyenne commune de 21 à 31 % pour quatre cas sans changement de code. Les profils séparés
conservent la limite de 20 % sans prendre un autre modèle de processeur comme référence.
Un modèle identique ne fixe pas toutes les variations d'ordonnancement ou de charge de l'hôte ;
le profil mesuré et les objectifs absolus restent tous deux imposés.

Pour calibrer un nouveau modèle, examiner le rapport complet de la CI refusée : projet
initialement vide, volume et réglages attendus, trois séries, aucune erreur et tous les
objectifs absolus respectés. Ajouter le profil matériel mesuré et sa source dans
`performance.json` par le processus habituel de PR, puis le vérifier avec la CI. Ne pas
reprendre les valeurs d'un autre modèle ni relever un profil existant pour masquer une
régression. Le résultat est enregistré avant le refus pour profil manquant : aucun contrôle
ne doit être désactivé pour calibrer.

Le critère de publication DNK-66 reste ouvert tant que le site produit ne publie pas ces
résultats. Cet échantillon ne prouve pas la capacité sur tout le volume de rétention.

L'outil demande Node.js 22 ou ultérieur, sans paquet à installer. `check-performance.sh` fixe
ses variables `DONKA_PERFORMANCE_*` (listées dans `.env.example`) puis appelle
`scripts/studio-performance.mjs --disposable`. Cette option confirme l'**ajout de décisions
synthétiques** : ne jamais utiliser une base client. Chaque exécution émet puis révoque son
propre jeton de flux et ferme sa session. Les lignes restent dans l'installation jetable pour
inspection ; une nouvelle exécution signale le volume augmenté au lieu de le masquer.
