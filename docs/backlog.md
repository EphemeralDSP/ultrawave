# Backlog

This file tracks prioritized follow-up work that is out of current in-flight tasks.
Use `bd show <id>` for full details and progress.

## Open Items

- [ ] `Ultrawave-2du` - Prevent concurrent writers to same RAM buffer
  - Problem: two Record instances can record to the same buffer concurrently, causing nondeterministic writes and potential clobbering.
  - Tracking: `bd show Ultrawave-2du`
