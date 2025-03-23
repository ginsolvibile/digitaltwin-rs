use log::{debug, error, info};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use serde::Deserialize;
use std::collections::HashMap;
use tokio;
use tokio::sync::mpsc;

use crate::core::twin_actor::ActorMessage;
use crate::core::{AssetID, DeviceID};

/// Network receiver message types
pub enum NetworkMessage {
    /// Subscribe an entity to a list of sensor/actuator IDs
    Subscribe(AssetID, mpsc::Sender<ActorMessage>, Vec<DeviceID>),
}

#[derive(Debug, Clone, Deserialize)]
struct Update {
    oid: DeviceID,
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
}

impl NetworkReceiver {
    pub fn new() -> Self {
        let (send_ch, recv_ch) = mpsc::channel(5);
        NetworkReceiver {
            asset_channels: HashMap::new(),
            subscriptions: HashMap::new(),
            send_ch,
            recv_ch,
        }
    }

    pub fn get_channel(&self) -> mpsc::Sender<NetworkMessage> {
        self.send_ch.clone()
    }

    async fn init(&self, topic: &str) -> EventLoop {
        let mut mqttoptions = MqttOptions::new("test-1", "broker.emqx.io", 1883);
        mqttoptions.set_keep_alive(std::time::Duration::from_secs(5));
        let (client, connection) = AsyncClient::new(mqttoptions, 10);
        client.subscribe(topic, QoS::AtLeastOnce).await.unwrap();
        connection
    }

    pub async fn body(&mut self) {
        info!("Network receiver body starting");

        let mut connection = self.init("twins/updates").await;

        loop {
            tokio::select! {
                event = connection.poll() => {
                    match event {
                        Ok(Event::Incoming(pkt)) => {
                            debug!("Received packet from MQTT: {pkt:?}");
                            if let Packet::Publish(publish) = pkt {
                                if let Ok(update) = serde_json::from_slice::<Update>(&publish.payload) {
                                    debug!("Decoded update: {:?}", update);
                                    if let Some(subscribers) = self.subscriptions.get(&update.oid) {
                                        subscribers.iter().for_each(|aid| {
                                            if let Some(ch) = self.asset_channels.get(aid) {
                                                let _ = ch.send(ActorMessage::InputChange(update.oid.clone(), update.value));
                                            } else {
                                                error!("No channel found for asset ID: {:?}", aid);
                                            }
                                        });
                                    }
                                } else if let Ok(command) = serde_json::from_slice::<Command>(&publish.payload) {
                                    debug!("Decoded command: {:?}", command);
                                    // TODO: send command to twin actor baded on command.target
                                } else {
                                    error!("Failed to decode update from payload");
                                }
                            }
                        }
                        Ok(event) => {
                            debug!("Received event from MQTT: {event:?}");
                        }
                        Err(e) => {
                            error!("Error receiving message from MQTT: {:?}", e);
                        }
                    }
                }
                Some(msg) = self.recv_ch.recv() => {
                    match msg {
                        NetworkMessage::Subscribe(src, ch, oids) => {
                            debug!("Adding new subscriber {} to messages from {:?}", src, oids);
                            self.asset_channels.insert(src.clone(), ch);
                            oids.iter().for_each(|oid| {
                                self.subscriptions.entry(oid.clone()).or_insert(vec![]).push(src.clone());
                            });
                        }
                    }
                }
            }
        }
    }
}
