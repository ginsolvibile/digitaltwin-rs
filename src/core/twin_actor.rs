use log::{debug, info, warn};
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::core::ActorStateType;
use crate::core::{AssetAdministrationShell, AssetID, DeviceID};
use crate::manager::ManagerMessage;
use crate::models::LightBulb;
use crate::network_receiver::NetworkMessage;

/// Actor message types
#[derive(Debug, Clone)]
pub enum ActorMessage {
    /// Change the value of an input slot
    InputChange(DeviceID, f32),
    /// Execute a command
    Command(String, serde_json::Value),
}

pub struct TwinActor {
    /// The AAS for this Digital Twin
    aas: AssetAdministrationShell,
    inner_state: Box<ActorStateType>,
    /// List of slots that the DT state will listen to
    slots: Vec<&'static str>,
    /// Mapping of sensor IDs to slot names
    slot_map: HashMap<DeviceID, String>,
    send_ch: mpsc::Sender<ActorMessage>,
    recv_ch: mpsc::Receiver<ActorMessage>,
    manager_ch: mpsc::Sender<ManagerMessage>,
    network_ch: mpsc::Sender<NetworkMessage>,
}

impl TwinActor {
    pub fn new(
        aas: AssetAdministrationShell,
        manager_ch: mpsc::Sender<ManagerMessage>,
        network_ch: mpsc::Sender<NetworkMessage>,
    ) -> Self {
        let object_type = aas.id.split(':').nth(3).unwrap(); // FIXME: unwrap
        let inner_state = match object_type {
            "light" => LightBulb::<()>::create(0.5),
            "ev" => LightBulb::<()>::create(0.5),
            "charging-station" => LightBulb::<()>::create(0.5),
            _ => panic!("Unknown object type: {}", object_type),
        };
        let slots = match object_type {
            "light" => LightBulb::<()>::slots(),
            "ev" => LightBulb::<()>::slots(),
            "charging-station" => LightBulb::<()>::slots(),
            _ => panic!("Unknown object type: {}", object_type),
        };
        let (send_ch, recv_ch) = mpsc::channel(5);
        TwinActor {
            aas,
            inner_state,
            slots,
            slot_map: HashMap::new(),
            send_ch,
            recv_ch,
            manager_ch,
            network_ch,
        }
    }

    pub fn id(&self) -> AssetID {
        self.aas.id.clone()
    }

    pub async fn init(&mut self) {
        // Register the actor with the manager
        let _ = self
            .manager_ch
            .send(ManagerMessage::Register(self.id(), self.send_ch.clone()))
            .await;

        // Register the actor with the network receiver
        let _ = self
            .network_ch
            .send(NetworkMessage::Register(
                self.id(),
                self.send_ch.clone(),
            ))
            .await;

        for s in self.slots.iter() {
            if let Some(sensor) = self
                .aas
                .find_reference_value_in_collection("PowerAndElectrical", s, "DataSource")
                .and_then(|ref_value| self.aas.resolve_sensor_reference(&ref_value))
            {
                self.slot_map.insert(sensor, s.to_string());
            } else {
                warn!("No sensor ID found for {}", s);
            }
        }
        debug!("Slot map for {} is: {:?}", self.id(), self.slot_map);

        let sensor_ids = self
            .aas
            .find_elements_in_collection("IoTDataSources", "Sensors", "SensorID");
        if sensor_ids.is_empty() {
            info!("No sensor IDs found for {}", self.id());
            return;
        }
        // Subscribe to the input sensors
        let _ = self
            .network_ch
            .send(NetworkMessage::Subscribe(
                self.id(),
                sensor_ids,
            ))
            .await;
    }
}

pub async fn body(mut twin: Box<TwinActor>) {
    twin.init().await;
    info!("Twin actor body {} starting", twin.id());
    loop {
        tokio::select! {
            Some(msg) = twin.recv_ch.recv() => {
                match msg {
                    ActorMessage::InputChange(obj_id, value) => {
                        if let Some(slot) = twin.slot_map.get(&obj_id) {
                            debug!("{} Received input change: {} = {}", twin.id(), slot, value);
                            twin.inner_state = twin.inner_state.input_change(slot, value);
                            debug!("{} New state: {:?}", twin.id(), twin.inner_state);
                        } else {
                            warn!("{} Received input change from unknown object: {}", twin.id(), obj_id);
                            debug!("{} current slot map: {:?}", twin.id(), twin.slot_map);
                        }
                    }
                    ActorMessage::Command(command, args) => {
                        debug!("{} Received command {command} with args {args:?}", twin.id());
                        twin.inner_state = twin.inner_state.execute(&command, args);
                        debug!("{} New state: {:?}", twin.id(), twin.inner_state);
                    }
                }
            }
        }
    }
}
