use log::{debug, error, info, trace};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufReader;
use thiserror::Error as ThisError;
use tokio::sync::mpsc;
use tokio::task;

use crate::core::{twin_actor, AssetAdministrationShell};
use crate::network_receiver;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    /// Generic error
    #[error("generic error: {0}")]
    GenericError(String),
}

/// Manager message types
pub enum ManagerMessage {
    /// Initialize the manager (sent by the main function)
    Initialize,
    /// Register a new actor (sent by an actor)
    Register(String, mpsc::Sender<twin_actor::ActorMessage>),
}

pub struct Manager {
    actors: HashMap<String, mpsc::Sender<twin_actor::ActorMessage>>,
    send_ch: mpsc::Sender<ManagerMessage>,
    recv_ch: mpsc::Receiver<ManagerMessage>,
    network_ch: mpsc::Sender<network_receiver::NetworkMessage>,
}

impl Manager {
    pub fn new(network_ch: mpsc::Sender<network_receiver::NetworkMessage>) -> Self {
        let (send_ch, recv_ch) = mpsc::channel(5);
        Manager {
            actors: HashMap::new(),
            send_ch,
            recv_ch,
            network_ch,
        }
    }

    pub fn get_channel(&self) -> mpsc::Sender<ManagerMessage> {
        self.send_ch.clone()
    }

    pub fn initialize_dtwins(&self) -> Result<(), Error> {
        let mut twins = HashSet::new();
        for entry in std::fs::read_dir("./twins")? {
            let path = entry?.path();
            if path.extension().unwrap_or_default() != "yaml" {
                continue;
            }
            debug!("Processing file: {:?}", path.display());
            if let Ok(reader) = File::open(&path).map(BufReader::new) {
                let aas = AssetAdministrationShell::from_reader(reader)
                    .map_err(|e| Error::GenericError(e.to_string()))?;
                trace!("{:#?}", aas);
                if !twins.insert(aas.id.clone()) {
                    error!("Duplicate AAS id: {}, ignored", aas.id);
                    continue;
                }
                info!(
                    "Creating new digital twin for {} ({:?})",
                    aas.id,
                    aas.description.as_ref()
                );
                let twin = twin_actor::TwinActor::new(aas, self.send_ch.clone(), self.network_ch.clone());
                task::spawn(twin_actor::body(Box::new(twin)));
            }
        }
        Ok(())
    }

    pub async fn body(&mut self) {
        info!("Manager body starting");
        loop {
            tokio::select! {
                Some(msg) = self.recv_ch.recv() => {
                    match msg {
                        ManagerMessage::Register(id, ch) => {
                            debug!("Registering actor with id: {}", id);
                            self.actors.insert(id, ch);
                        }
                        ManagerMessage::Initialize => {
                            debug!("Initializing digital twins...");
                            if let Err(e) = self.initialize_dtwins() {
                                error!("Error initializing digital twins: {:?}", e);
                            }
                        }
                    }
                }
            }
        }
    }
}
