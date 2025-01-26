pub struct DroneStats {
    pub packets_forwarded: u32,
    pub packets_dropped: u32,
    pub acks_forwarded: u32,
    pub nacks_forwarded: u32,
    pub pdr: f32,
}

impl DroneStats {
    pub fn new(pdr: f32) -> Self {
        Self {
            packets_forwarded: 0,
            packets_dropped: 0,
            acks_forwarded: 0,
            nacks_forwarded: 0,
            pdr,
        }
    }
}

pub struct ClientStats {
    packets_sent: u32,
    packets_received: u32,
    acks_sent: u32,
}

impl ClientStats {
    pub fn new() -> Self {
        Self {
            packets_sent: 0,
            packets_received: 0,
            acks_sent: 0,
        }
    }
}

pub struct ServerStats {
    packets_sent: u32,
    packets_received: u32,
    acks_sent: u32,
}

impl ServerStats {
    pub fn new() -> Self {
        Self {
            packets_sent: 0,
            packets_received: 0,
            acks_sent: 0,
        }
    }
}
