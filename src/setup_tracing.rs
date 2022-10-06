use opentelemetry::sdk::trace as sdktrace;
use opentelemetry_otlp::WithExportConfig;
use std::env;
use std::str::FromStr;
use tonic::metadata::{MetadataKey, MetadataMap};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;
use tracing_tree::HierarchicalLayer;

const HEADER_PREFIX: &str = "OTEL_METADATA_";

// Used environment variables
// OTEL_METADATA_X_HONEYCOMB_TEAM = honeycomb api key
// OTEL_EXPORTER_OTLP_ENDPOINT = https://api.honeycomb.io:443
fn init_opentelemetry() -> Option<sdktrace::Tracer> {
    let mut metadata = MetadataMap::new();
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
        metadata.insert(MetadataKey::from_str(&key).unwrap(), value.parse().unwrap());
    }

    if !metadata.contains_key(String::from("otlp-endpoint")) {
        return None;
    }

    env::set_var("OTEL_SERVICE_NAME", "sandpack-cdn");
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_env()
        .with_metadata(dbg!(metadata));

    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .install_batch(opentelemetry::runtime::Tokio)
        .map(|v| Some(v))
        .unwrap_or(None)
}

pub fn setup_tracing() {
    // NOTE: the underlying subscriber MUST be the Registry subscriber
    let subscriber = Registry::default() // provide underlying span data store
        .with(LevelFilter::INFO) // filter out low-level debug tracing (eg tokio executor)
        .with(tracing_subscriber::fmt::Layer::default());

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
