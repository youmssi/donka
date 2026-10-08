# Crédit aux particuliers (Cameroun, XAF)

[English](guide.md)

Une grille de score de prêt personnel pour démarrer : elle vérifie la capacité de remboursement,
note le demandeur, le place dans une classe et répond **approve** (accorder), **refer** (un
analyste décide) ou **decline** (refuser), avec des motifs en anglais et en français. Les montants
sont en FCFA (XAF), sans décimales.

> **Un point de départ, pas un modèle validé.** Les points, classes et limites sont des valeurs
> d'expert choisies pour être plausibles chez un prêteur camerounais. Avant la production, votre
> équipe risques les règle selon votre politique de crédit et vérifie les classes sur votre propre
> portefeuille. Chaque valeur à revoir se trouve dans le nœud **Credit policy** ou dans une table
> de points.

## L'importer

```bash
STUDIO_URL=https://donka.banque.example DONKA_EMAIL=vous@banque.example DONKA_PASSWORD='...' \
scripts/import-pack.sh packs/retail-credit
```

Cela crée le projet `retail-credit` avec deux décisions, une première version de chacune et 8
scénarios de test, tous réussis. C'est votre projet : modifiez tout. Pour démarrer un autre
produit à partir du même pack, importez-le de nouveau sous une autre clé :
`PROJECT_KEY=avance-salaire PROJECT_NAME="Avance sur salaire" scripts/import-pack.sh packs/retail-credit`.

## Ce qu'elle lit

| Champ | Type | Signification |
| --- | --- | --- |
| `applicant.age` | nombre | Âge en années |
| `applicant.employment` | `public`, `private`, `self-employed`, `informal` | Type de revenu |
| `applicant.monthsInJob` | nombre | Mois chez l'employeur ou dans l'activité actuels |
| `applicant.monthlyIncome` | nombre (XAF) | Revenu mensuel net |
| `applicant.monthlyDebtPayments` | nombre (XAF) | Remboursements des crédits en cours |
| `applicant.salaryDomiciled` | booléen | Le salaire est versé sur un compte de la banque |
| `loan.amount` | nombre (XAF) | Montant demandé |
| `loan.termMonths` | nombre | Durée en mois |
| `bureau.checked` | booléen | Un bureau de crédit a été consulté pour cette demande |
| `bureau.paymentIncidents` | nombre | Incidents de paiement signalés (24 mois) |
| `bureau.activeLoans` | nombre | Crédits en cours ailleurs |
| `bureau.worstDaysPastDue` | nombre | Pire retard signalé, en jours (24 mois) |

Les champs `bureau` sont remplis par votre système à partir de la source consultée (un bureau de
crédit privé ou la centrale des risques de la banque centrale). Si vous préférez interroger le
bureau depuis le Runtime, remplacez-les par un nœud connecteur (voir le guide des connecteurs du
Runtime).

## Ce qu'elle répond

```json
{
  "decision": "approve",
  "band": "A",
  "score": 143,
  "reasons": [],
  "installment": 87893,
  "debtToIncome": 0.284,
  "maxAmount": 3980000,
  "points": { "age": 25, "employment": 30, "tenure": 18, "debtToIncome": 20, "history": 30, "salaryDomiciled": 20 }
}
```

`reasons` liste d'abord les codes d'exclusion (`K..`), puis les motifs (`R..`), chacun avec
`code`, `en` et `fr`. `maxAmount` est le plus grand prêt que le demandeur peut rembourser sur
cette durée.

## Comment elle décide

1. **Politique de crédit** : les valeurs que votre équipe risques fixe (ci-dessous).
2. **Capacité de remboursement** (une décision à part, utilisable seule) : mensualité au taux de
   la politique, taux d'endettement après le nouveau prêt, et plus grand montant remboursable.
3. **Points** : âge, type d'emploi, ancienneté, taux d'endettement, historique de crédit et
   domiciliation du salaire, une table de décision chacun (160 points au maximum).
4. **Classe** : A ≥ 120 et B ≥ 100 accordent, C ≥ 80 renvoie à un analyste, D et E refusent.
5. **Règles d'exclusion** : une seule suffit à refuser, quel que soit le score.
6. **Décision** : une exclusion refuse ; sans consultation du bureau, un accord devient un renvoi
   à l'analyste.

### Politique de crédit (à confirmer)

| Paramètre | Défaut | Signification |
| --- | --- | --- |
| `minAge` | 21 | Âge minimum |
| `maxAgeAtMaturity` | 65 | Âge maximum à la dernière échéance |
| `minMonthlyIncome` | 75 000 | Revenu mensuel net minimum (XAF) |
| `maxDebtToIncome` | 0,40 | Part maximale du revenu consacrée aux remboursements |
| `maxDaysPastDue` | 90 | Au-delà de ce retard, refus |
| `maxTermMonths` | 60 | Durée maximale |
| `maxAmount` | 10 000 000 | Montant maximum (XAF) |
| `annualRate` | 0,16 | Taux nominal annuel du calcul de la mensualité ; restez sous le taux maximum légal |

### Codes d'exclusion et motifs

| Code | Signification |
| --- | --- |
| K01 | Âge inférieur au minimum |
| K02 | Le prêt se terminerait après l'âge maximum |
| K03 | Revenu inférieur au minimum |
| K04 | Remboursements au-delà du taux d'endettement maximum |
| K05 | Impayés graves au bureau de crédit |
| K06 | Durée supérieure au maximum |
| K07 | Montant supérieur au maximum |
| R10 | Taux d'endettement élevé |
| R20 | Retards de paiement au cours des 24 derniers mois |
| R21 | Plusieurs crédits déjà en cours |
| R30 | Moins d'un an dans l'emploi actuel |
| R40 | Revenus d'une activité indépendante ou informelle |
| R50 | Salaire non domicilié dans la banque |
| R60 | Pas de consultation du bureau : un analyste doit revoir le dossier |

## L'adapter

- Changez une valeur dans le nœud **Credit policy** et enregistrez une version : les 8 scénarios
  s'exécutent et montrent ce que votre changement déplace.
- Ajoutez un critère : ajoutez une table de points, ajoutez ses points dans le nœud **Score** et
  élargissez les classes.
- Avant la mise en production, passez un échantillon de demandes passées dans le simulateur et
  comparez les classes avec ce qu'il est advenu de ces prêts.
