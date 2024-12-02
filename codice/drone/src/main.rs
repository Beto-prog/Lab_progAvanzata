use crossbeam_channel::{select_biased, Receiver, Sender};
use rand::{Rng};
use std::collections::HashMap;
use rand::rngs::ThreadRng;
use wg_2024::controller::NodeEvent::{PacketDropped, PacketSent};
use wg_2024::controller::{DroneCommand, NodeEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Nack, NackType, Packet, PacketType};

struct TrustDrone {
    id: NodeId,
    controller_send: Sender<NodeEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rng: ThreadRng,
}

impl Drone for TrustDrone {
    fn new(
        id: NodeId,
        controller_send: Sender<NodeEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Self {
        TrustDrone {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
            rng: rand::thread_rng(),
        }
    }

    fn run(&mut self) {
        loop {
            select_biased! {
                 recv(self.controller_recv) -> command => {
                    match command {
                        Ok(command) => {
                           self.handle_command(command);
                        },
                        Err(e) => {
                            eprintln!("Error on command reception of drone {}: {}", self.id, e)
                        },
                    }
                }
                recv(self.packet_recv) -> packet => {
                    match packet {
                        Ok(packet) => {
                           self.handle_packet(packet);
                        },
                        Err(e) => {
                            eprintln!("Error on packet reception of drone {}: {}", self.id, e)
                        }
                    }
                }
            }
        }
    }
}

impl TrustDrone {
    fn handle_command(&mut self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(id, sender) => {
                self.add_sender(id, sender);
            }
            DroneCommand::SetPacketDropRate(pdr) => {
                self.set_packet_drop_rate(pdr);
            }
            DroneCommand::Crash => {
                println!("Drone with id {} crashed", self.id);
                // TODO implement drone crashing
            }
            DroneCommand::RemoveSender(_) => todo!()
        }
    }

    fn handle_packet(&mut self, mut packet: Packet) {
        let routing_headers = &mut packet.routing_header;
        if routing_headers.hops[routing_headers.hop_index] != self.id {
            self.send_nack(routing_headers, NackType::UnexpectedRecipient(self.id));
            return;
        }

        routing_headers.hop_index += 1;

        if routing_headers.hop_index == routing_headers.hops.len() {
            self.send_nack(routing_headers, NackType::DestinationIsDrone);
            return;
        }

        let next_hop = routing_headers.hops[routing_headers.hop_index];

        if !self.is_next_hop_neighbour(next_hop) {
            self.send_nack(routing_headers, NackType::ErrorInRouting(next_hop));
            return;
        }

        match packet.pack_type {
            PacketType::MsgFragment(_) => {
                let should_drop = self.rng.gen_range(0.0..1.0) < self.pdr;
                if should_drop {
                    self.send_nack(routing_headers, NackType::Dropped);
                } else {
                    self.send_valid_packet(next_hop, packet);
                }
            }
            PacketType::Nack(_) => {
                self.send_valid_packet(next_hop, packet);
            }
            PacketType::Ack(_) => {
                self.send_valid_packet(next_hop, packet);
            }
            PacketType::FloodRequest(_) => {}
            PacketType::FloodResponse(_) => {}
        }
    }

    fn add_sender(&mut self, id: NodeId, sender: Sender<Packet>) {
        self.packet_send.insert(id, sender);
    }

    fn set_packet_drop_rate(&mut self, pdr: f32) {
        self.pdr = pdr;
    }

    fn send_packet_sent_event(&mut self, packet: Packet) {
        self.controller_send
            .send(PacketSent(packet))
            .expect("Failed to send message to simulation controller");
    }
    fn send_packet_dropped_event(&mut self, packet: Packet) {
        self.controller_send
            .send(PacketDropped(packet))
            .expect("Failed to send message to simulation controller");
    }

    fn reverse_headers(source_routing_header: &SourceRoutingHeader) -> SourceRoutingHeader {
        let mut new_hops = source_routing_header.hops[..source_routing_header.hop_index].to_vec();
        new_hops.reverse();
        let new_headers = SourceRoutingHeader {
            hops: new_hops,
            hop_index: 1,
        };
        new_headers
    }

    fn send_nack(&mut self, routing_headers: &SourceRoutingHeader, nack_type: NackType) {
        let new_headers = Self::reverse_headers(routing_headers);

        let is_dropped = match &nack_type {
            NackType::Dropped => true,
            _ => false,
        };

        let next_hop = new_headers.hops[1];

        let nack = Packet {
            pack_type: PacketType::Nack(Nack {
                fragment_index: 0,
                nack_type,
            }),
            routing_header: new_headers,
            session_id: 0,
        };

        if is_dropped {
            self.drop_packet(next_hop, nack)
        } else {
            self.send_valid_packet(next_hop, nack);
        }
    }

    fn send_packet(&mut self, dest_id: NodeId, packet: Packet) {
        let sender = self.packet_send.get(&dest_id);

        match sender {
            None => {}
            Some(sender) => {
                sender.send(packet).expect("Sender should be valid");
            }
        }
    }

    fn send_valid_packet(&mut self, dest_id: NodeId, packet: Packet) {
        self.send_packet(dest_id, packet.clone());
        self.send_packet_sent_event(packet);
    }
    fn drop_packet(&mut self, dest_id: NodeId, packet: Packet) {
        self.send_packet(dest_id, packet.clone());
        self.send_packet_dropped_event(packet);
    }

    fn is_next_hop_neighbour(&self, next_hop: NodeId) -> bool {
        let sender = self.packet_send.get(&next_hop);
        match sender {
            None => false,
            Some(_) => true,
        }
    }
}

fn main() {
    println!("Hello, world!");
}
