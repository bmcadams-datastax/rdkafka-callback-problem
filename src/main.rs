use std::env;
use std::time::Duration;

use futures::*;
use futures::select;
use futures_channel::oneshot;
use futures_channel::oneshot::Receiver;
use log::error;
use rdkafka::{ClientConfig, ClientContext, producer::FutureProducer};
use rdkafka::error::KafkaError;
use rdkafka::producer::FutureRecord;

struct KafkaErrorHandlingContext {
    callback: oneshot::Sender<KafkaError>
}

impl KafkaErrorHandlingContext {
    fn new() -> (Self, Receiver<KafkaError>) {
        let (err_callback, err_receiver) = oneshot::channel::<KafkaError>();
        (Self { callback: err_callback }, err_receiver)
    }
}

impl ClientContext for KafkaErrorHandlingContext {
    fn error(&self, error: KafkaError, reason: &str) {
        error!("*** Kafka Error: {:?} Reason: {}", error, reason);
        self.callback.send(error);
    }
}

#[tokio::main]
async fn main() {
    let bootstrap_servers = match env::var("BOOTSTRAP_SERVERS") {
        Ok(v) => v,
        Err(_) => panic!("'BOOTSTRAP_SERVERS' is not set")
    };
    let sasl_username = match env::var("SASL_USERNAME") {
        Ok(v) => v,
        Err(_) => panic!("'SASL_USERNAME' is not set")
    };
    let sasl_password = match env::var("SASL_PASSWORD") {
        Ok(v) => v,
        Err(_) => panic!("'SASL_PASSWORD' is not set")
    };
    let mut producer_cfg = ClientConfig::new();
    producer_cfg
        .set("bootstrap.servers", bootstrap_servers)
        .set("socket.timeout.ms", "30000");
    producer_cfg.set("security.protocol", "SASL_SSL");
    producer_cfg.set("sasl.username", sasl_username);
    producer_cfg.set("sasl.password", sasl_password);
    producer_cfg.set("sasl.mechanism","PLAIN");
    producer_cfg.set("enable.ssl.certificate.verification", "true");
    producer_cfg.set("ssl.endpoint.identification.algorithm", "https");
    // todo - we dont support using compression in 3pm so we don't test it here but... maybe we should?
    producer_cfg.set("message.timeout.ms", "10000");

    let (ctx, mut err_receiver) = KafkaErrorHandlingContext::new();
    // skipping batch configs since we intend to send only one message
    let producer: FutureProducer<KafkaErrorHandlingContext> =
        match producer_cfg.create_with_context(ctx) {
            Ok(p) => p,
            Err(e) => panic!("Failed to create Kafka producer: {:?}", e),
        };
    let msg_json = "{{\"success\": false, \"message\": \"A Fatal Error occurred. Please try again later.\"}}";

    let msg_record = FutureRecord::to("topic_1")
        .key("rdkafka-test")
        .payload(msg_json);


    // In the case a message sends successfully the send future does return, and if there's an Error,
    // It will call the context's callback…
    // I think if we pass a closure for the context? hmm...
    let delivery_f = producer.send(msg_record, Duration::from_secs(3));

    let result = select! {
        df = delivery_f.map(|_| ()) => Ok(df),
        er = err_receiver => Err(er),
    };

    println!("{:?}", result);
    match result {
        Ok(_) => println!("Successful message send"),
        Err(e) => println!("Got an error {:?}", e),
    }
}

