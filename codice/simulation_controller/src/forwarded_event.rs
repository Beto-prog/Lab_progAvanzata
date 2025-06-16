use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

#[derive(Debug, Clone)]
pub enum ForwardedEvent {
    PacketSent(Packet),
    PacketDropped(Packet),
    PDRSet(NodeId, f32),
    DroneCrashed(NodeId),
    ConnectionAdded(NodeId, NodeId),
    ConnectionRemoved(NodeId, NodeId),
} 