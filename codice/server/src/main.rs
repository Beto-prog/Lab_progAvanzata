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
use wg_2024::packet::PacketType::Ack as AckType;
use message::net_work as NewWork;
use NewWork::bfs_shortest_path;
use crate::message::packaging::Repackager;
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
    packet_send: HashMap<NodeId, Sender<Packet>>,   //directly connected neighbour.  
    server_type: ServerType,
    
    // extra field

    graph: HashMap<NodeId, Vec<NodeId>>,            //I nees this for bfs
    packageHandler : Repackager
    
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
            server_type,
            packageHandler: Repackager::new(),
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
        
        //first thing you do is calculatre the right path to deliver the packet
        let source = packet.routing_header.hop_index;
        let path = bfs_shortest_path(&self.graph, self.id, source as NodeId);
        
        match path { 
            Some(x) => {

                //start creating the new packet
                let mut response = Packet
                {
                    routing_header:x,
                    session_id: packet.session_id + 1,     
                    pack_type: packet.pack_type.clone()        //just to fill up the filed it will be changed in a moment
                };
        
        
        match packet.pack_type {
            PacketType::MsgFragment(mesg) => {

                //Send back an ack 
                 response.pack_type = AckType(Ack{
                     fragment_index: mesg.fragment_index ,
                 });
                
                let result = self.packageHandler.process_fragment(packet.session_id ,packet.routing_header.hop_index as u64,mesg);
                //let message = Repackager::reassembled_to_string(result);
                
                
   
                

            }
            PacketType::Nack(_) => {
               // self.send_valid_packet(next_hop, packet);
           /*     let c = Packet{
                    routing_header: Default::default(),
                    session_id: 0,
                    pack_type: PacketType::Ack(wg_2024::wg_packet::Ack { fragment_index: 1 }),
                };*/
            }
            PacketType::Ack(_) => {
                //self.send_valid_packet(next_hop, packet);
            }
       
        
            PacketType::FloodResponse(path) => {
               NewWork::recive_flood_response(&mut self.graph, path.path_trace);
                
            }
    
            PacketType::FloodRequest(mut flood_packet) =>
                {
                    //The Server is note a drone so when he recive a flood request he can send back a flood response with no problem
                    
                    let response = flood_packet.generate_response(packet.session_id);
                    NewWork::recive_flood_response(&mut self.graph, flood_packet.path_trace);

                    
                    //self.send_packet(previous_neighbour, flood_packet.generate_response(42));
                }
            
            _ => {panic!("Impossible")} // I have to put this because i moved the FloodRequest at the beginning
            
            
        }

            }
            None => {println!("No path found for source {}  found !!ERROR!!", source);}
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