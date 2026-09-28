# Contributing to Donka Studio

This guide is the workflow every change follows, from picking a story to shipping it. The
engineering rules live in `AGENTS.md` and `docs/engineering/`; read them before your first
change.

## 1. Branches

| Branch | Role | Who writes to it |
|---|---|---|
| `main` | What customers install | Promotion PRs (`develop` → `main`) and the release PR |
| `develop` | The next release, always green | Squash-merged story PRs only |
| `dnk-<n>-<slug>` | One story | Its author |

Nothing is committed directly to `main` or `develop`.

### Repository settings (GitHub → Settings)

- **General → Pull Requests:** allow squash merging (default message: pull request title and
  description) and merge commits; disable rebase merging; enable "Automatically delete head
  branches".
- **Branches → rulesets** for `main` and `develop`: require a pull request, require the `CI`
  status check to pass, block force pushes, block deletion.
- **Default branch:** `develop` (pull requests target it by default).

## 2. One story, one branch, merged before the next

1. Take the next story from `docs/backlog/` (order in `docs/ROADMAP.md`) and its number `DNK-<n>`.
2. Branch from an up-to-date `develop`:

   ```bash
   git fetch origin develop
   git checkout -b dnk-<n>-<slug> origin/develop
   ```

3. Build the story, its tests and its documentation on that branch.
4. Open a **draft** pull request against `develop` early.
5. **Merge the story into `develop` before starting the next one.** A story is finished only when
   its PR is squash-merged.

**Never stack a story on another unmerged story.**

**A story that spans repositories** (for example a Studio change and the Runtime change that reads
it) has one branch per repository with the same name. The producer merges first: **Donka Runtime
before Studio** when the artifact format or Runtime API changes; **Studio before donka-cli** when
the `rules-sync` API changes.

## 3. What a story contains

Stories live in `docs/backlog/` and use `docs/templates/story.md`: Why, Decision(s), Behaviour,
Acceptance criteria, Out of scope. A choice with lasting consequences gets an ADR in `docs/adr/`
(template: `docs/templates/adr.md`).

## 4. Before opening (or marking ready) the pull request

Every check in `AGENTS.md` §5 passes locally; CI runs the same ones and blocks the merge.

And:

- Tests cover every acceptance criterion, including the edge cases the story names.
- Every new environment variable is in `.env.example`, with a safe local default where possible.
- Database migrations are additive; a merged migration is never edited.
- Every user-facing string exists in English and French.
- UI changes: screenshots, desktop and 390 px, attached to the PR.
- Dead code left behind by the change is removed.

## 5. Commits and pull requests

- **Commits:** Conventional Commits, `<type>(<scope>): <description>`, a body that explains *what
  changed and why*, and a `Refs: DNK-<n>` trailer.
- **PR title** = the squash commit title. **PR body** follows `.github/pull_request_template.md`.
- **No AI authorship trace** in commits, PRs, comments or file headers.
- Keep PRs reviewable in one sitting. Split big stories by layer (schema → service → API → UI).

## 6. Review and merge

1. CI is green on the latest commit.
2. Every review thread is answered: fixed (name the commit) or explained.
3. Mark the PR ready and **squash-merge** into `develop`; delete the branch.
4. Only then start the next story from the updated `develop`.

Story PRs are merged by their author once CI is green; the product owner reviews asynchronously
and can request changes on `develop` at any time through a new story.

A failing check is fixed at its root cause. Never skip, disable or quarantine a test to get
green; never push an empty commit to re-trigger CI.

## 7. Releasing

Versions and changelogs come from the commit messages (release-please, DNK-30): `feat` bumps
the minor version (while below 1.0), `fix` and `perf` the patch; a `!` or `BREAKING CHANGE:`
footer marks a breaking change.

1. Open a promotion PR `develop` → `main` listing the stories it ships, the migrations, the new
   variables and whether a Runtime release is required first. Merge it with a **merge commit**
   once CI is green.
2. The Release workflow opens (or updates) the release PR on `main`: next version in
   `Cargo.toml`, `apps/web/package.json` and `version.txt`, the `CHANGELOG.md` entry, and a
   refreshed `Cargo.lock`. Read the changelog; edit a commit message upstream rather than the
   generated text.
3. Squash-merge the release PR: it tags `vX.Y.Z` and creates the GitHub release. Build the image
   with `--build-arg SERVICE_VERSION=X.Y.Z`.
4. Merge the back-merge PR `main` → `develop` that the workflow opens, with a **merge commit**.
5. Order: **Donka Runtime first** when the artifact format or Runtime API changed, then Studio.
4. Confirm the deployment: `GET /api/v1/health`, migrations applied, a simulate smoke test.
5. If production breaks, roll back first, then fix forward through the normal workflow.

## 8. Definition of done

- [ ] Acceptance criteria met and tested
- [ ] Checks green locally and in CI
- [ ] Docs updated (story, ADR, README, `.env.example`) when behaviour changed
- [ ] Screenshots for visual changes
- [ ] Squash-merged into `develop`, branch deleted
