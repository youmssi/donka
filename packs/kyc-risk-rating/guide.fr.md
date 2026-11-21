# Notation du risque KYC (CEMAC)

[English](guide.md)

Note le risque de blanchiment et de financement du terrorisme d'un client, à l'entrée en relation
et à chaque revue : **faible** (`low`), **moyen** (`medium`), **élevé** (`high`) ou **interdit**
(`prohibited`). Elle donne le niveau de vigilance (simplifiée, standard, renforcée), le nombre de
mois avant la prochaine revue, et répond **accept** (accepter), **refer** (la conformité décide)
ou **reject** (refuser), avec des motifs en anglais et en français.

> **Un point de départ, pas un modèle validé.** Points, seuils et listes de pays sont des valeurs
> d'expert. Votre responsable conformité les règle selon les règles LBC/FT de la CEMAC, les
> instructions de la COBAC et votre propre évaluation des risques avant la production. Chaque
> valeur à revoir est dans le nœud **AML policy** ou dans une table de points.

## L'importer

Dans Studio, **Projets → Depuis un pack → Notation du risque KYC**, puis choisissez le nom et la
clé du projet. Cela crée un projet avec la décision `risk-rating`, une première version et 10
scénarios de test, tous réussis. Importez-le de nouveau sous une autre clé pour lancer un autre
produit.

## Ce qu'elle lit

| Champ | Type | Sens |
| --- | --- | --- |
| `customer.type` | `individual` ou `business` | Particulier ou entreprise |
| `customer.residence` | code pays ISO | Pays de résidence |
| `customer.nationality` | code pays ISO | Nationalité, ou pays d'immatriculation d'une entreprise |
| `customer.activity` | `salaried`, `civil-servant`, `self-employed`, `trader`, `ngo`, `cash-intensive`, `other` | Profession ou activité |
| `customer.pep` | booléen | Personne politiquement exposée, membre de sa famille ou proche |
| `customer.yearsKnown` | nombre | Ancienneté de la relation en années (0 à l'entrée) |
| `customer.beneficialOwnersIdentified` | booléen, entreprises | Toute personne détenant ou contrôlant 25 % ou plus est identifiée |
| `account.channel` | `branch`, `agent` ou `online` | Agence, agent ou à distance |
| `account.expectedMonthlyVolume` | nombre (XAF) | Volume mensuel attendu |
| `screening.sanctionsHit` | booléen | Correspondance confirmée sur une liste de sanctions |
| `screening.adverseMedia` | booléen | Informations négatives dans les médias |
| `documents.identityVerified` | booléen | Pièce d'identité vérifiée |
| `documents.addressVerified` | booléen | Adresse vérifiée |

Ces champs forment le contrat d'entrée de la décision (**Champs d'entrée** dans l'éditeur) : le
Runtime refuse une requête à laquelle il en manque un par `400`, en nommant le champ.

## Ce qu'elle répond

`decision`, `rating`, `dueDiligence` (`simplified`, `standard`, `enhanced`, `none`),
`reviewMonths`, `score`, `reasons` (code, `en`, `fr`) et le détail des `points`, par exemple
pour un changeur de devises entré à distance avec un volume élevé : `refer`, `high`,
`enhanced`, 12 mois, 60 points, motifs R04, R06 et R07.

## Comment elle décide

1. **Géographie** : résidence ou nationalité dans un pays à haut risque, ou résidence hors CEMAC.
2. **Points** : activité, géographie, PPE, canal, volume attendu, relation récente et
   informations négatives s'additionnent.
3. **Notation** : une correspondance sur une liste de sanctions est interdite ; une PPE ou un pays
   à haut risque est toujours élevé ; sinon élevé dès 60 points, moyen dès 30, faible en dessous.
4. **Vigilance et revue** : faible = simplifiée, revue après 36 mois ; moyen = standard, 24 mois ;
   élevé = renforcée, 12 mois.
5. **Décision** : interdit refuse ; élevé, bénéficiaires effectifs manquants ou pièce non vérifiée
   renvoie à la conformité ; sinon accepte.

### Politique LBC/FT (à confirmer)

| Réglage | Défaut | Sens |
| --- | --- | --- |
| `cemac` | CM, CF, CG, GA, GQ, TD | Pays traités comme marché domestique |
| `highRiskCountries` | IR, KP, MM | Pays visés par un appel à l'action du GAFI ; à tenir à jour |
| `highVolume` | 50 000 000 | Volume mensuel attendu jugé élevé |
| `mediumVolume` | 10 000 000 | Volume mensuel attendu jugé moyen |
| `mediumFrom` | 30 | Score à partir duquel la notation est moyenne |
| `highFrom` | 60 | Score à partir duquel la notation est élevée |

| Facteur | Points |
| --- | --- |
| Activité : espèces / ONG / commerçant / indépendant ou autre | 30 / 20 / 15 / 10 |
| Pays à haut risque / résidence hors CEMAC | 40 / 15 |
| PPE | 40 |
| Canal : à distance / agent | 10 / 5 |
| Volume attendu : élevé / moyen | 20 / 10 |
| Client depuis moins d'un an | 10 |
| Informations négatives | 30 |

### Motifs

| Code | Sens |
| --- | --- |
| K01 | Correspondance confirmée sur une liste de sanctions : relation impossible |
| R01 | Personne politiquement exposée : vigilance renforcée et accord de la direction |
| R02 | Résident ou ressortissant d'un pays à haut risque |
| R03 | Résident hors de la CEMAC |
| R04 | Activité à forte intensité d'espèces |
| R05 | Informations négatives dans les médias |
| R06 | Entrée en relation à distance |
| R07 | Volume attendu élevé |
| R08 | Pièce d'identité non vérifiée |
| R09 | Adresse non vérifiée |
| R10 | Bénéficiaires effectifs non identifiés |

## L'adapter

- Tenez les listes de pays à jour avec les déclarations du GAFI et votre évaluation nationale des
  risques.
- Le filtrage sur les listes de sanctions se fait avant cette décision : elle en lit le résultat.
- Changez une valeur, enregistrez une version : les 10 scénarios montrent ce qui bouge.
