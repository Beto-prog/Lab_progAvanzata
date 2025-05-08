use serde::Deserialize;
use wg_2024::network::NodeId;

#[derive(Debug, Deserialize)]
pub struct DroneConfig {
    pub id: NodeId,
    pub connected_node_ids: Vec<NodeId>,
    pub pdr: f32,
}

#[derive(Debug, Deserialize)]
pub struct ClientConfig {
    pub id: NodeId,
    pub connected_drone_ids: Vec<NodeId>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub id: NodeId,
    pub connected_drone_ids: Vec<NodeId>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfig {
    pub drone: Vec<DroneConfig>,
    pub client: Vec<ClientConfig>,
    pub server: Vec<ServerConfig>,
}