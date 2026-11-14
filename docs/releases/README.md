# Release Evidence Archive

This directory is intentionally empty. The per-release verification ledgers for
the v1.2.0 cycle (July 2026) were removed because they described a superseded
release process and linked to plan documents that no longer exist. Their content
remains in Git history through the commit that removed them.

Durable release records live elsewhere:

- [docs/release.md](../release.md): the canonical release checklist and policy
- [docs/verification.md](../verification.md): verification doctrine and failure
  ownership
- [CHANGELOG.md](../../CHANGELOG.md): per-version change and evidence summary
- [plans/registry.md](../../plans/registry.md) and
  [plans/closure/](../../plans/closure/): milestone closure records
- Annotated git tags (`git tag -l 'v*'`) and their commit messages

Do not add new per-release ledgers here. Add release notes to `CHANGELOG.md` and,
for a milestone, a closure record under `plans/closure/`.
