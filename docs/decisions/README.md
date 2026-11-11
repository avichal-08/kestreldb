# Architecture decisions

Each file records one decision that shapes the system and would be expensive to reverse: the problem, what was chosen, the alternatives and what the choice costs. Smaller choices live in the code and the commit history.

A record isn't edited to say something else. If a decision is revisited, a new record supersedes it and the old one gets a `Superseded by` line.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-object-storage-as-the-only-state.md) | Object storage is the only place state lives | Accepted |
| [0002](0002-create-only-puts-instead-of-a-lock-service.md) | Create-only puts instead of a lock service | Accepted |
| [0003](0003-one-namespace-per-agent.md) | One namespace per agent, not one sharded graph | Accepted |
| [0004](0004-close-facts-instead-of-deleting.md) | Close facts instead of deleting them | Accepted |
| [0005](0005-no-model-calls-in-the-core.md) | No model calls in the core | Accepted |

## Template

```md
# NNNN. Title

- Status: Proposed | Accepted | Superseded by NNNN
- Date: YYYY-MM-DD

## Context

## Decision

## Alternatives considered

## Consequences
```
