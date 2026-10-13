# Sandpack NPM CDN

The sandpack cdn is used to serve npm modules in a browser-optimized way, by having the entire npm registry on disk and query it, download the needed files from npm and serve those in a msgpack bundle to the client. Besides this it always add a really fast resolver that uses the in-memory/on-disk npm registry.

## Running the project

Run the following command: `cargo run`

## Self-hosting

### Environment variables

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `NPM_ROCKS_DB` | Yes | - | Filesystem path where the RocksDB-backed npm registry mirror is stored (e.g. `/persisted/npm_rocks_db`). This directory must be on persistent storage — it is where the full replicated registry metadata lives, and losing it means a full re-sync from the npm registry's `_changes` feed. |
| `PORT` | No | `8080` | Port the HTTP server listens on. |
| `ENVIRONMENT` | Only if OTel tracing is enabled | - | Deployment environment name (e.g. `staging`, `production`), reported as the `deployment.environment` resource attribute on traces. Only read/required when `OTEL_EXPORTER_OTLP_ENDPOINT` is set. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | No | - | OTLP/HTTP endpoint to export traces to (e.g. `https://api.honeycomb.io:443`). If unset, tracing falls back to local hierarchical console output instead of OpenTelemetry export. |
| `OTEL_SERVICE_NAME` | No | `sandpack-cdn` | Service name reported on exported traces. Only relevant when `OTEL_EXPORTER_OTLP_ENDPOINT` is set. |
| `OTEL_METADATA_*` | No | - | Any variable with this prefix is forwarded as a header on the OTLP export request, with the prefix stripped, underscores turned into dashes, and the name lowercased. For example `OTEL_METADATA_X_HONEYCOMB_TEAM=<token>` sends the header `x-honeycomb-team: <token>`, and `OTEL_METADATA_AUTHORIZATION=<value>` sends a basic-auth `authorization` header for collectors that require it. |

A minimal local `.env` only needs:

```
PORT=8080
NPM_ROCKS_DB=registry.rocksdb
```

`ENVIRONMENT` and the `OTEL_*` variables can be omitted entirely for local development — tracing will fall back to console output.

### Running with Docker

The included `Dockerfile` builds a release binary and runs it as a non-root user. The npm registry mirror needs to live on a volume so it survives container restarts/redeploys:

```sh
docker build -t sandpack-cdn .
docker run -p 8080:8080 \
  -e NPM_ROCKS_DB=/home/appuser/npm_db/registry.rocksdb \
  -v sandpack-cdn-db:/home/appuser/npm_db \
  sandpack-cdn
```

On first boot the server starts a background replication worker that streams the npm registry's `_changes` feed and populates the RocksDB mirror from scratch (starting at sequence `0`). This can take a long time and a lot of bandwidth/disk before the mirror is useful for resolving arbitrary packages — see `/v2/npm_sync_status` below to track progress.

### Health and sync status

- `GET /health` — liveness check. Returns `200 ok` once the HTTP server is up. This does **not** reflect npm registry replication status — the server can report healthy while the background sync worker is still catching up, stalled, or has stopped.
- `GET /v2/npm_sync_status` — returns the last synced CouchDB `_changes` sequence number the background replication worker has committed to RocksDB, as `{"last_seq": <number>}`. Compare this against the [current npm registry sequence](https://replicate.npmjs.com/registry) to gauge replication lag, or watch it over time to confirm the mirror isn't stalled. This is the right endpoint to wire into uptime/monitoring checks if you need to know whether the mirror is actually current, as opposed to just "the process is running".

### API routes

| Route | Purpose |
| --- | --- |
| `GET /v2/mod/:specifier` | Fetch a package's file contents as a msgpack bundle. |
| `GET /v2/deps/:query` / `GET /v2/json/deps/:query` | Resolve a dependency graph for one or more `name@version` specifiers. |
| `GET /v2/npm_sync_status` | Replication/sync status (see above). |
| `GET /health` | Liveness check (see above). |
