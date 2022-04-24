# Sandpack NPM CDN

🚧 This project is experimental so there should definitely always be a fallback to `unpkg`/`jsdelivr` until this project has a reliable test suite and deploy strategy.

The sandpack cdn is used to speedup SandPack's transpilation of node_modules by having a central server/cdn that pre-transpiles every node_module it requests with the aim to return a slimmed down version of the requested npm module, so that sandpack should not do any additional requests nor transpile any node_module code itself.

## Testing the CDN

Some good packages to test the CDN:

- No exports: `react@17.0.2`
- Conditional root exports: `framer@1.3.6`
- Relative and wildcard exports: `framer@2.0.0-beta.13`
- Array exports: `@babel/runtime@7.16.5`
- No main export, fallback to index: `object-assign@4.1.1`
- Remapping using the browser field: `@emotion/unitless@0.7.5` - Not implemented yet, TODO: Implement this

## Running the project

Run the following command: `RUST_MIN_STACK=16777216 cargo run`

## Environment variables

### Tracing

- OpenTelemetry exporter endpoint: `OTEL_EXPORTER_OTLP_ENDPOINT`
- OpenTelemetry metadata headers, prefix with `OTEL_METADATA_`, for example for honeycomb you add: `OTEL_METADATA_X_HONEYCOMB_TEAM=<API_TOKEN>`

## Deploy the app

### Staging

```shell
flyctl deploy --config ./fly.staging.toml
```
