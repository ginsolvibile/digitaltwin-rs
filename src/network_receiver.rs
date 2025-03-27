use clap::Parser;
use log::{debug, error, info, trace};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::core::twin_actor::ActorMessage;
use crate::core::{AssetID, DeviceID};

#[derive(Parser, Clone)]
pub struct NetworkOptions {
    /// MQTT broker address (e.g., "localhost")
    #[clap(short, long, env = "MQTT_BROKER")]
    broker: String,

    /// topic (default is "twins/updates")
    #[clap(short, long, default_value = "twins/updates", env = "MQTT_TOPIC")]
    topic: String,
}

/// Network receiver message types
pub enum NetworkMessage {
    /// Subscribe an entity to a list of sensor/actuator IDs
    Subscribe(AssetID, mpsc::Sender<ActorMessage>, Vec<DeviceID>),
}

#[derive(Debug, Clone, Deserialize)]
struct Message {
    /// data value update
    update: Option<Update>,
    /// command to be executed
    command: Option<Command>,
}

#[derive(Debug, Clone, Deserialize)]
struct Update {
    object: DeviceID,
    value: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct Command {
    // TODO: define command structure
    target: DeviceID,
}

pub struct NetworkReceiver {
    /// Map of asset IDs to message channels
    asset_channels: HashMap<AssetID, mpsc::Sender<ActorMessage>>,
    /// Map of subscriptions (sensor/actuator ID to asset IDs)
    subscriptions: HashMap<DeviceID, Vec<AssetID>>,
    send_ch: mpsc::Sender<NetworkMessage>,
    recv_ch: mpsc::Receiver<NetworkMessage>,
    /// Options
    options: NetworkOptions,
}

impl NetworkReceiver {
    pub fn new(options: NetworkOptions) -> Self {
        let (send_ch, recv_ch) = mpsc::channel(5);
        NetworkReceiver {
            asset_channels: HashMap::new(),
            subscriptions: HashMap::new(),
            send_ch,
            recv_ch,
            options,
        }
    }

    pub fn get_channel(&self) -> mpsc::Sender<NetworkMessage> {
        self.send_ch.clone()
    }

    async fn init(&self, topic: &str) -> EventLoop {
        debug!("Initializing MQTT connection to {}", self.options.broker);
        let mut mqttoptions = MqttOptions::new("dt-recv", &self.options.broker, 1883);
        mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));
        let (client, connection) = AsyncClient::new(mqttoptions, 10);
        client.subscribe(topic, QoS::AtLeastOnce).await.unwrap();
        connection
    }

    pub async fn body(&mut self) {
        info!("Network receiver body starting");

        debug!("subscribing to MQTT topic {}", self.options.topic);
        let mut connection = self.init(&self.options.topic).await;

        loop {
            tokio::select! {
                event = connection.poll() => {
                    match event {
                        Ok(Event::Incoming(pkt)) => {
                            trace!("Received packet from MQTT: {pkt:?}");
                            if let Packet::Publish(publish) = pkt {
                                if let Ok(message) = serde_json::from_slice::<Message>(&publish.payload) {
                                    debug!("Decoded update: {message:?}");
                                    if let Some (update) = message.update {
                                        if let Some(subscribers) = self.subscriptions.get(&update.object) {
                                            let channels = subscribers.iter().filter_map(|aid| {
                                                self.asset_channels.get(aid).or_else(|| {
                                                    error!("No channel found for asset ID: {aid:?}");
                                                    None
                                                })
                                                .map(|ch| (aid, ch))
                                            });
                                            for (target, ch) in channels {
                                                debug!("sending update to asset {target}: {update:?}");
                                                if let Err(e) = ch.send(ActorMessage::InputChange(update.object.clone(), update.value)).await {
                                                    error!("failed to send update to asset {update:?}: {e:?}");
                                                }
                                            }
                                        }
                                    }
                                    if let Some (command) = message.command {
                                        debug!("Decoded command: {command:?}");
                                        // TODO: send command to twin actor based on command.target
                                    }
                                } else {
                                    error!("Failed to decode update from payload");
                                }
                            }
                        }
                        Ok(event) => {
                            trace!("Received event from MQTT: {event:?}");
                        }
                        Err(e) => {
                            error!("Error receiving message from MQTT: {e:?}");
                        }
                    }
                }
                Some(msg) = self.recv_ch.recv() => {
                    match msg {
                        NetworkMessage::Subscribe(src, ch, oids) => {
                            debug!("Adding new subscriber {src} to messages from {oids:?}");
                            self.asset_channels.insert(src.clone(), ch);
                            oids.iter().for_each(|oid| {
                                self.subscriptions.entry(oid.clone()).or_default().push(src.clone());
                            });
                        }
                    }
                }
            }
        }
    }
}
