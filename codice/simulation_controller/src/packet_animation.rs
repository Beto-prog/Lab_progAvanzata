use wg_2024::network::NodeId;

#[derive(Clone, Copy, Debug)]
pub enum AnimationType {
    Fragment,
    Ack,
    Nack,
}

#[derive(Clone, Debug)]
pub struct PacketAnimation {
    pub source: NodeId,
    pub dest: NodeId,
    pub start_time: f64,
    pub anim_type: AnimationType,
}
