# 05 — Storage (the local store)

The committed book is plain text in git. The **local store** (`.inklore/local/book.db`) is a single-file
embedded database that holds the code graph, an indexed copy of the committed entries, computed
membership, blobs and task state. It is derived data: delete it and `inklore sync` rebuilds it.

We write this store ourselves (crate `inklore-store`) behind a small trait, and keep
[redb](https://github.com/cberner/redb) as a second backend for differential testing and benchmarks.
If our store loses on the benchmarks, that is reported honestly — the trait makes swapping cheap.

## Workload

- **Read-heavy.** Every hook call and brief is a burst of point lookups and short range scans.
- **Bursty writes.** Ingest after a sync or branch switch writes thousands of records at once; between
  bursts, writes are small (one entry, one task update).
- **Snapshots are a feature, not an overhead.** The scope check needs "the graph as it was when the task
  started".
- **Sizes.** A 140k-line backend is roughly 30k symbols, 150k edges and a few thousand entries — tens of
  megabytes.

A copy-on-write B+tree fits this: fast reads, atomic commits without a separate log, and free
snapshots. (An LSM tree would suit sustained heavy writes, which we don't have.)

## Process model

One process owns the store at a time.

- `inklore serve` is a small daemon, started automatically by the first client, that owns the store and
  serves requests over a local socket (Unix domain socket; named pipe on Windows).
- The CLI, MCP server and hook commands are thin clients. Hooks are short-lived processes, so they must
  not each open and lock the database.
- If no daemon is running and none can be started, the CLI opens the store directly under an exclusive
  file lock.

This keeps reader tracking in one process (no shared-memory reader table, unlike LMDB) and lets the
daemon keep the page cache warm.

## File layout

```text
page 0   meta A ┐ alternating meta pages; the valid one with the highest txn id wins
page 1   meta B ┘
page 2…  B+tree branch/leaf pages, overflow pages, freelist pages
```

- **Page size** 4 KiB.
- **Page header**: page type, flags, entry count, checksum (CRC32C of the page), and for overflow pages
  the next-page pointer.
- **Meta page**: magic `INKLORE\0`, format version, page size, txn id, root page, freelist root, page
  count, pinned-snapshot list, checksum.

## Copy-on-write B+tree

- **Single tree, prefixed keys.** The first key byte is a table ID (below). One tree keeps commits
  atomic across all tables and makes range scans per table natural.
- **Writes copy the path.** Changing a leaf allocates new pages for the leaf and every ancestor up to the
  root. Old pages are untouched, which is what makes snapshots and crash safety free.
- **Commit protocol.**
  1. write all new pages
  2. `fsync` the file
  3. write the *inactive* meta page with `txn + 1` and the new root
  4. `fsync` again

  A crash before step 4 completes leaves the previous meta valid: the commit simply did not happen.
  No write-ahead log is needed for the main store.
- **Readers** read the current meta at begin and hold that root for their lifetime — a consistent
  snapshot, never blocked by the writer.
- **Single writer**, many readers (within the daemon).
- **Free pages.** Pages replaced by txn `T` go onto a pending list tagged `T`. They return to the free
  list only when no reader and no **pinned snapshot** is older than `T`. Task snapshots are pinned in the
  meta page so pins survive a daemon restart.
- **Overflow pages** hold values larger than a quarter page.
- **Group commit.** Ingest batches many files into one transaction.

## Encoding

- **Keys are order-preserving.** Table byte, then components: integers big-endian; strings with `0x00`
  escaped as `0x00 0xFF` and terminated by `0x00 0x01`. Composite keys therefore sort component by
  component and support prefix scans.
- **Values** use a small hand-written binary codec with a version byte per record type, so formats can
  evolve without a migration tool in v0.
- **Postings lists** (text search) are delta-encoded varints.

## Tables

| ID | Table | Key | Value |
|---|---|---|---|
| 1 | `entity` | entity id | Entity |
| 2 | `entity_by_path` | path, entity id | — |
| 3 | `edge_out` | from, kind, to | confidence, site |
| 4 | `edge_in` | to, kind, from | — |
| 5 | `entry` | entry id | parsed entry |
| 6 | `entry_by_anchor` | entity id, entry id | `on`, stored hash |
| 7 | `entry_by_scope` | scope, entry id | — |
| 8 | `feature` | slug | Feature |
| 9 | `feature_member` | slug, entity id | shared flag, depth |
| 10 | `member_feature` | entity id, slug | — |
| 11 | `blob` | content hash | bytes (body snapshots for stale diffs) |
| 12 | `term` | token | postings (for `find`) |
| 13 | `task` | task id | Task |
| 14 | `file_state` | path | content hash, mtime, size (cheap change detection) |
| 15 | `text_state` | `.inklore/` file path | content hash (sync of committed text) |

## Task event log

Hook events arrive many times a minute and are append-only, so they go to a separate **log-structured**
file per task: `.inklore/local/events/<task>.log`.

- record = `[len: u32][crc32c: u32][payload]`
- appends are buffered and `fsync`ed at checkpoints and at task finish
- on open, a torn or corrupt tail record is truncated

## Page cache and I/O

- Fixed-size buffer pool with CLOCK eviction.
- v0 uses **positioned reads/writes** (`pread`/`pwrite`; `seek_read`/`seek_write` on Windows), not
  `mmap`: Windows cannot grow a file while it is mapped, and turning I/O errors on mapped memory into
  clean errors is awkward. A read-only `mmap` path can come later behind a feature flag.
- `fsync` = `File::sync_data` (FlushFileBuffers on Windows).
- File locking via `LockFileEx` / `flock` (through a crate such as `fd-lock`).

## The trait

```rust
pub trait Store: Send + Sync {
    type Read<'a>: ReadTxn where Self: 'a;
    type Write<'a>: WriteTxn where Self: 'a;

    fn read(&self) -> Result<Self::Read<'_>>;
    fn write(&self) -> Result<Self::Write<'_>>;
    fn pin(&self, txn: TxnId) -> Result<()>;      // keep a snapshot alive (task start)
    fn unpin(&self, txn: TxnId) -> Result<()>;
    fn read_at(&self, txn: TxnId) -> Result<Self::Read<'_>>;   // open a pinned snapshot
}

pub trait ReadTxn {
    fn txn_id(&self) -> TxnId;
    fn get(&self, key: &[u8]) -> Result<Option<Bytes>>;
    fn range(&self, lo: Bound<&[u8]>, hi: Bound<&[u8]>) -> Result<RangeIter<'_>>;
}

pub trait WriteTxn: ReadTxn {
    fn put(&mut self, key: &[u8], value: &[u8]) -> Result<()>;
    fn delete(&mut self, key: &[u8]) -> Result<bool>;
    fn commit(self) -> Result<TxnId>;
    // dropping without commit aborts
}
```

Implementations: `InkStore` (ours) and `RedbStore` (adapter over redb).

## Testing

| Kind | What |
|---|---|
| Property tests | random sequences of put/delete/range/commit/abort checked against a `BTreeMap` model |
| Differential | identical operation streams on `InkStore` and `RedbStore`; results must match |
| Crash simulation | an I/O layer that can drop unsynced writes or tear a page at any point; sweep "crash after write N" for every N in a workload and check the store opens to the last committed state |
| Corruption | flip bits in pages; checksums must catch it and report, never return wrong data |
| Fuzzing | `cargo fuzz` on page decoding and key/value codecs |
| Benchmarks | `criterion`: bulk ingest, point gets, prefix scans, brief-shaped read bursts — against redb and SQLite |

## Targets (v0, on a laptop)

| Operation | Target |
|---|---|
| Full ingest, 140k-line repo (tree-sitter only) | < 30 s |
| Incremental sync of one changed file | < 50 ms |
| `brief` for a feature | < 50 ms p99 |
| Hook round trip (client → daemon → client) | < 20 ms p99 |
