# Évaluation de trésorerie PME (Cameroun, XAF)

[English](guide.md)

Évalue un découvert ou un crédit court terme pour une petite entreprise : ce que son chiffre
d'affaires peut porter selon son secteur, comment sa trésorerie couvre les remboursements, et son
historique. Elle répond **approve** (accorder), **refer** (un analyste décide) ou **decline**
(refuser), avec une limite recommandée et des motifs en anglais et en français. Les montants sont
en FCFA (XAF).

> **Un point de départ, pas un modèle validé.** Facteurs sectoriels, points, classes et limites
> sont des valeurs d'expert. Votre équipe risques les règle selon votre politique de crédit et les
> vérifie sur votre propre portefeuille PME avant la production. Chaque valeur à revoir se trouve
> dans le nœud **Credit policy**, la table **Sector factors** ou une table de points.

## L'importer

```bash
STUDIO_URL=https://donka.banque.example DONKA_EMAIL=vous@banque.example DONKA_PASSWORD='...' \
scripts/import-pack.sh packs/sme-treasury
```

Cela crée le projet `sme-treasury` avec la décision `evaluation`, une première version et 6
scénarios de test, tous réussis. Importez-le de nouveau sous une autre clé
(`PROJECT_KEY=... PROJECT_NAME=...`) pour en faire un autre produit.

## Ce qu'elle lit

| Champ | Type | Signification |
| --- | --- | --- |
| `business.sector` | `trade`, `services`, `manufacturing`, `agriculture`, `construction` (autres : facteurs par défaut) | Activité principale |
| `business.yearsActive` | nombre | Années d'activité |
| `business.monthlyTurnover` | nombre (XAF) | Chiffre d'affaires mensuel moyen des 6 derniers mois |
| `business.turnoverTrend` | nombre | Chiffre d'affaires des 3 derniers mois ÷ les 3 précédents (1,1 = +10 %) |
| `business.averageBalance` | nombre (XAF) | Solde moyen du compte sur 6 mois |
| `business.monthlyDebtService` | nombre (XAF) | Remboursements des crédits en cours |
| `business.taxRegistered` | booléen | Un numéro d'identifiant unique (NIU) est au dossier |
| `request.product` | `overdraft` ou `short-term-loan` | Découvert ou crédit court terme |
| `request.amount` | nombre (XAF) | Limite ou montant demandé |
| `request.termMonths` | nombre | Durée d'un crédit court terme |
| `bureau.checked` | booléen | Un bureau de crédit a été consulté |
| `bureau.paymentIncidents` | nombre | Incidents de paiement (par exemple chèques impayés), 12 mois |
| `bureau.worstDaysPastDue` | nombre | Pire retard signalé, en jours |

## Ce qu'elle répond

```json
{
  "decision": "approve",
  "band": "A",
  "score": 130,
  "recommendedLimit": 2000000,
  "capacityLimit": 2000000,
  "debtServiceCover": 2.14,
  "reasons": [
    { "code": "R71", "en": "The request is above the business's capacity: the limit is reduced.", "fr": "La demande dépasse la capacité de l'entreprise : la limite est réduite." }
  ],
  "points": { "years": 25, "trend": 25, "debtServiceCover": 30, "balanceCover": 20, "taxRegistered": 10, "history": 20 }
}
```

`recommendedLimit` est le plus petit entre le montant demandé et la capacité de l'entreprise, et
0 quand la réponse est un refus.

## Comment elle décide

1. **Politique de crédit** et **facteurs sectoriels** : la part du chiffre d'affaires mensuel qui
   peut devenir une limite, et la marge de trésorerie de chaque secteur.
2. **Nouveau service de la dette** : intérêts d'un découvert, ou mensualité d'un crédit, au taux
   de la politique.
3. **Couverture** : couverture du service de la dette (trésorerie mensuelle ÷ tous les
   remboursements) et couverture par le solde (solde moyen ÷ tous les remboursements).
4. **Points** : ancienneté, tendance du chiffre d'affaires, couverture du service de la dette,
   couverture par le solde, NIU et historique de crédit (130 points au maximum).
5. **Classe** : A ≥ 100 et B ≥ 80 accordent, C ≥ 60 renvoie à un analyste, D refuse.
6. **Décision** : une exclusion refuse ; un accord sans consultation du bureau, ou avec une
   couverture sous le minimum de la politique, devient un renvoi à l'analyste.

### Politique de crédit (à confirmer)

| Paramètre | Défaut | Signification |
| --- | --- | --- |
| `minYearsActive` | 2 | Ancienneté minimale |
| `minDebtServiceCover` | 1,2 | En dessous, un accord passe par un analyste |
| `maxDaysPastDue` | 90 | Au-delà de ce retard, refus |
| `maxPaymentIncidents` | 2 | Au-delà, refus |
| `annualRate` | 0,14 | Taux nominal annuel du service de la dette |

| Secteur | Part du chiffre d'affaires | Marge de trésorerie |
| --- | --- | --- |
| trade (commerce) | 30 % | 10 % |
| services | 25 % | 20 % |
| manufacturing (industrie) | 25 % | 15 % |
| agriculture | 20 % | 15 % |
| construction (BTP) | 20 % | 12 % |
| autres | 15 % | 10 % |

### Codes d'exclusion et motifs

| Code | Signification |
| --- | --- |
| K11 | Entreprise plus jeune que le minimum |
| K12 | Aucun chiffre d'affaires enregistré |
| K13 | La trésorerie ne couvre pas les remboursements (couverture inférieure à 1) |
| K14 | Impayés graves au bureau de crédit |
| K15 | Trop d'incidents de paiement |
| R60 | Pas de consultation du bureau : un analyste doit revoir le dossier |
| R61 | Le chiffre d'affaires baisse |
| R62 | Couverture des remboursements sous le minimum : un analyste doit revoir le dossier |
| R63 | Solde moyen faible |
| R64 | Pas de NIU au dossier |
| R65 | Retards de paiement au cours des 24 derniers mois |
| R66 | Entreprise jeune |
| R71 | Demande au-delà de la capacité : la limite est réduite |

## L'adapter

- Fixez les facteurs sectoriels d'après votre propre portefeuille PME : les activités
  saisonnières (agriculture) demandent souvent une part plus faible ou un contrôle saisonnier.
- Changez une valeur et enregistrez une version : les 6 scénarios montrent ce qui bouge.
