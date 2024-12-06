// For a better understanding of the code please read the protocol specification at : https://github.com/WGL-2024/WGL_repo_2024/blob/main/AP-protocol.md
use crossbeam_channel::{select_biased, Receiver, Sender};
use rand::rngs::ThreadRng;
use rand::Rng;
use std::collections::HashMap;
use wg_2024::controller::DroneEvent::{PacketDropped, PacketSent};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Nack, NackType, Packet, PacketType, FloodRequest, FloodResponse};
use wg_2024::packet::NodeType::Drone as DroneType;
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
    rng: ThreadRng,
    flood_ids : Vec<u64>,
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
        TrustDrone {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
            rng: rand::thread_rng(),
            flood_ids : Vec::new(),
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
                println!("Drone with id {} crashed", self.id);
                // TODO implement drone crashing
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


        //create a reference to the routing headers just to have a shorthand

        let routing_headers = &mut packet.routing_header;

        //Step 1 of the protocol , if the packet was not meant for him
        if routing_headers.hops[routing_headers.hop_index] != self.id {
            self.send_nack(routing_headers, NackType::UnexpectedRecipient(self.id));
            return;
        }

        //Step 2
        routing_headers.hop_index += 1;

        //Step 3,
        if routing_headers.hop_index == routing_headers.hops.len() {
            self.send_nack(routing_headers, NackType::DestinationIsDrone);
            return;
        }

        //step 4, check if the node to which it must send the packet is one of his neighbour
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



            /*
             pub struct FloodRequest {
                 flood_id: u64,               Unique identifier for the flooding operation
                 initiator_id: NodeId,        ID of the node that started the flooding
                 path_trace: Vec<(NodeId, NodeType)>, // Trace of nodes traversed during the flooding
             }
         */
            
            PacketType::FloodRequest(mut flood_packet) => {
                
                flood_packet.path_trace.push((self.id, DroneType));
                let previous_neighbour = packett.routing_header.hops[packett.routing_header.hop_index - 1];


                if self.flood_ids.contains(&flood_packet.flood_id)      // if the drone had already seen this FloodRequest  it sends a FloodResponse back
                {
                    packett.pack_type =PacketType::FloodResponse(FloodResponse {
                        flood_id: flood_packet.flood_id,
                        path_trace: flood_packet.path_trace.clone(),
                    });
                    self.send_packet(previous_neighbour, packett);  //send back
                    
                } else {
                    self.flood_ids.push(flood_packet.flood_id); //save the flood id for next use
                    
                    
                    if self.packet_send.len()-1 ==0{
                        //if there are no neighbour send back flooding response 

                        packett.pack_type =PacketType::FloodResponse(FloodResponse {
                            flood_id: flood_packet.flood_id,
                            path_trace: flood_packet.path_trace.clone(),
                        });
                        self.send_packet(previous_neighbour, packett);
                        
                        
                    } else {    //send packet to all the neibourgh except the sender
                        for (key, _ ) in  self.packet_send.clone() {
                            
                            if (key != previous_neighbour) {
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

 /*
 implemented elsewhere
    fn send_flood_request()
    {todo!()}
    fn send_flood_response(){todo!()}*/
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
    fn test_handle_packet(){}
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
    fn test_send_packet(){}
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
