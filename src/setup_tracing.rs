use opentelemetry::sdk::trace as sdktrace;
use opentelemetry_otlp::WithExportConfig;
use std::collections::HashMap;
use std::env;
use std::str::FromStr;
use tonic::metadata::{MetadataKey, MetadataMap};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;
use tracing_tree::HierarchicalLayer;

const HEADER_PREFIX: &str = "OTEL_METADATA_";

// Used environment variables
// OTEL_METADATA_AUTHORIZATION = otel collector basic auth
// OTEL_EXPORTER_OTLP_ENDPOINT = http://otel-collector.csbops.io
fn init_opentelemetry() -> Option<sdktrace::Tracer> {
    let mut headers = HashMap::new();
    for (key, value) in env::vars()
        .filter(|(name, _)| name.starts_with(HEADER_PREFIX))
        .map(|(name, value)| {
            let header_name = name
                .strip_prefix(HEADER_PREFIX)
                .map(|h| h.replace('_', "-"))
                .map(|h| h.to_ascii_lowercase())
                .unwrap();
            (header_name, value)
        })
    {
        println!("Found tracing metadata env variable: {}", key);
        headers.insert(key, value.parse().unwrap());
    }

    if let Err(_err) = env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        println!("env variable OTEL_EXPORTER_OTLP_ENDPOINT has not been set");
        return None;
    }

    if let Err(_err) = env::var("OTEL_SERVICE_NAME") {
        println!("env variable OTEL_SERVICE_NAME has not been set, falling back to sandpack-cdn as the service name");
        env::set_var("OTEL_SERVICE_NAME", "sandpack-cdn");
    }

    let mut headers = HashMap::new();

    // First, create a OTLP exporter builder.
    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_headers(headers)
        .with_env();

    match opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .install_batch(opentelemetry::runtime::Tokio)
    {
        Ok(v) => Some(v),
        Err(err) => {
            println!("Failed to setup tracing {:?}", err);
            None
        }
    }
}

pub fn setup_tracing() {
    // NOTE: the underlying subscriber MUST be the Registry subscriber
    let subscriber = Registry::default() // provide underlying span data store
        .with(LevelFilter::INFO); // filter out low-level debug tracing (eg tokio executor)

    // Install a new OpenTelemetry trace pipeline
    let tracer_res = init_opentelemetry();
    match tracer_res {
        Some(tracer) => {
            // Create a tracing layer with the configured tracer
            let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);
            let new_subscriber = subscriber.with(telemetry_layer);
            tracing::subscriber::set_global_default(new_subscriber).unwrap();
        }
        None => {
            println!("Could not setup open telemetry, falling back to tracing_tree");
            let new_subscriber = subscriber.with(
                HierarchicalLayer::new(2)
                    .with_targets(true)
                    .with_bracketed_fields(true),
            );
            tracing::subscriber::set_global_default(new_subscriber).unwrap();
        }
    }
}
