# Sandpack NPM CDN

The sandpack cdn is used to serve npm modules in a browser-optimized way, by having the entire npm registry on disk and query it, download the needed files from npm and serve those in a msgpack bundle to the client. Besides this it always add a really fast resolver that uses the in-memory/on-disk npm registry.

## Running the project

Run the following command: `cargo run`

## Environment variables

### Tracing

- OpenTelemetry exporter endpoint: `OTEL_EXPORTER_OTLP_ENDPOINT`
- OpenTelemetry metadata headers, prefix with `OTEL_METADATA_`, for example for honeycomb you add: `OTEL_METADATA_X_HONEYCOMB_TEAM=<API_TOKEN>`
