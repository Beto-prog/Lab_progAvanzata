#![allow(warnings)]


mod message;

use crossbeam_channel::{select_biased, Receiver, Sender};
use rand::{Rng, SeedableRng};
use std::collections::HashMap;
use std::ops::Index;
use wg_2024::network::*;

use crate::message::file_system::ServerTrait;
use crate::message::packaging::Repackager;
use message::net_work as NewWork;
use wg_2024::drone::Drone;
use wg_2024::network::NodeId;
use wg_2024::packet::PacketType::{Ack as AckType, MsgFragment};
use wg_2024::packet::{Ack, FloodRequest, Fragment, NodeType, Packet, PacketType};
use NewWork::bfs_shortest_path;
use crate::PacketType::FloodRequest as FloodRequestType;

pub use message::file_system;
pub struct  Server  
{
    id: NodeId,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,   //directly connected neighbour.  
    server_type: Box<dyn ServerTrait>,
  
    // extra field
    graph: HashMap<NodeId, Vec<NodeId>>,            //I nees this for bfs
    package_handler: Repackager,
    paket_ack_manger: HashMap<(NodeId, u64), Vec<Fragment>>,
}


impl Server{

    pub fn new(
        id: NodeId,
        packet_recv: Receiver<Packet>,
        packet_send : HashMap<NodeId, Sender<Packet>>,
        
        server_type: Box< dyn ServerTrait>,
    ) -> Self {
        
        let mut graph = HashMap::new();
        for (key,_) in packet_send.iter() {  //start by filling the graph with the directly connected neighbour
            graph.insert(*key,Vec::new());
        }

       Server {
            id :  id,
            packet_recv: packet_recv,
            packet_send: packet_send,   //directly connected neighbour.  
            server_type: server_type,
            
           
           // extra field
            graph: graph,            //I nees this for bfs
            package_handler: Repackager::new(),
            paket_ack_manger: HashMap::new(),
        }
    }    
    
    /*
Start the flood protocol to fill up the hashmap and create the tree of the graph.
Small remainder the hash map is composed in this way HashMap<NodeId, Vec<NodeId>> , The NodeId 
and the list of it's neighbour 
 */

 
    

