use log::info;
use tokio::join;

mod core;
mod manager;
mod models;
mod network_receiver;

#[tokio::main]
async fn main() {
    env_logger::init();

    info!("Creating components");
    let mut network_receiver = network_receiver::NetworkReceiver::new();
    let network_channel = network_receiver.get_channel();
    let mut manager = manager::Manager::new(network_channel);

    let manager_channel = manager.get_channel();
    let _ = manager_channel.send(manager::ManagerMessage::Initialize).await;

    info!("Starting services");
    let _ = join!(
        manager.body(),
        network_receiver.body(),
        // TODO add rest_server.body(),
    );
}
