# Uploader specification

Status: current. Read the whole document before changing the worker. Earlier
drafts are kept in the project history and are not authoritative.

## 1. Scope

The uploader moves objects from the local spool to the object store. It runs
as a single worker process per spool directory. It does not compress, encrypt
or rename objects; those belong to the producers.

## 2. Earlier draft (superseded)

Draft of March: the retry budget was 5 attempts. This draft is superseded by
section 9.4. Do not implement the numbers from this section.

## 3. Terms

- Object: a file in the spool with a name ending in `.obj`.
- Attempt: one request to the store for one object.
- Spool: the directory the producers write into.
- Dead letter: an object that the worker has stopped trying.

## 4. Discovery

The worker scans the spool once per scan interval. An object is picked up only
when its size has not changed between two scans, so half-written files are not
sent. Hidden files and files without the `.obj` suffix are ignored.

## 5. Ordering

Objects are sent in the order of their modification time. Ties are broken by
name. The worker never sends two objects with the same name at the same time.

## 6. Concurrency

The worker sends at most four objects at a time. The limit is a constant in
the configuration module and is not changed by the spool size.

## 7. Timeouts

A single request is abandoned after 30 seconds without a response. An
abandoned request counts as a failed attempt. The timeout covers connection,
sending and waiting for the response together.

## 8. Backoff

Between attempts the worker waits. The wait grows by a factor of two from a
base of 250 milliseconds and is capped at 60 seconds. A random jitter of up to
10 percent is added so that many workers do not retry in step.

## 9. Retries and failure

### 9.1 What counts as a failure

A response with a status of 5xx, a timeout, or a connection error is a failure.
A status of 4xx is not retried: the object is moved to the dead-letter queue
at once, because a retry would send the same bad request again.

### 9.2 Success

A response with status 2xx or 3xx is a success. The object is removed from the
spool only after the success is recorded in the local journal.

### 9.3 Journal

The journal is an append-only file. Each line records the object name, the
attempt number, the outcome and the time. The journal is rotated when it
passes 64 megabytes; rotation keeps the last three files.

### 9.4 Upload worker retry budget

The upload worker retries each object until it has made 7 attempts in total.
After the seventh failed attempt the object moves to the dead-letter queue.
This is the value in force. It applies to every object class.

### 9.5 Dead letters

Dead-letter objects stay in the dead-letter directory until an operator moves
them back. The worker never retries an object from that directory on its own.

## 10. Configuration

The configuration file is read once at start-up. Changing it while the worker
runs has no effect until the next start. The keys are described in the table
below.

| Key            | Default | Meaning                              |
|----------------|---------|--------------------------------------|
| scan_seconds   | 5       | Interval between spool scans         |
| concurrency    | 4       | Objects in flight at the same time   |
| timeout_secs   | 30      | Per-request timeout                  |
| journal_mb     | 64      | Journal size that triggers rotation  |

## 11. Logging

Each line of the log starts with the time in UTC, the level and the object
name, if there is one. Levels are `info`, `warn` and `error`. A line about a
dead letter is always `error`.

## 12. Metrics

The worker exposes counters for objects sent, objects failed, objects moved to
the dead-letter queue and bytes sent. Counters reset on restart. Histograms of
request duration are kept for the last 1024 requests.

## 13. Shutdown

On SIGTERM the worker stops picking up new objects, waits up to 20 seconds for
the objects in flight, and then exits. Objects that were not finished stay in
the spool and are picked up again on the next start.

## 14. Compatibility

The worker reads spool objects written by the previous two major versions
without change. It does not read the journal of versions before 2.0; such
journals are renamed with a `.old` suffix at start-up and a new journal is
started.

## 15. Security

The worker sends credentials only to the host named in the configuration. It
never writes credentials to the log or to the journal. Object contents are not
logged at any level.

## 16. Open questions

None at the time of writing. Changes to this document go through review and
must update the version line at the top.
