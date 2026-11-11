# 0002. Create-only puts instead of a lock service

- Status: Accepted
- Date: 2026-11-08

## Context

With storage in a bucket ([0001](0001-object-storage-as-the-only-state.md)), two processes can open the same namespace and both try to commit. Two failure cases have to be ruled out: both writers believing they committed batch `N`, and a writer committing on top of state it never saw.

The usual answers are a lock or lease in a coordination service (etcd, ZooKeeper, DynamoDB), or a single elected leader per namespace. Both add a system that has to be up for writes to work, and leases still need fencing for a writer that paused past its lease.

## Decision

Batch `N` is written to `wal/<N>.json` with a create-only put (`PutMode::Create`, sent as `If-None-Match: *` on S3). Exactly one writer can create a given sequence number. The loser gets `AlreadyExists`, which the log turns into `Error::Conflict { seq }`. It then calls `refresh()` to read what it missed, rebuilds its batch against the new state and tries the next sequence number.

The sequence number in the object name is the fencing token. A writer that's behind can't overwrite anything, because the slot it's aiming for is already taken.

## Alternatives considered

- **A lease in a coordination service.** Lets one writer own a namespace and skip conflicts, but every write path then depends on that service, and a paused writer can still act after its lease ends unless storage checks a fencing token anyway.
- **Compare-and-swap on a single head object** (a manifest updated with `If-Match`). Also safe, but every commit becomes a read and a conditional write of the same hot object, plus a separate write for the batch itself. It's the right tool for snapshots, which change rarely, and is planned for the manifest.
- **Last writer wins.** Simple, and loses data the first time two writers overlap.

## Consequences

- Conflicts are visible to callers. A retry must rebuild the batch, not resend it, because merges and exclusive facts depend on the state the writer had.
- Under heavy contention on one namespace, writers spend their time retrying. Routing a namespace's writes to one node keeps that rare, but correctness doesn't depend on it.
- The store must support conditional creates. S3, GCS, Azure, R2, MinIO and the local filesystem backend do. An S3 store configured with `conditional_put` set to `disabled` fails every commit.
- Sequence numbers must have no gaps. Replay treats a missing number as corruption.
