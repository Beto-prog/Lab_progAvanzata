use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use rand::rngs::StdRng;
use rand::{random, Rng, SeedableRng};

// For a better understanding of the code please read the protocol specification at : https://github.com/WGL-2024/WGL_repo_2024/blob/main/AP-protocol.md


use std::collections::HashMap;
use std::process::exit;
use wg_2024::controller::DroneEvent::{ControllerShortcut, PacketDropped, PacketSent};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::NodeType::Drone as DroneType;
use wg_2024::packet::{Ack, FloodResponse, Nack, NackType, Packet, PacketType};
/*
================================================================================================
                                    TrustNode Documentation

TrustNode is the implementation of the drone controller used in the simulation.

- controller_send:
    Channel used to send events to the simulation controller.
    Events are represented by the DroneEvent enum, which includes:
    - PacketSent
    - PacketDropped
    - ControllerShortcut.

- controller_recv:
    Channel used to receive commands from the simulation controller.
    These commands modify the drone's behavior during the simulation and include:
    - AddSender
    - RemoveSender
    - SetPacketDropRate
    - Crash.

    Note: Both controller_send and controller_recv channels are exclusively used for
    simulation purposes and are not involved in communication between drones.

- packet_recv:
    Channel used to receive packets from other drones.

- packet_send:
    Channel used to send packets to other drones.

- pdr (Packet Drop Rate):
    The rate at which packets are dropped, determining the likelihood of discarding a packet.

- rng (Random Number Generator):
    A random number generator used to decide whether a packet is dropped based on the
    configured packet drop rate.


    Note: Sender and Receiver are part of the crossbeam_channel is a useful tool that allow
          process (thread) to communicate with each other.
================================================================================================
*/


struct TrustDrone {
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    pdr: f32,
    packet_send: HashMap<NodeId, Sender<Packet>>,

    rng: StdRng, //The random number generator

    flood_ids: Vec<u64>,
}


