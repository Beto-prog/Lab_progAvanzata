use wg_2024::network::NodeId;

#[derive(Clone, Copy)]
pub enum UICommand {
    CrashDrone(NodeId),
    SetPDR(NodeId, f32),
    AddConnection(NodeId, NodeId),
    RemoveConnection(NodeId, NodeId),
}

pub enum UIResponse {
    Success(String),
    Falure(String),
}
