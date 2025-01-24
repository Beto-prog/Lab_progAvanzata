#![allow(warnings)]


mod message;

use wg_2024::network::*;
use std::collections::{HashMap, VecDeque};
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use rand::rngs::StdRng;
use rand::{random, Rng, SeedableRng};

use wg_2024::controller::DroneEvent::{ControllerShortcut, PacketDropped, PacketSent};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::NodeType::{Drone as DroneType, Server as OtherServer};
use wg_2024::packet::{Ack, FloodResponse, Nack, NackType, Packet, PacketType};
use message::net_work as NewWork;
use crate::ServerType::TextServer;

pub enum ServerType
{
    TextServer,
    MediaServer,
}
struct  Server 
{
    id: NodeId,
    packet_recv: Receiver<Packet>,
    graph: HashMap<NodeId, Vec<NodeId>>,
    packet_send: HashMap<NodeId, Sender<Packet>>,   //directly connected neighbour.  
    server_type: ServerType,
}


impl Server
{

    fn new(
        id: NodeId,
        packet_recv: Receiver<Packet>,
        packet_send : HashMap<NodeId, Sender<Packet>>,
        server_type: ServerType,
    ) -> Self {
        
        let mut graph = HashMap::new();
        for (key,_) in packet_send.iter() {  //start by filling the graph with the directly connected neighbour
            graph.insert(*key,Vec::new());
        }
        
        Server {
            id,
            packet_recv,
            graph,
            packet_send,
            server_type
        }
    }    
    
    /*
Start the flood protocol to fill up the hashmap and create the tree of the graph.
Small remainder the hash map is composed in this way HashMap<NodeId, Vec<NodeId>> , The NodeId 
and the list of it's neighbour 
 */

    fn initialization(&mut self)
    {

    }

    fn start(&mut self)
    {

    }


    fn run(&mut self) {
        loop {
            select_biased! {        
                recv(self.packet_recv) -> packet => {
                    match packet {
                        Ok(packet) => {
                          // self.handle_packet(packet);
                        },
                        Err(e) => {
                            eprintln!("Error packet reception of server {}: {}", self.id, e)
                        }
                    }
                }
            }
        }
    }

    
    
    
    /*
    struct SourceRoutingHeader {
	// must be set to 1 initially by the sender
	hop_index: usize,
	// Vector of nodes with initiator and nodes to which the packet will be forwarded to.
	hops: Vec<NodeId>
}

    Packet 
    {
        routing_header 
                hop_index: usize,
                hops: Vec<NodeId>,
        pack_type
        session_id
    }
    
    */
     
    /*
    Rule :
        1 - Each time you receive a packet you send an Ack
        2 - Nack : you have to send again the packet
        -----Problema da risolvere con limit case 
        
        3 - Ogni pacchetto e frammentato 
        
    
     */
    
    
    
    fn handle_packet(&mut self, mut packet: Packet) {
        match packet.pack_type {
            PacketType::MsgFragment(_) => {
                //elaborate the packet you have to reassable it 
                
                /*
                
                To reassemble fragments into a single packet, a client or server uses the fragment header as follows:
                
                    1. The client or server receives a fragment.
                    2. It first checks the (session_id, src_id) tuple in the header.    
                    3. If it has not received a fragment with the same (session_id, src_id) tuple, then it creates a vector (Vec<u8> with capacity of total_n_fragments * 128) where to copy the data of the fragments.
                    4. It would then copy length elements of the data array at the correct offset in the vector.

                
                */
            }
            PacketType::Nack(_) => {
               // self.send_valid_packet(next_hop, packet);
            }
            PacketType::Ack(_) => {
                //self.send_valid_packet(next_hop, packet);
            }
       
        
            PacketType::FloodResponse(path) => {
               NewWork::recive_flood_response(&mut self.graph, path.path_trace);
                
            }
    
            PacketType::FloodRequest(mut flood_packet) =>
                {
                    flood_packet.increment(self.id, OtherServer);
                    //self.send_packet(previous_neighbour, flood_packet.generate_response(42));
                }
            
            _ => {panic!("Impossible")} // I have to put this because i moved the FloodRequest at the beginning
            
            
        }
    }




    fn send_packet(&mut self, dest_id: NodeId, packet: Packet) {
        let sender = self.packet_send.get(&dest_id);
        match sender {
            None => match packet.pack_type {
                PacketType::Ack(_) | PacketType::Nack(_) | PacketType::FloodResponse(_) => {
                   // self.send_shortcut(packet);           ---- SImulation control
                }
                _ => (),
            },
            Some(sender) => {
                sender.send(packet).expect("Sender should be valid");
            }
        }
    }
  


}


fn main() {

}