//just the initialization of the drone
impl Drone for TrustDrone {
    fn new(
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Self {
        let random_seed: u64 = random();
        TrustDrone {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,

            rng: StdRng::seed_from_u64(random_seed),

            flood_ids: Vec::new(),
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
    // This is the part that handle command received from the simulation controller (it has nothing to do with the packet exchange)
    fn handle_command(&mut self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(id, sender) => {
                self.add_sender(id, sender);
            }
            DroneCommand::SetPacketDropRate(pdr) => {
                self.set_packet_drop_rate(pdr);
            }
            DroneCommand::Crash => {
                self.crash_behavior();
                println!("Drone with id {} crashed", self.id);
                exit(0);
            }
            DroneCommand::RemoveSender(neighbor_id) => self.remove_sender(neighbor_id),
        }
    }


    /*
Structure of a Packet (Packet)

A Packet is the fundamental unit of communication in the network.
It is composed of several fields that define its content and behavior.

Fields of Packet:
- pack_type: PacketType
  Specifies the type of the packet and its role in the network. It is an enumeration with the following variants:
    - MsgFragment(Fragment): Represents a fragment of a higher-level message, used for data transport.
    - Ack(Ack): Confirms the successful receipt of a fragment.
    - Nack(Nack): Indicates an error during processing or routing of a packet.
    - FloodRequest(FloodRequest): Used for network topology discovery via query flooding.
    - FloodResponse(FloodResponse): A response to a flooding request, containing topology information.

- routing_header: SourceRoutingHeader
  Contains routing information for the packet. This enables source-based routing, where the sender precomputes the entire path the packet should take through the network.
  Components:
    - hop_index: usize
      Indicates the current hop's position in the hops list. Starts at 1 when the packet is first sent.
    - hops: Vec<NodeId>
      A list of node identifiers representing the full path from the sender to the destination.

- session_id: u64
  A unique identifier for the session associated with the packet. It helps group related packets, such as fragments of the same message, and differentiates them from others in the network.

Packets are routed through the network using the information in the routing_header. The type, defined by pack_type, determines how each packet is processed and its role in the communication flow.
*/


    // This is the part that handle packet received from the other drones.
    fn handle_packet(&mut self, mut packett: Packet) {
        let mut packet = packett.clone();   //used because flooding needs the original packet

        let old_routing_headers = packet.routing_header.clone();

        //Step 1 of the protocol , if the packet was not meant for him
        if packet.routing_header.hops[packet.routing_header.hop_index] != self.id {
            self.send_nack(&old_routing_headers, NackType::UnexpectedRecipient(self.id), packet.session_id, packet.get_fragment_index());
            return;
        }

        //Step 2
        packet.routing_header.hop_index += 1;

        //Step 3,
        if packet.routing_header.hop_index == packet.routing_header.hops.len() {
            self.send_nack(&old_routing_headers, NackType::DestinationIsDrone, packet.session_id, packet.get_fragment_index());
            return;
        }

        //step 4, check if the node to which it must send the packet is one of his neighbour
        let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];

        if !self.is_next_hop_neighbour(next_hop) {
            self.send_nack(&old_routing_headers, NackType::ErrorInRouting(next_hop), packet.session_id, packet.get_fragment_index());
            return;
        }

        //step 5
        match packet.pack_type {
            PacketType::MsgFragment(_) => {
                //check if it should drop
                let should_drop = self.rng.gen_range(0.0..1.0) <= self.pdr;
                if should_drop {
                    self.send_nack(&old_routing_headers, NackType::Dropped, packet.session_id, packet.get_fragment_index());
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



            /*
             pub struct FloodRequest {
                 flood_id: u64,               Unique identifier for the flooding operation
                 initiator_id: NodeId,        ID of the node that started the flooding
                 path_trace: Vec<(NodeId, NodeType)>, // Trace of nodes traversed during the flooding
             }
         */

            PacketType::FloodRequest(mut flood_packet) => {
                flood_packet.increment(self.id, DroneType);

                let mut previous_neighbour = 0;
                if let Some(last) = flood_packet.path_trace.last()
                {
                    previous_neighbour = last.0;
                } else {
                    panic!("Can not find neighbour who send this packet {} ", flood_packet);
                }

                if self.flood_ids.contains(&flood_packet.flood_id)      // if the drone had already seen this FloodRequest  it sends a FloodResponse back
                {
                    self.send_packet(previous_neighbour, flood_packet.generate_response(7));  //send back   !!!!!!!!!!Session id unknown

                } else {
                    self.flood_ids.push(flood_packet.flood_id); //save the flood id for next use

                    if self.packet_send.len() - 1 == 0 {
                        //if there are no neighbour send back flooding response 

                        self.send_packet(previous_neighbour, flood_packet.generate_response(7));  //send back   !!!!!!!!!!Session id unknown

                    } else {    //send packet to all the neibourgh except the sender
                        for (key, _) in self.packet_send.clone() {
                            if key != previous_neighbour {
                                let mut cloned_packett = packett.clone();
                                cloned_packett.pack_type = PacketType::FloodResponse(FloodResponse {
                                    flood_id: flood_packet.flood_id,
                                    path_trace: flood_packet.path_trace.clone(),
                                });
                                self.send_packet(key, cloned_packett);
                            }
                        }
                    }
                }
            }

            PacketType::FloodResponse(_) => {
                self.send_valid_packet(next_hop, packet);
            }
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
        if let Err(e) = self.controller_send.send(PacketDropped(packet)) {
            println!("{}", e);
        }
    }

    //reverse the headers to send nacks and acks
    fn reverse_headers(source_routing_header: &SourceRoutingHeader) -> SourceRoutingHeader {
        let mut new_hops = source_routing_header.hops[..source_routing_header.hop_index + 1].to_vec();
        new_hops.reverse();
        let new_headers = SourceRoutingHeader {
            hops: new_hops,
            hop_index: 1,
        };
        new_headers
    }

    fn send_nack(&mut self, routing_header: &SourceRoutingHeader, nack_type: NackType, session_id: u64, fragment_index: u64) {
        let new_headers = Self::reverse_headers(routing_header);

        let is_dropped = match &nack_type {
            NackType::Dropped => true,
            _ => false,
        };

        let next_hop = new_headers.hops[1];

        let nack = Packet {
            pack_type: PacketType::Nack(Nack {
                fragment_index,
                nack_type,
            }),
            routing_header: new_headers,
            session_id,
        };

        if is_dropped {
            self.drop_packet(next_hop, nack)
        } else {
            self.send_valid_packet(next_hop, nack);
        }
    }

    fn send_shortcut(&mut self, packet: Packet) {
        if let Err(e) = self.controller_send.send(ControllerShortcut(packet)) {
            println!("{}", e);
        }
    }

    fn send_packet(&mut self, dest_id: NodeId, packet: Packet) {
        let sender = self.packet_send.get(&dest_id);

        match sender {
            None => {
                match packet.pack_type {
                    PacketType::Ack(_) | PacketType::Nack(_) | PacketType::FloodResponse(_) => {
                        self.send_shortcut(packet);
                    }
                    _ => ()
                }
            }
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

    fn crash_behavior(&mut self) {
        println!("Drone {} entering crashing behavior...", self.id);

        // Process remaining messages until the channel is closed and empty
        while let Ok(packet) = self.packet_recv.recv() {
            let mut packet = packet.clone();
            let old_routing_headers = packet.routing_header.clone();

            match packet.pack_type {
                // FloodRequests can be lost during crash
                PacketType::FloodRequest(_) => {
                    // Simply ignore/drop the packet
                    continue;
                }

                // Ack, Nack and FloodResponse should still be forwarded
                PacketType::Ack(_) | PacketType::Nack(_) | PacketType::FloodResponse(_) => {
                    // Basic routing checks still apply
                    if packet.routing_header.hops[packet.routing_header.hop_index] != self.id {
                        continue; // Skip invalid packets during crash
                    }

                    packet.routing_header.hop_index += 1;

                    if packet.routing_header.hop_index < packet.routing_header.hops.len() {
                        let next_hop = packet.routing_header.hops[packet.routing_header.hop_index];
                        if self.is_next_hop_neighbour(next_hop) {
                            self.send_valid_packet(next_hop, packet);
                        }
                    }
                }

                // For all other packet types (MsgFragment), send ErrorInRouting Nack
                _ => {
                    self.send_nack(
                        &old_routing_headers,
                        NackType::ErrorInRouting(self.id),
                        packet.session_id,
                        packet.get_fragment_index(),
                    );
                }
            }
        }

        println!("Drone {} has successfully crashed.", self.id);
    }

    /*
    implemented elsewhere
       fn send_flood_request()
       {todo!()}
       fn send_flood_response(){todo!()}*/
}

#[cfg(test)]
mod tests {
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
        let id_test: u8 = 234;
        drone.add_sender(id_test, packet_sender);
        match drone.packet_send.get(&id_test) {
            Some(r) => (),
            None => { panic!("Error: packet_send not found/inserted correctly") } // used panic! because I should have written impl of Eq for Sender<Packet>
        }
    }
    #[test]
    fn test_set_packet_drop_rate() {
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
    fn test_handle_command() {
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
        let dc1 = DroneCommand::AddSender(id_test, packet_sender);
        let dc2 = DroneCommand::SetPacketDropRate(0.7);
        let dc3 = DroneCommand::RemoveSender(id_test);
        let dc4 = DroneCommand::Crash;

        //test AddSender
        drone.handle_command(dc1);
        match drone.packet_send.get(&id_test) {
            Some(r) => {

                //test RemoveSender
                drone.handle_command(dc3);
                match drone.packet_send.get(&id_test) {
                    Some(r) => { panic!("Error: sender should have been eliminated") }
                    None => ()
                }
            }
            None => { panic!("Error: packet_send not found/inserted correctly") }
        }

        //test SetPacketDropRate
        drone.handle_command(dc2);
        assert_eq!(drone.pdr, 0.7);

        //test RemoveSender
        // TODO test Crash drone.handle_command(dc4);

    }
}

use std::thread;
use wg_2024::packet::Fragment;
/* THE FOLLOWING TESTS CHECKS IF YOUR DRONE IS HANDLING CORRECTLY PACKETS (FRAGMENT) */

/// Creates a sample packet for testing purposes. For convenience, using 1-10 for clients, 11-20 for drones and 21-30 for servers
///
fn create_sample_packet() -> Packet {
    Packet {
        pack_type: PacketType::MsgFragment(Fragment {
            fragment_index: 1,
            total_n_fragments: 1,
            length: 128,
            data: [1; 128],
        }),
        routing_header: SourceRoutingHeader {
            hop_index: 1,
            hops: vec![1, 11, 12, 21],
        },
        session_id: 1,
    }
}
#[test]
/// This function is used to test the packet forward functionality of a drone.
pub fn generic_fragment_forward() {
    // drone 2 <Packet>
    let (d_send, d_recv) = unbounded();
    // drone 3 <Packet>
    let (d2_send, d2_recv) = unbounded::<Packet>();
    // SC commands
    let (_d_command_send, d_command_recv) = unbounded();

    let neighbours = HashMap::from([(12, d2_send.clone())]);
    let mut drone = TrustDrone::new(
        11,
        unbounded().0,
        d_command_recv,
        d_recv.clone(),
        neighbours,
        0.0,
    );
    // Spawn the drone's run method in a separate thread
    thread::spawn(move || {
        drone.run();
    });

    let mut msg = create_sample_packet();

    // "Client" sends packet to d
    d_send.send(msg.clone()).unwrap();
    msg.routing_header.hop_index = 2;
    let t = d2_recv.recv().unwrap();
    // d2 receives packet from d
    assert_eq!(t, msg);
}

#[test]
/// Checks if the packet is dropped by one drone. The drone MUST have 100% packet drop rate, otherwise the test will fail sometimes.
pub fn generic_fragment_drop() {
    // Client 1
    let (c_send, c_recv) = unbounded();
    // Drone 11
    let (d_send, d_recv) = unbounded();
    // SC commands
    let (_d_command_send, d_command_recv) = unbounded();

    let neighbours = HashMap::from([(12, d_send.clone()), (1, c_send.clone())]);
    let mut drone = TrustDrone::new(
        11,
        unbounded().0,
        d_command_recv,
        d_recv.clone(),
        neighbours,
        1.0,
    );

    // Spawn the drone's run method in a separate thread
    thread::spawn(move || {
        drone.run();
    });

    let msg = create_sample_packet();

    // "Client" sends packet to the drone
    d_send.send(msg.clone()).unwrap();

    let dropped = Nack {
        fragment_index: 1,
        nack_type: NackType::Dropped,
    };
    let srh = SourceRoutingHeader {
        hop_index: 1,
        hops: vec![11, 1],
    };
    let nack_packet = Packet {
        pack_type: PacketType::Nack(dropped),
        routing_header: srh,
        session_id: 1,
    };

    // Client listens for packet from the drone (Dropped Nack)
    let t = c_recv.recv().unwrap();
    assert_eq!(t, nack_packet);
}

#[test]
/// Checks if the packet is dropped by the second drone. The first drone must have 0% PDR and the second one 100% PDR, otherwise the test will fail sometimes.
pub fn generic_chain_fragment_drop() {


    // Client 1 channels
    let (c_send, c_recv) = unbounded();
    // Server 21 channels
    let (s_send, _s_recv) = unbounded();
    // Drone 11
    let (d_send, d_recv) = unbounded();
    // Drone 12
    let (d12_send, d12_recv) = unbounded();
    // SC - needed to not make the drone crash
    let (_d_command_send, d_command_recv) = unbounded();
    let (d_command_send, _d_command_recv) = unbounded();

    // Drone 11
    let neighbours11 = HashMap::from([(12, d12_send.clone()), (1, c_send.clone())]);
    let mut drone = TrustDrone::new(
        11,
        d_command_send.clone(),
        d_command_recv.clone(),
        d_recv.clone(),
        neighbours11,
        0.0,
    );
    // Drone 12
    let neighbours12 = HashMap::from([(11, d_send.clone()), (21, s_send.clone())]);
    let mut drone2 = TrustDrone::new(
        12,
        d_command_send.clone(),
        d_command_recv.clone(),
        d12_recv.clone(),
        neighbours12,
        1.0,
    );

    // Spawn the drone's run method in a separate thread
    thread::spawn(move || {
        drone.run();
    });

    thread::spawn(move || {
        drone2.run();
    });

    let mut msg = create_sample_packet();

    // "Client" sends packet to the drone
    d_send.send(msg.clone()).unwrap();

    // Client receive an NACK originated from 'd2'
    let t3 = c_recv.recv().unwrap();


    let t4 = Packet {
        pack_type: PacketType::Nack(Nack {
            fragment_index: 1,
            nack_type: NackType::Dropped,
        }),
        routing_header: SourceRoutingHeader {
            hop_index: 2,
            hops: vec![12, 11, 1],
        },
        session_id: 1,
    };


    assert_eq!(t3, t4);
}
/// Checks if the packet can reach its destination. Both drones must have 0% PDR, otherwise the test will fail sometimes.

#[test]
pub fn generic_chain_fragment_ack() {
    // Client<1> channels
    let (c_send, c_recv) = unbounded();
    // Server<21> channels
    let (s_send, s_recv) = unbounded();
    // Drone 11
    let (d_send, d_recv) = unbounded();
    // Drone 12
    let (d12_send, d12_recv) = unbounded();
    // SC - needed to not make the drone crash
    let (_d_command_send, d_command_recv) = unbounded();
    let (d_command_send, _d_command_recv) = unbounded();

    // Drone 11
    let neighbours11 = HashMap::from([(12, d12_send.clone()), (1, c_send.clone())]);
    let mut drone = TrustDrone::new(
        11,
        d_command_send.clone(),
        d_command_recv.clone(),
        d_recv.clone(),
        neighbours11,
        0.0,
    );
    // Drone 12
    let neighbours12 = HashMap::from([(11, d_send.clone()), (21, s_send.clone())]);
    let mut drone2 = TrustDrone::new(
        12,
        d_command_send.clone(),
        d_command_recv.clone(),
        d12_recv.clone(),
        neighbours12,
        0.0,
    );

    // Spawn the drone's run method in a separate thread
    thread::spawn(move || {
        drone.run();
    });

    thread::spawn(move || {
        drone2.run();
    });


    let mut msg = Packet {
        pack_type: PacketType::MsgFragment(Fragment {
            fragment_index: 1,
            total_n_fragments: 1,
            length: 128,
            data: [1; 128],
        }),
        routing_header: SourceRoutingHeader {
            hop_index: 1,
            hops: vec![1, 11, 12, 21],
        },
        session_id: 1,
    };

    // "Client" sends packet to d
    d_send.send(msg.clone()).unwrap();


    // "Server" receives the fragment
    s_recv.recv().unwrap();

    // Server sends Ack to d12
    let ack = Packet {
        pack_type: PacketType::Ack(Ack { fragment_index: 1 }),
        routing_header: SourceRoutingHeader {
            hop_index: 1,
            hops: vec![21, 12, 11, 1],
        },
        session_id: 1,
    };
    d12_send.send(ack.clone()).unwrap();

    // "Client" receives the Ack from d
    let t6 = c_recv.recv().unwrap();
    let ack2 = Packet {
        pack_type: PacketType::Ack(Ack { fragment_index: 1 }),
        routing_header: SourceRoutingHeader {
            hop_index: 3,
            hops: vec![21, 12, 11, 1],
        },
        session_id: 1,
    };
    assert_eq!(t6, ack2);
}


fn main() {

    //generic_chain_fragment_drop();
    generic_chain_fragment_ack();


}
