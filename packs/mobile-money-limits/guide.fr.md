# Plafonds mobile money par niveau (CEMAC, XAF)

[English](guide.md)

Autorise ou refuse une opération mobile money selon les plafonds du niveau KYC du portefeuille :
solde maximal, plafond par opération, sorties journalières et mensuelles, et transferts vers un
autre pays de la CEMAC. Elle répond **allow** (autoriser) ou **decline** (refuser) avec les
frais, le solde après l'opération, ce qui reste des plafonds journalier et mensuel, des motifs en
anglais et en français, et le niveau qui autoriserait une opération refusée. Montants en FCFA
(XAF).

> **Un point de départ, pas un modèle validé.** Plafonds et frais sont des valeurs d'exemple.
> Réglez-les selon votre agrément, les règles de la BEAC sur les services de paiement et votre
> grille tarifaire avant la production. Chaque valeur à revoir est dans les tables **Limits by
> tier** et **Fees**.

## L'importer

Dans Studio, **Projets → Depuis un pack → Plafonds mobile money par niveau**, puis choisissez le
nom et la clé du projet. Cela crée un projet avec la décision `transaction-limits`, une première
version et 9 scénarios de test, tous réussis.

## Ce qu'elle lit

| Champ | Type | Sens |
| --- | --- | --- |
| `wallet.tier` | 1, 2 ou 3 | Niveau KYC : 1 numéro et identité déclarée, 2 pièce d'identité vérifiée, 3 KYC complet |
| `wallet.balance` | nombre (XAF) | Solde avant l'opération |
| `wallet.dailyOutgoing` | nombre (XAF) | Déjà envoyé ou retiré aujourd'hui |
| `wallet.monthlyOutgoing` | nombre (XAF) | Déjà envoyé ou retiré ce mois-ci |
| `transaction.type` | `deposit`, `transfer`, `withdrawal`, `merchant-payment` | Dépôt, transfert, retrait, paiement marchand |
| `transaction.amount` | nombre (XAF) | Montant |
| `transaction.crossBorder` | booléen | Vers un autre pays de la CEMAC |

## Ce qu'elle répond

`decision`, `fee`, `balanceAfter`, `dailyRemaining`, `monthlyRemaining`, `reasons` (code, `en`,
`fr`) et `upgradeTo`. Une opération refusée laisse le solde et les plafonds inchangés.
`upgradeTo` n'apparaît que si un niveau supérieur l'autoriserait : jamais pour un solde
insuffisant.

## Comment elle décide

1. **Limits by tier** et **Fees** pour l'opération.
2. **After the transaction** : solde et sorties du jour et du mois tels qu'ils seraient.
3. **Limit checks** : chaque plafond dépassé donne un motif. Les dépôts ne rencontrent que le solde
   maximal ; transferts, retraits et paiements rencontrent les autres.
4. **Décision** : autorisée quand aucun plafond n'est dépassé.

### Plafonds et frais (à confirmer)

| Niveau | Solde max. | Par opération | Sorties / jour | Sorties / mois | Transfrontalier |
| --- | --- | --- | --- | --- | --- |
| 1 | 500 000 | 200 000 | 300 000 | 1 000 000 | non |
| 2 | 2 000 000 | 1 000 000 | 1 500 000 | 5 000 000 | oui |
| 3 | 10 000 000 | 5 000 000 | 7 500 000 | 25 000 000 | oui |

| Montant jusqu'à | Transfert | Retrait |
| --- | --- | --- |
| 5 000 | 50 | 100 |
| 50 000 | 250 | 500 |
| 300 000 | 1 000 | 1 500 |
| au-delà | 2 500 | 3 500 |

Dépôts et paiements marchands sont gratuits pour le client.

### Motifs

| Code | Sens |
| --- | --- |
| M01 | Au-dessus du plafond par opération du niveau |
| M02 | Au-dessus du plafond journalier du niveau |
| M03 | Au-dessus du plafond mensuel du niveau |
| M04 | Le solde ne couvre pas le montant et les frais |
| M05 | Le solde dépasserait le plafond du niveau |
| M06 | Les transferts vers un autre pays de la CEMAC demandent le niveau 2 ou plus |

## L'adapter

- Ajoutez une ligne à **Limits by tier** pour un portefeuille agent ou marchand, avec son propre
  numéro de niveau.
- Changez une valeur, enregistrez une version : les 9 scénarios montrent ce qui bouge.
