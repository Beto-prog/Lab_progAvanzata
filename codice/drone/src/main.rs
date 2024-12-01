use crossbeam_channel::{select_biased, Sender};
use rand::rngs::ThreadRng;
use rand::Rng;
use wg_2024::controller::DroneCommand;
use wg_2024::controller::NodeEvent::{PacketDropped, PacketSent};
use wg_2024::drone::{Drone, DroneOptions};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Nack, NackType, Packet, PacketType};

struct TrustDrone {
    options: DroneOptions,
    rng: ThreadRng,
}

impl Drone for TrustDrone {
    fn new(options: DroneOptions) -> Self {
        TrustDrone { options, rng: rand::thread_rng() }
    }

    fn run(&mut self) {
        loop {
            select_biased! {
                 recv(self.options.controller_recv) -> command => {
                    match command {
                        Ok(command) => {
                            match command {
                                DroneCommand::AddSender(id,sender) => {
                                    self.add_sender(id, sender);
                                },
                                DroneCommand::SetPacketDropRate(pdr) => {
                                    self.set_packet_drop_rate(pdr);
                                },
                                DroneCommand::Crash => {
                                    println!("Drone with id {} crashed", self.options.id);
                                    break;
                                }
                            }
                        },
                        Err(e) => {
                            eprintln!("Error on command reception of drone {}: {}", self.options.id, e)
                        },
                    }
                }
                recv(self.options.packet_recv) -> packet => {
                    match packet {
                        Ok(mut packet) => {
                            let routing_headers = &mut packet.routing_header;
                            if routing_headers.hops[routing_headers.hop_index] != self.options.id {
                                self.send_nack(routing_headers, NackType::UnexpectedRecipient(self.options.id));
                                continue
                            }

                            routing_headers.hop_index += 1;

                            if routing_headers.hop_index == routing_headers.hops.len() {
                                self.send_nack(routing_headers, NackType::DestinationIsDrone);
                                continue
                            }

                            let next_hop = routing_headers.hops[routing_headers.hop_index];

                            if !self.is_next_hop_neighbour(next_hop) {
                                self.send_nack(routing_headers, NackType::ErrorInRouting(next_hop));
                                continue
                            }


                            match packet.pack_type {
                              PacketType::MsgFragment(_) => {
                                    let should_drop = self.rng.gen_range(0.0..1.0) < self.options.pdr;
                                    if should_drop {
                                        self.send_nack(routing_headers, NackType::Dropped);
                                    } else {
                                        self.send_packet(next_hop, packet);
                                    }
                                },
                              PacketType::Nack(_) => {
                                        self.send_packet(next_hop, packet);
                                },
                              PacketType::Ack(_) => {
                                        self.send_packet(next_hop, packet);
                                },
                              PacketType::FloodRequest(_) => {},
                              PacketType::FloodResponse(_) => {}
                            }
                        },
                        Err(e) => {
                            eprintln!("Error on packet reception of drone {}: {}", self.options.id, e)
                        }
                    }
                }
            }
        }
    }
}

impl TrustDrone {
    fn add_sender(&mut self, id: NodeId, sender: Sender<Packet>) {
        self.options.packet_send.insert(id, sender);
    }

    fn set_packet_drop_rate(&mut self, pdr: f32) {
        self.options.pdr = pdr;
    }

    fn send_packet_sent_event(&mut self, packet: Packet) {
        self.options
            .controller_send
            .send(PacketSent(packet))
            .expect("Failed to send message to simulation controller");
    }
    fn send_packet_dropped_event(&mut self, packet: Packet) {
        self.options
            .controller_send
            .send(PacketDropped(packet))
            .expect("Failed to send message to simulation controller");
    }

    fn send_nack(&mut self, routing_headers: &SourceRoutingHeader, nack_type: NackType) {
        let mut new_headers = routing_headers.hops[..routing_headers.hop_index].to_vec();
        new_headers.reverse();

        let is_dropped = match &nack_type {
            NackType::Dropped => true,
            _ => false,
        };

        let next_hop = new_headers[1];
        let nack = Packet {
            pack_type: PacketType::Nack(Nack { fragment_index: 0, nack_type }),
            routing_header: SourceRoutingHeader {
                hops: new_headers,
                hop_index: 1,
            },
            session_id: 0,
        };

        if is_dropped {
            self.drop_packet(next_hop, nack)
        } else {
            self.send_packet(next_hop, nack);
        }
    }

    fn send_packet(&mut self, dest_id: NodeId, packet: Packet) {
        let sender = self.options.packet_send.get(&dest_id);

        match sender {
            None => {}
            Some(sender) => {
                sender.send(packet.clone()).expect("Sender should be valid");
                self.send_packet_sent_event(packet);
            }
        }
    }

    fn drop_packet(&mut self, dest_id: NodeId, packet: Packet) {
        let sender = self.options.packet_send.get(&dest_id);

        match sender {
            None => {}
            Some(sender) => {
                sender.send(packet.clone()).expect("Sender should be valid");
                self.send_packet_dropped_event(packet);
            }
        }
    }

    fn is_next_hop_neighbour(&self, next_hop: NodeId) -> bool {
        let sender = self.options.packet_send.get(&next_hop);
        match sender {
            None => false,
            Some(_) => true,
        }
    }
}

fn main() {
    println!("Hello, world!");
}
