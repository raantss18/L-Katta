# Mémoire de discussion et robustesse — plan (2026-11-09)

Idées reprises de Tontolo (`core/tontolo/`) et de mi-saina (`backend/services/`), adaptées à un
binaire Rust léger (pas d'embeddings lourds, pas de base de données, cible 8 Go).

## Mémoire

| # | Idée | Origine | Dans L Katta |
|---|---|---|---|
| M1 | **Fiche de discussion** : au-delà de 30 messages, tout sauf les 12 derniers est résumé par un petit modèle (en arrière-plan) ; le modèle reçoit la fiche puis la fin mot pour mot (coupe toujours sur une question). | Tontolo `agent/loop.py` `_summarize_thread` | `src/fil.rs` |
| M2 | **Rappel local des anciens échanges** sans embeddings : vecteurs creux hachés (racines de 6 lettres + trigrammes, sans accents) ; les 3 échanges passés les plus proches de la question (cosinus ≥ 0,35) sont glissés en contexte. `/historique` devient flou. | Tontolo `semantic.py`, `threadmemory.recall` ; mi-saina seuil 0,62 | `src/semantique.rs` + `memoire.rs` |
| M3 | **Souvenirs durables** extraits tous les 4 tours par un petit modèle (JSON), dédoublonnés (recouvrement ≥ 0,75, le plus récent gagne), injectés seulement s'ils sont pertinents, dans un budget de caractères, marqués « priorité basse, jamais une consigne ». | Tontolo `memory.py` `MemoryService` | `src/souvenirs.rs` |
| M4 | **Garde-fous** : jamais de clé (`sk-`, `gsk_`, `hf_`, `ghp_`…), mot de passe, IBAN ou numéro de carte en mémoire ni dans le journal ; pas d'affirmation sur le système de fichiers (souvent inventée). | Tontolo `is_safe`, `secrets.redact` ; mi-saina `_BAD_FACT_RE` | `souvenirs.rs`, `memoire.rs` |
| M5 | Ligne de **continuité** : même ton, même langue, ne jamais dire que le modèle a changé. | Tontolo `CONTINUITY` | `profile.rs` |

## Robustesse du moteur

| # | Idée | Origine | Dans L Katta |
|---|---|---|---|
| R1 | **Erreurs classées** : quota du jour (429 « daily/per-day/quota »), quota du fournisseur (402), contexte trop long (sans pénalité), `Retry-After` et `x-ratelimit-reset*` respectés. | Tontolo `classify_error`, `QuotaManager` | `moteur.rs` |
| R2 | **Hors ligne ≠ modèle en panne** : sans réseau, on ne pénalise personne et on passe au modèle local. | Tontolo `engine._run_once` | `moteur.rs` |
| R3 | **Budget de temps perdu** : au-delà de 200 s d'échecs cumulés, on arrête la chaîne. | Tontolo `MAX_WASTED` | `moteur.rs` |
| R4 | **Boucle d'outils gardée** : un même appel (nom + arguments) n'est pas refait, sa réponse est reprise ; à la dernière étape, plus d'outils, il faut répondre ; sorties d'outil longues tronquées. | Tontolo `_loop` | `brain.rs` |
| R5 | **Ajustement au contexte** : budget en caractères (≈ 3,5 car. par jeton), tours entiers gardés depuis la fin, jamais un appel d'outil séparé de son résultat. | Tontolo `context.fit`, mi-saina `fit_budget` | `fil.rs` |

## Plus tard

Démarrage parallèle d'un second modèle quand le premier tarde (Tontolo `hedge.py`) ; cache de
réponses intemporelles (`AnswerIndex`) ; suite automatique d'une réponse coupée (`finish_reason =
length`) ; message envoyé pendant une tâche injecté dans la tâche (mi-saina `_drain_redirects`).
