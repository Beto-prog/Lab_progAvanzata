use wg_2024::network::NodeId;

pub enum UICommand {
    CrashDrone(NodeId),
    SetPDR(NodeId, f32),
}
