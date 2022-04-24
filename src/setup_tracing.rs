use opentelemetry::sdk::trace as sdktrace;
use opentelemetry::trace::TraceError;
use opentelemetry_otlp::WithExportConfig;
use std::env;
use std::str::FromStr;
use tonic::metadata::{MetadataKey, MetadataMap};
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

const HEADER_PREFIX: &str = "OTEL_METADATA_";

// Used environment variables
// OTEL_METADATA_X_HONEYCOMB_TEAM = honeycomb api key
// OTEL_EXPORTER_OTLP_ENDPOINT = https://api.honeycomb.io:443
fn init_opentelemetry() -> Result<sdktrace::Tracer, TraceError> {
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

    env::set_var("OTEL_SERVICE_NAME", "sandpack-cdn");
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_env()
        .with_metadata(dbg!(metadata));

    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .install_batch(opentelemetry::runtime::Tokio)
}

pub fn setup_tracing() {
    // Install a new OpenTelemetry trace pipeline
    let tracer = init_opentelemetry().unwrap();

    // Create a tracing layer with the configured tracer
    let telemetry_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    // NOTE: the underlying subscriber MUST be the Registry subscriber
    let subscriber = Registry::default() // provide underlying span data store
        .with(LevelFilter::INFO) // filter out low-level debug tracing (eg tokio executor)
        .with(tracing_subscriber::fmt::Layer::default()) // log to stdout
        .with(telemetry_layer); // publish to honeycomb backend

    tracing::subscriber::set_global_default(subscriber).expect("setting global default failed");
}