    pub fn run(&mut self) {
        loop {
            select_biased! {        
                recv(self.packet_recv) -> packet => {
                    match packet {
                        Ok(packet) => {
                           self.handle_packet(packet);
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
        let source_id = packet.routing_header.hops[0];
        
        
        match path {
            Some(x) => {

                //start creating the new packet
                let mut response = Packet
                {
                    routing_header: x,
                    session_id: packet.session_id,
                    pack_type: packet.pack_type.clone()        //just to fill up the filed it will be changed in a moment
                };


                match packet.pack_type {
                    PacketType::MsgFragment(msg) => {

                        //Send back an ack 
                        response.pack_type = AckType(Ack {          
                            fragment_index: msg.fragment_index,
                        });
                        self.send_valid_packet(packet.routing_header.hop_index as NodeId, response.clone());
                        
                        //Start transforming the fragment in a vector with all the data in it 
                        let result = self.package_handler.process_fragment(packet.session_id, packet.routing_header.hop_index as u64, msg);    //All the request send by the client are sort . I refuse to eleborate request longer than 128
                        
                        
                        
                        match result { 
                            Ok(Some(data)) => {
                                
                                //Reassemble the vector to a string with the original message 
                                let message = Repackager::assemble_string(data);
                                
                                //Process the rewquest
                                let result =self.server_type.process_request(message.unwrap(),source as u32);
                                match result {
                                    Ok(value) => {
                                        
                                        //It's the structure used to control the that the packet send to the client are received correctly
                                        self.paket_ack_manger.insert((source_id, packet.session_id), value.clone());

                                        //It starts sending the first fragment to the client.
                                        response.pack_type = MsgFragment(value.index(0).clone());


                                        self.send_valid_packet(source_id, response);    
                                    }
                                    Err(x) => {print!("Error : {}",x)}
                                }
  
                            }
                            _=> {println!("Error fragment to long - refuse to process")}
                        }
                        //let message = Repackager::reassembled_to_string(result);
                        
                    }
                    PacketType::Nack(msg) => {
                        
                        let result =self.paket_ack_manger.get(&(source_id, packet.session_id));
                        match result {
                            None => {println!("Received an NAck but I can't trace buck the number to any packet {:?}", msg)}
                            Some(ack_value) => {
                                if let Some(pos) = ack_value.iter().position(|f| f.fragment_index == msg.fragment_index) {
                                    
                                    //Send the previous  packet 
                                    response.pack_type = MsgFragment(ack_value[pos].clone());
                                    self.send_valid_packet(source_id, response);
                                } else {
                                    println!("Index of Nack not found.");
                                }
                            }
                        }
                        
                    }
                    PacketType::Ack(msg) => {
                        
                        
                        let result =self.paket_ack_manger.get(&(source_id, packet.session_id));
                        match result {
                            None => {println!("Received an Ack but I can't trace buck the number to any packet {:?}", msg)}
                            Some(ack_value) => {
                                if let Some(pos) = ack_value.iter().position(|f| f.fragment_index == msg.fragment_index+1) {
                                    if pos + 1 < ack_value.len(){
                                        
                                        //Send the next packet 
                                        response.pack_type = MsgFragment(ack_value[pos].clone());
                                        self.send_valid_packet(source_id, response);
                                    } else {
                                        println!("All fragment sent correctly");
                                        self.paket_ack_manger.remove(&(source_id, packet.session_id));
                                    }
                                } else {
                                    println!("Index of ack not found.");
                                }
                            }
                        }
                        
                        
                        //self.send_valid_packet(next_hop, packet);
                    }

                    PacketType::FloodResponse(path) => {
                        NewWork::recive_flood_response(&mut self.graph, path.path_trace);            //It's not the job of the server to propagate the message is not a drone

                    }

                    PacketType::FloodRequest(mut flood_packet) =>
                        {
                            let mut previous_neighbour = 0;
                            if let Some(last) = flood_packet.path_trace.last()
                            {
                                previous_neighbour = last.0;
                                //The Server is note a drone so when he receives a flood request he can send back a flood response with no problem
                                let response = flood_packet.generate_response(packet.session_id);
                                NewWork::recive_flood_response(&mut self.graph, flood_packet.path_trace);

                                self.send_packet(previous_neighbour,response );
                            } else {
                                panic!("Can not find neighbour who send this packet {} ", flood_packet);
                            }
                        }
                }
            }
            None => { println!("No path found for source {}  found !!ERROR!!", source);}



        }
    }

    fn sendflod_request(& self)
    {
        let request = FloodRequest {
            flood_id : rand::random(),
            initiator_id: self.id,
            path_trace: vec![(self.id, NodeType::Server)],
        };
        
        
        //Send flood request to all his neighbour
        println!("Sending flood request: {:?}", request);                               
        for (node,sender) in self.packet_send.iter() {
            println!("Sending flood request to node {:?}",  node);
            
            self.send_valid_packet(*node,Packet{
                routing_header: Default::default(),
                session_id: rand::random(),
                pack_type: FloodRequestType(request.clone()),
            });
        }
    }
    
    
   /* fn send_shortcut(&mut self, packet: Packet) {
        if let Err(e) = self.controller_send.send(ControllerShortcut(packet)) {
            println!("{}", e);
        }
    }*/

  /*  fn send_packet_sent_event(&mut self, packet: Packet) {
        self.controller_send
            .send(PacketSent(packet))
            .expect("Failed to send message to simulation controller");
    }*/

    fn send_packet(& self, dest_id: NodeId, mut packet: Packet) {
        
        let sender = self.packet_send.get(&dest_id);
        
        match NewWork::bfs_shortest_path(&self.graph, self.id, dest_id) {
            Some(path) => { 
                packet.routing_header = path;
                let c =self.packet_send.get(&packet.routing_header.hops[1]);
                match c {
                    None => {}
                    Some(x) => {x.send(packet);}
                }
            },
            None => {print!("Error not found a valid path to follow")}
        }

    }

    fn send_valid_packet(& self, dest_id: NodeId, packet: Packet) {
        self.send_packet(dest_id, packet.clone());
       // self.send_packet_sent_event(packet);
    }


}
