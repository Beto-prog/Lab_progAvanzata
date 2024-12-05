use crossbeam_channel::{select_biased, Receiver, Sender};
use rand::rngs::ThreadRng;
use rand::Rng;
use std::collections::HashMap;
use wg_2024::controller::DroneEvent::{PacketDropped, PacketSent};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Nack, NackType, Packet, PacketType};

struct TrustDrone {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    rng: ThreadRng, //The random number generator
}

impl Drone for TrustDrone {
    fn new(
        id: NodeId,
        controller_send: Sender<DroneEvent>,
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
        // The drone runs in an infinite loop, when it receives either a command or packet it processes it, giving priority to the command
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
            DroneCommand::RemoveSender(neighbor_id) => self.remove_sender(neighbor_id),
        }
    }

    fn handle_packet(&mut self, mut packet: Packet) {

        //create a reference to the routing headers just to have a shorthand

        let routing_headers = &mut packet.routing_header;

        //Step 1 of the protocol
        if routing_headers.hops[routing_headers.hop_index] != self.id {
            self.send_nack(routing_headers, NackType::UnexpectedRecipient(self.id));
            return;
        }

        //Step 2
        routing_headers.hop_index += 1;

        //Step 3
        if routing_headers.hop_index == routing_headers.hops.len() {
            self.send_nack(routing_headers, NackType::DestinationIsDrone);
            return;
        }

        //step 4
        let next_hop = routing_headers.hops[routing_headers.hop_index];

        if !self.is_next_hop_neighbour(next_hop) {
            self.send_nack(routing_headers, NackType::ErrorInRouting(next_hop));
            return;
        }

        //step 5
        match packet.pack_type {
            PacketType::MsgFragment(_) => {
                //check if it should drop
                let should_drop = self.rng.gen_range(0.0..1.0) < self.pdr;
                if should_drop {
                    self.send_nack(routing_headers, NackType::Dropped);
                } else {
                    self.send_ack(routing_headers);
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

    fn remove_sender(&mut self, neighbor_id: NodeId) {
        self.packet_send.retain(|id, _| *id != neighbor_id);
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

    //reverse the headers to send nacks and acks
    fn reverse_headers(source_routing_header: &SourceRoutingHeader) -> SourceRoutingHeader {
        let mut new_hops = source_routing_header.hops[..source_routing_header.hop_index].to_vec();
        new_hops.reverse();
        let new_headers = SourceRoutingHeader {
            hops: new_hops,
            hop_index: 1,
        };
        new_headers
    }

    fn send_ack(&mut self, routing_headers: &SourceRoutingHeader) {
        let new_headers = Self::reverse_headers(routing_headers);
        let next_hop = new_headers.hops[1];

        let ack = Packet {
            pack_type: PacketType::Ack(Ack {
                fragment_index: 0,
            }),
            routing_header: new_headers,
            session_id: 0,
        };

        self.send_valid_packet(next_hop, ack);
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

#[cfg(test)]
mod tests {
    //use std::thread;
    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_add_sender() {
        //Create a drone for testing
        let id: u8 = 123;
        let pdr: f32 = 0.5;
        let mut packet_channels = HashMap::<NodeId, (Sender<Packet>, Receiver<Packet>)>::new();
        packet_channels.insert(id, unbounded());

        //controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        let (node_event_send, node_event_recv) = unbounded();

        //packet
        let packet_recv = packet_channels[&id].1.clone();
        let packet_sender = packet_channels[&id].0.clone();
        let packet_send = HashMap::<NodeId, Sender<Packet>>::new();

        //drone instance
        let mut drone = TrustDrone::new(
            id,
            node_event_send,
            controller_drone_recv,
            packet_recv,
            packet_send,
            pdr,
        );
        //test
        let id_test:u8 = 234;
        drone.add_sender(id_test,packet_sender);
        match drone.packet_send.get(&id_test){
            Some(r) => (),
            None =>{panic!("Error: packet_send not found/inserted correctly")} // used panic! because I should have written impl of Eq for Sender<Packet>
        }


    }
    #[test]
    fn test_set_packet_drop_rate(){
        let id: u8 = 123;
        let pdr: f32 = 0.5;
        let mut packet_channels = HashMap::<NodeId, (Sender<Packet>, Receiver<Packet>)>::new();
        packet_channels.insert(id, unbounded());

        //controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        let (node_event_send, node_event_recv) = unbounded();

        //packet
        let packet_recv = packet_channels[&id].1.clone();
        let packet_sender = packet_channels[&id].0.clone();
        let packet_send = HashMap::<NodeId, Sender<Packet>>::new();

        //drone instance
        let mut drone = TrustDrone::new(
            id,
            node_event_send,
            controller_drone_recv,
            packet_recv,
            packet_send,
            pdr,
        );

        //test
        drone.set_packet_drop_rate(0.7);
        assert_eq!(drone.pdr, 0.7);
    }
    #[test]
    fn test_handle_command(){
        let id: u8 = 123;
        let pdr: f32 = 0.5;
        let mut packet_channels = HashMap::<NodeId, (Sender<Packet>, Receiver<Packet>)>::new();
        packet_channels.insert(id, unbounded());

        //controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        let (node_event_send, node_event_recv) = unbounded();

        //packet
        let packet_recv = packet_channels[&id].1.clone();
        let packet_sender = packet_channels[&id].0.clone();
        let packet_send = HashMap::<NodeId, Sender<Packet>>::new();

        //drone instance
        let mut drone = TrustDrone::new(
            id,
            node_event_send,
            controller_drone_recv,
            packet_recv,
            packet_send,
            pdr,
        );

        //test
        let packet_sender_test = packet_sender.clone();
        let id_test = 234;
        let dc1 = DroneCommand::AddSender(id_test,packet_sender);
        let dc2 = DroneCommand::SetPacketDropRate(0.7);
        let dc3 = DroneCommand::RemoveSender(id_test);
        let dc4 = DroneCommand::Crash;

        //test AddSender
        drone.handle_command(dc1);
        match drone.packet_send.get(&id_test) {
            Some(r) =>{

                //test RemoveSender
                drone.handle_command(dc3);
                match drone.packet_send.get(&id_test){
                    Some(r) => {panic!("Error: sender should have been eliminated")}
                    None => ()
                }
            },
            None => {panic!("Error: packet_send not found/inserted correctly")}
        }

        //test SetPacketDropRate
        drone.handle_command(dc2);
        assert_eq!(drone.pdr, 0.7);

        //test RemoveSender
        // TODO test Crash drone.handle_command(dc4);

    }
    #[test]
    fn test_handle_packet(){
        let id: u8 = 123;
        let pdr: f32 = 0.5;
        let mut packet_channels = HashMap::<NodeId, (Sender<Packet>, Receiver<Packet>)>::new();
        packet_channels.insert(id, unbounded());

        //controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        let (node_event_send, node_event_recv) = unbounded();

        //packet
        let packet_recv = packet_channels[&id].1.clone();
        let packet_sender = packet_channels[&id].0.clone();
        let packet_send = HashMap::<NodeId, Sender<Packet>>::new();

        //drone instance
        let mut drone = TrustDrone::new(
            id,
            node_event_send,
            controller_drone_recv,
            packet_recv,
            packet_send,
            pdr,
        );
        let id2: u8 = 234;
        let pdr2: f32 = 0.7;
        let mut packet_channels2 = HashMap::<NodeId, (Sender<Packet>, Receiver<Packet>)>::new();
        packet_channels2.insert(id, unbounded());

        //controller
        let (controller_drone_send2, controller_drone_recv2) = unbounded();
        let (node_event_send2, node_event_recv2) = unbounded();

        //packet
        let packet_recv2 = packet_channels[&id].1.clone();
        let packet_sender2 = packet_channels[&id].0.clone();
        let packet_send2 = HashMap::<NodeId, Sender<Packet>>::new();

        //drone instance
        let mut drone = TrustDrone::new(
            id2,
            node_event_send2,
            controller_drone_recv2,
            packet_recv2,
            packet_send2,
            pdr2,
        );

    }
    /*

    #[test]
    fn test_send_packet_sent_event(){}
    #[test]
    fn test_send_packet_dropped_event(){}
    #[test]
    fn test_reverse_headers(){}
    #[test]
    fn test_send_nack(){}
    #[test]
    fn test_send_packet(){} // vedere se testare o meno, abbastanza ez
    #[test]
    fn test_send_valid_packet(){}
    #[test]
    fn test_drop_packet(){}
    #[test]
    fn test_is_next_hop_neighbour(){}

     */
}
fn main() {
    println!("Hello, world!");
}
