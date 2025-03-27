use clap::Parser;
use rumqttc::{Client, MqttOptions, QoS};
use serde_json::{json, Value};
use std::sync::mpsc;
use std::time::Duration;

/// Test MQTT message / command sender. Run with
/// cargo run --bin mqtt_sender -- --broker 192.168.10.112 update \
///                --object urn:iot-sensor:powerAbs123 --value 10.0
/// cargo run --bin mqtt_sender -- --broker 192.168.10.112 command \
///                --cmd EngineOn --target urn:aas:smart-home:ev:vw-eup:vin-WVWZZZAAZJD000001

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Sends a JSON message to an MQTT broker",
    long_about = None
)]
struct Args {
    /// MQTT broker address (e.g., "localhost")
    #[arg(short, long, env = "MQTT_BROKER")]
    broker: String,

    /// topic (default is "twins/updates")
    #[arg(short, long, default_value = "twins/updates", env = "MQTT_TOPIC")]
    topic: String,

    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    /// Send an update message.
    Update {
        /// Object for update (e.g., "urn:iot-sensor:powerAbs123")
        #[arg(long)]
        object: String,
        /// Value for update (e.g., 0.5)
        #[arg(long)]
        value: f64,
    },
    /// Send a command message.
    Command {
        /// Command name (e.g., "SwitchOn")
        #[arg(long)]
        cmd: String,
        /// Target for command (e.g., "urn:aas:smart-home:light:light-bulb:id-000001")
        #[arg(long)]
        target: String,
    },
}

fn main() {
    let args = Args::parse();

    let mut message_obj = serde_json::Map::new();
    match args.action {
        Action::Update { object, value } => {
            let update_obj = json!({
                "object": object,
                "value": value
            });
            message_obj.insert("update".to_string(), update_obj);
        }
        Action::Command { cmd: command, target } => {
            let command_obj = json!({
                "command": command,
                "target": target
            });
            message_obj.insert("command".to_string(), command_obj);
        }
    }
    println!(
        "Sending message to {}:{}: {}",
        args.broker,
        args.topic,
        Value::Object(message_obj.clone())
    );

    let payload = Value::Object(message_obj).to_string();
    let mut mqttoptions = MqttOptions::new("dt-send", args.broker, 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    let (client, mut connection) = Client::new(mqttoptions, 10);
    let (ack_tx, ack_rx) = mpsc::channel();

    client
        .publish(&args.topic, QoS::AtLeastOnce, false, payload)
        .expect("Failed to publish message");

    // we need to process the client events for packets to be actually sent
    std::thread::spawn(move || {
        for event in connection.iter() {
            println!("Event: {:?}", event);
            // when we receive a PubAck, we can send the ack to exit the main thread
            if let Ok(rumqttc::Event::Incoming(rumqttc::Packet::PubAck(_))) = event {
                let _ = ack_tx.send(());
            }
        }
    });

    ack_rx.recv().expect("Didn't receive PubAck");
    client.disconnect().expect("Failed to disconnect");
}
