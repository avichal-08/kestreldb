# 0004. Close facts instead of deleting them

- Status: Accepted
- Date: 2026-11-08

## Context

Facts about people and projects change. A user moves cities, a project changes owner, a preference flips. If memory overwrites or deletes the old fact, the agent can no longer answer "where did they live before", can't tell a correction from a change, and can't explain why it once believed something.

There are also two different times involved. When a fact was true in the world isn't the same as when the agent learned it: a user can say today that they moved last month.

## Decision

Relations are never deleted. Each one has valid time (`valid_from`, `valid_to`), when it held in the world, and recorded time (`created_at`, `invalidated_at`), when memory wrote it and when it was closed.

- Closing a fact sets `valid_to` and `invalidated_at`. If a fact is closed twice, the earlier `valid_to` wins.
- Current queries skip closed facts. `as_of: t` queries use valid time, so a closed fact still answers questions about the period it covered.
- `exclusive()` closes current facts with the same subject and predicate when a new one is written, for things that have one value at a time, like `lives_in`.
- Every fact keeps the ids of the episodes it came from, so recall can return the source text with it.

## Alternatives considered

- **Overwrite in place.** Smallest data and simplest queries, and the history is gone.
- **Valid time only.** Answers "what was true then", but can't separate "it changed" from "we got it wrong", and can't replay what memory believed at a past moment.
- **Keep history only in the log.** The log has every op, but answering a history question would mean replaying it, which doesn't work once old batches are compacted away.

## Consequences

- Memory only grows. Closed facts take space and are scanned by recall until there's an index. Eventually they'll need an age-out policy per namespace.
- `as_of` currently filters on valid time only. Asking "what did memory believe at time t" would also need a filter on recorded time, which isn't exposed yet.
- Merging entities keeps one record per kind and name, so an entity's summary and attributes show their latest values only. History is kept for facts, not entity fields.
