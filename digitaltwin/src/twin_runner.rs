use log::{debug, info, warn};
use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::manager::ManagerMessage;
use crate::models::{ChargingStationFactory, LightBulbFactory};
use crate::network_receiver::NetworkMessage;
use digitaltwin_core::{ActorFactory, ActorStateType, AssetAdministrationShell, AssetID, DeviceID};

/// Actor message types
#[derive(Debug, Clone)]
pub enum ActorMessage {
    /// Change the value of an input slot
    InputChange(DeviceID, f32),
    /// Execute a command
    Command(String, serde_json::Value),
}

pub struct TwinRunner {
    /// The AAS for this Digital Twin
    aas: AssetAdministrationShell,
    /// The actor's internal state
    inner_state: Box<ActorStateType>,
    /// All the slots the actor will listen to (used only during initialization)
    slots: Vec<&'static str>,
    /// Mapping of sensor IDs to slot names
    slot_map: HashMap<DeviceID, String>,
    send_ch: mpsc::Sender<ActorMessage>,
    recv_ch: mpsc::Receiver<ActorMessage>,
    manager_ch: mpsc::Sender<ManagerMessage>,
    network_ch: mpsc::Sender<NetworkMessage>,
}

impl TwinRunner {
    pub fn new(
        aas: AssetAdministrationShell,
        manager_ch: mpsc::Sender<ManagerMessage>,
        network_ch: mpsc::Sender<NetworkMessage>,
    ) -> Self {
        let object_type = aas.id.split(':').nth(3).unwrap(); // FIXME: unwrap
        let (inner_state, slots) = match object_type {
            "light" => LightBulbFactory::create_default(),
            "ev" => LightBulbFactory::create_default(),
            "charging-station" => ChargingStationFactory::create_default(),
            _ => panic!("Unknown object type: {}", object_type),
        };

        let (send_ch, recv_ch) = mpsc::channel(5);
        TwinRunner {
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
            .send(NetworkMessage::Register(self.id(), self.send_ch.clone()))
            .await;

        for s in self.slots.iter() {
            // Create an input slot for each reference to the DataSource subsystem found in the PowerAndElectrical submodel
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

        // Subscribe to any sensor IDs found in the AAS in the IoTDataSources submodel under Sensors
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
            .send(NetworkMessage::Subscribe(self.id(), sensor_ids))
            .await;
    }
}

pub async fn body(mut twin: Box<TwinRunner>) {
    twin.init().await;
    info!("Twin runner body {} starting", twin.id());
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
