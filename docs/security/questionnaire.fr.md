# Questionnaire de sécurité — réponses pré-remplies

**Objet (DNK-73) :** les questions que l'équipe sécurité d'une banque pose à un éditeur
de logiciels, répondues à l'avance, chaque réponse renvoyant à une preuve vérifiable de
ce dossier. Les statuts sont exacts : *auto-évalué* n'est jamais écrit *certifié*.
Rédigé le 2026-11-09 sur `develop` à `4ce8625`. [English version](questionnaire.md).

| # | Question | Réponse | Preuve |
| --- | --- | --- | --- |
| 1 | Quel est le produit et comment est-il déployé ? | Logiciel de gestion des décisions (Studio + Runtime + CLI) hébergé sur les serveurs de la banque avec Docker ; Donka n'exploite rien. | Guides d'installation EN/FR |
| 2 | Détenez-vous une certification ISO 27001 ou SOC 2 ? | Non. Les deux certifient une organisation dans la durée, pas un logiciel ; le programme est une décision de phase 3. Il existe en revanche une correspondance contrôle par contrôle. | [Matrices de conformité](compliance.md) |
| 3 | L'application a-t-elle été testée en sécurité ? | En continu : audit des dépendances verrouillées, scan des images et suites de tests complètes à chaque changement ; auto-évaluation OWASP ASVS 4.0.3 niveau 2 avec exceptions nommées. Test d'intrusion indépendant : périmètre publié, choix du cabinet dans le programme client (DNK-69 partie 2). | [ASVS](asvs.md), [périmètre du test](penetration-test-scope.md) |
| 4 | Comment le logiciel est-il construit et livré ? | Correspondance NIST SSDF ; une histoire par branche relue ; fusion squash uniquement quand les six jobs CI passent. | [Correspondance SSDF](ssdf.md) |
| 5 | Les livraisons sont-elles signées et vérifiables hors ligne ? | Les artefacts de release sont signés Ed25519 par la clé de l'installation et vérifiés par le Runtime avant usage ; la provenance des images (SLSA) est préparée et indiquée exactement comme en attente de la première release signée (DNK-73 partie 4). | DNK-87 ; guides d'installation |
| 6 | Comment signaler et corriger une vulnérabilité ? | Signalement privé par dépôt ; délais et versions supportées dans `SECURITY.md` ; chaque constat entre dans le modèle de menaces avec un responsable. | `SECURITY.md`, [modèle de menaces](threat-model.md) |
| 7 | Où vivent nos données ? Qui y a accès ? | Uniquement sur les serveurs de la banque. Donka n'a ni accès, ni télémétrie, ni copie hébergée. | Documentation d'architecture |
| 8 | Comment les données personnelles sont-elles protégées ? | Les données personnelles sont marquées dans les contrats, masquées dans les flux, rejeux et explications, conservées selon la politique et supprimables par personne ; les numéros de carte sont refusés net. | [Matrices](compliance.md), [périmètre PCI](pci-dss-scope.md) |
| 9 | Le produit entre-t-il dans le périmètre PCI DSS ? | Non, par conception : tout contrat contenant ou ressemblant à un numéro de carte est refusé à l'enregistrement. | [Périmètre PCI DSS](pci-dss-scope.md) |
| 10 | Authentification et contrôle d'accès ? | Comptes personnels, mots de passe Argon2, second facteur optionnel ou requis (TOTP ou code par courriel), rôles projet avec approbations à quatre yeux, jetons à portée limitée et expirants. | [ASVS](asvs.md) V2–V4 |
| 11 | Chiffrement ? | Scellement AES-256-GCM des secrets stockés et du journal de décisions ; signatures d'artefacts Ed25519 ; TLS au proxy de la banque ; HSTS et CSP stricte dans le navigateur. | [ASVS](asvs.md) V6/V9, DNK-85 |
| 12 | Auditabilité ? | Journal d'audit immuable et journal de décisions scellé et vérifiable ; chaque décision peut être rejouée et expliquée selon les règles de masquage. | DNK-54, DNK-88 |
| 13 | Continuité et reprise ? | Sauvegarde livrée avec l'installation ; la répétition chronométrée de restauration/mise à niveau (RPO/RTO) est DNK-50 et ses résultats seront publiés ici. | [Matrices](compliance.md) (ISO 22301) |
| 14 | Accessibilité ? | Auto-évaluation WCAG 2.2 AA avec limites écrites (clavier dans l'éditeur de graphes, tailles de cibles). | [WCAG](wcag.md) |
| 15 | Quelles réglementations la correspondance couvre-t-elle ? | ISO/IEC 27001:2022 Annexe A, critères SOC 2, RGPD, ISO 22301, obligations haut risque de l'AI Act européen, Convention de Malabo, CEMAC/COBAC, UEMOA/BCEAO, Guinée, et lois des pays pilotes comme la loi camerounaise 2024/017, chacune avec responsable (Donka/client/partagé) et statut. | [Matrices](compliance.md) |
| 16 | Pouvons-nous tout vérifier nous-mêmes ? | Oui : chaque réponse nomme un fichier, un job CI ou une page ouvrable. La page Confiance (DNK-66) publiera le dossier avec les SBOM et la clé publique de signature. | Ce dossier |

Les réponses changent dans le même commit que le contrôle qu'elles décrivent ; une réponse
devenue fausse est un défaut, signalé via `SECURITY.md` comme tout autre.
