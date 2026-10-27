# Questionnaire d'intégration du pilote

[English](pilot-questionnaire.md)

À remplir avec les équipes de l'institution (SI, sécurité, risque de crédit, exploitation) avant
l'installation. Chaque réponse ouvre ou ferme un travail précis ; la colonne « Pourquoi » le dit.
Une réponse « je ne sais pas » est utile : elle devient une question à suivre.

## 1. Périmètre du pilote

| Question | Pourquoi |
|---|---|
| Quel produit de crédit (ou autre décision) le pilote couvre-t-il ? Volume mensuel de demandes ? | Choix du pack de départ, dimensionnement |
| Qui écrit les règles, qui les approuve, qui les exploite ? Combien de personnes ? | Rôles et comptes dans Studio |
| Quels canaux demandent une décision (agence, mobile, web, back-office) ? Délai de réponse attendu ? | Appels au Runtime, objectifs de latence |
| Quels critères diront que le pilote est réussi ? | Critères de sortie de la Phase 1 |

## 2. Hébergement et réseau

| Question | Pourquoi |
|---|---|
| Serveurs disponibles : nombre, système, CPU, mémoire, disque ? Docker est-il autorisé ? | Installation (DNK-64) |
| Studio et le Runtime de production seront-ils sur la même machine ou dans des zones réseau différentes ? | Stockage des releases : volume disque ou S3 |
| Existe-t-il déjà un stockage S3 (Ceph, NetApp, Dell ECS, MinIO, cloud) ? | S3 de la banque plutôt qu'un nouveau service |
| Les serveurs ont-ils accès à internet ? Un registre ou miroir d'images interne existe-t-il ? | Images en ligne ou archive hors ligne |
| Proxy sortant, reverse proxy, certificats TLS internes ? | Exposition de Studio et du Runtime |
| Base PostgreSQL fournie par la banque ou conteneur Donka ? | Sauvegarde et exploitation de la base |

## 3. Identité et accès

| Question | Pourquoi |
|---|---|
| Fournisseur d'identité utilisé (Entra ID, AD FS, Keycloak, Okta, autre) ? Supporte-t-il OpenID Connect ? | SSO (DNK-60) |
| Politique de double authentification exigée ? | Politique TOTP (DNK-59) |
| Quels groupes de l'annuaire correspondent aux administrateurs de Donka ? | Correspondance groupes et rôles |

## 4. Core banking (CBS)

| Question | Pourquoi |
|---|---|
| Produit et version du CBS ? | Préparer l'intégration |
| Comment expose-t-il les données : API REST, SOAP, ESB ou passerelle, fichiers ? | Appel direct ou via la couche d'intégration de la banque |
| Authentification : OAuth2 (quel flux, URL du jeton, scopes), mTLS (quelle autorité de certification), clé API ? | Connecteurs (DNK-63) |
| Un environnement de test du CBS est-il accessible depuis le serveur Donka ? Avec des données fictives ? | Tests avant production |
| Quelles données sont nécessaires à la décision : encours, retards, ancienneté, revenus domiciliés… ? Sous quels noms ? | Contrat d'entrée et correspondance des champs |
| Limites d'appels et temps de réponse garantis ? | Délais, reprises et disjoncteur du connecteur |

## 5. Bureaux de crédit, KYC et AML

| Question | Pourquoi |
|---|---|
| Quels services externes appeler (bureau, KYC, listes de sanctions) ? Documentation d'API ? | Connecteurs supplémentaires |
| Contrats, environnements de test et identifiants disponibles ? | Planning d'intégration |

## 6. Politique de crédit et données

| Question | Pourquoi |
|---|---|
| Existe-t-il un scorecard ou une grille actuelle ? Sous quelle forme (document, tableur, outil) ? | Import dans Studio |
| Seuils, plafonds, exceptions et circuit d'approbation actuels ? | Règles et approbations |
| Historique de crédits avec les remboursements connus (défauts à 90 jours) ? Combien de dossiers et de défauts, sur quelle période ? | Faisabilité d'un scorecard statistique (DNK-61) et suivi de performance |

## 7. Données personnelles, sécurité et conformité

| Question | Pourquoi |
|---|---|
| Pays, superviseur bancaire (COBAC, Commission Bancaire de l'UMOA, BCRG…) et autorité de protection des données ? | Matrices de conformité (DNK-73), droits des personnes (DNK-75) |
| Règles internes et réglementaires applicables (protection des données, régulateur bancaire) ? | Chiffrement, masquage, conservation |
| Durée de conservation des décisions exigée ? | `DONKA_DECISION_LOG_RETENTION_DAYS` |
| Où doivent être gardées les clés de chiffrement (coffre, HSM, fichiers) ? | Secrets (DNK-65) |
| Les explications en langage clair sont-elles souhaitées ? Avec quel modèle (hébergé, interne) ? | Configuration LLM (DNK-19) |
| Questionnaire de sécurité ou revue d'architecture à prévoir ? | Section sécurité de la documentation (DNK-66) |

## 8. Exploitation

| Question | Pourquoi |
|---|---|
| Processus de changement (comité, fenêtres de maintenance) ? | Mises à jour assistées (DNK-64) |
| Politique de sauvegarde et de restauration ? | Répétition DNK-50 |
| Outil de supervision et de journaux (OpenTelemetry, Prometheus, autre) ? | Export des métriques et traces |
| Serveur mail pour les invitations et alertes ? | `DONKA_SMTP_URL` |
| Contact technique et sécurité côté institution ? | Suivi du pilote |
