#![allow(warnings)]

mod message;
mod interface;


use crate::interface::interface::*;
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
use wg_2024::packet::PacketType::{Ack as AckType,  MsgFragment};
use wg_2024::packet::{Ack, FloodRequest, FloodResponse, Fragment, NodeType, Packet, PacketType};
use NewWork::bfs_shortest_path;
use std::{sync::{Arc, Mutex}, thread, time::Duration};
use ratatui::style::Color;

pub use message::file_system;
pub struct  Server  
{
    id: NodeId,
    packet_recv: Receiver<Packet>,                  //packet receiver
    packet_send: HashMap<NodeId, Sender<Packet>>,   //directly connected neighbour (drone  for send the packet)  
    server_type: Box<dyn ServerTrait>,              //Server type 
  
    // extra field
    graph: HashMap<NodeId, Vec<NodeId>>,            //I need this for bfs
    package_handler: Repackager,                    // Fragment and reassemble file
    paket_ack_manger: HashMap<(NodeId, u64), Vec<Fragment>>,        // Keep track of the ack 
    message_list: Arc<Mutex<Vec<(String, Color, String, Color)>>>,
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
           message_list: Arc::new(Mutex::new(vec![])),
        }
    }    
    
    /*
Start by sending a flood request to all the neighbour to fill up the graph
 */

 
    

    pub fn run(&mut self) {
        self.sendflod_request();
        start_ui("pollo".to_string(), self.message_list.clone());
        loop {
            select_biased! {        //copied from the drone        
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


    
     */
    
    
    
    fn handle_packet(&mut self, mut packet: Packet) {

        //first I take the source id (it's used for processing ack, send the packet.....)
        let mut source_id :NodeId = 0; //don't worry I am just initializing it
            match packet.routing_header.hops.get(0){    //trying to get the source id 
                None => {       
                        match &packet.pack_type
                    {
                        PacketType::FloodRequest(_) => { add_message(&self.message_list, "Server", "Received FloodRequest", Color::White, Color::White); }
                        PacketType::FloodResponse(_) => { add_message(&self.message_list, "Server", "Received FloodResponse", Color::White, Color::White);}
                        _ => { add_message(&self.message_list, "Server", "I received an packet without headers", Color::White, Color::Yellow);}
                        //the only packets that don't have the routing_header are the FloodRequest and the   FloodResponse
                    }
                }
                Some(x) => {source_id =*x}    
            }
        
        let mut response = packet.clone();  //The response will be modified later . For now, it's just a copy
        
                match packet.pack_type {                    //Process different packet type
                    PacketType::MsgFragment(msg) => {

                        add_message(&self.message_list, "Server", "Recived Fragment", Color::White, Color::White);
                        
                        //Send back an ack 
                        response.pack_type = AckType(Ack { fragment_index: msg.fragment_index, });
                        self.send_valid_packet(source_id as NodeId, response.clone());
                        add_message(&self.message_list, "Server", "Send an Ack back", Color::White, Color::White);
                        
                        
                        //Start transforming the fragment in a vector with all the data in it 
                        let result = self.package_handler.process_fragment(packet.session_id, source_id as u64, msg);    //All the request send by the client are command. I refuse to elaborate request longer than 128
                        
                        
                        match result { 
                            Ok(Some(data)) => {//If we are able to reassemble the data we proceed
                                add_message(&self.message_list, "Server", "I was able to reassemble a message!", Color::White, Color::Blue);
                                
                                //Reassemble the vector to a string with the original message 
                                let message = Repackager::assemble_string(data);
                                add_message(&self.message_list, "Server", &format!("{} {}","Fragmented message:",message.clone().unwrap()), Color::White, Color::White);
        
                               
                                let mut  flag:i32 = 0;  //1 = client not found
                                let msg_work = message.clone().unwrap();        //temp value
                                
                                //Process the request
                                let result =self.server_type.process_request(message.unwrap(),source_id as u32,&mut flag);

                                if flag==0
                                {
                                    /*
                          Here there is an exception if the message start with messageFor?(...)
                          It means that is a message for another user, so I have to change the source id
                          */
                                    if let Some(content) = msg_work.strip_prefix("message_for?(").and_then(|s| s.strip_suffix(")")) {
                                        let parts: Vec<&str> = content.splitn(2, ',').collect();
                                        if parts.len() == 2 {
                                            source_id = parts[0].parse::<NodeId>().unwrap();
                                        }
                                    }
                                }
                 
                                match result {
                                    Ok(value) => {
                                        
                                        //It's the structure used to control the that the packet send to the client are received correctly 
                                        self.paket_ack_manger.insert((source_id, packet.session_id), value.clone());

                                        //It starts sending the first fragment to the client.
                                        response.pack_type = MsgFragment(value.index(0).clone());


                                        add_message(&self.message_list, "Server", &format!("{} {}","Sending response to client source id  : ",source_id), Color::White, Color::White);
                                        self.send_valid_packet(source_id, response);    
                                    }
                                    Err(x) => {
                                        add_message(&self.message_list, "Server", &format!("{} {}","ERRORE : ",x), Color::White, Color::Red); 
                                        }
                                }
  
                            }
                            _=> {
                                add_message(&self.message_list, "Server", "Error I was unable to process the packet. It could be because the fragment to long (I refuse to process)", Color::White, Color::Red); }
                        }
                    }
                    
                    
                    
                    
                    
                    PacketType::Nack(msg) => {


                        add_message(&self.message_list, "Server","I recived a NACK", Color::White, Color::Blue);
                        let mut found =  false;
                        
                        //try to find the packet in the packet ack manager
                        for ((source_id, session_id), vec) in self.paket_ack_manger.iter() {
                            if *session_id == packet.session_id {
                                
                                let mut new_response = response.clone();
                                new_response.pack_type = MsgFragment(vec[msg.fragment_index as usize].clone());
                                self.send_valid_packet(*source_id, new_response);
                                found = true;

                                add_message(&self.message_list, "Server",&format!("{}{}","I sent another time a message to ",source_id), Color::White, Color::Blue);
                                
                            }
                        }
                        
                        if !found {
                            add_message(&self.message_list, "Server", &format!("{}{} {:?}{}{}{}{:?}",source_id, 
                                                                               "Received an NAck but I can't trace back the number to any packet ",msg.clone(), 
                                                                               " session id: ",packet.session_id," current packet handler: ",self.paket_ack_manger), Color::White, Color::Red); 
                        }
                        
                        else
                        {
                            
                        }
                        
          
                        /*
                        let result =self.paket_ack_manger.get(&(source_id, packet.session_id));
                        match result {
                            None => {
                                add_message(&self.message_list, "Server", &format!("{}{} {:?}{}{}{}{:?}",source_id, "Received an NAck but I can't trace back the number to any packet ",msg.clone(), " session id: ",packet.session_id," current packet handler: ",self.paket_ack_manger), Color::White, Color::Red); }
                            Some(ack_value) => {
                                if let Some(pos) = ack_value.iter().position(|f| f.fragment_index == msg.fragment_index) {
                                    
                                    //Send the previous  packet 
                                    //response.pack_type = MsgFragment(ack_value[pos].clone());
                                    add_message(&self.message_list, "Server",&format!("{} {} {}", "I recived a Nack. Send back the packet ", ack_value[pos].clone()," again."), Color::White, Color::Red); 
                                    
                                    //self.send_valid_packet(source_id, response);
                                } else {
                                    add_message(&self.message_list, "Server", "Index of Nack not found.", Color::White, Color::Red); }
                                    
                                }
                            }*/
                        }
                        
                    
                    
                    
                    
                    
                    PacketType::Ack(msg) => {   //Send the next packet 
                        let result =self.paket_ack_manger.get(&(source_id, packet.session_id)); //Get the correct session
                        match result {
                            None => {
                                add_message(&self.message_list, "Server", " Received an Ack but I can't trace back the number to any packet ", Color::White, Color::Red); 
                               
                            }
                            Some(ack_value) => {
                                /*
                                The ack manager is a structure that use for a ky the source_id and the session id
                                it has a vector containing all the packet that need to be sends
                                 */
                                    //I try to get the next packet that is needs to be sent 
                                    if let Some(pos) = ack_value.iter().position(|f| f.fragment_index == msg.fragment_index + 1) {
                                        
                                            //Send the next packet
                                            response.pack_type = MsgFragment(ack_value[pos].clone());
                                        add_message(&self.message_list, "Server", &format!("{} {} {} {} {}","Received a ack from source id: ",source_id," and Session id: ",packet.session_id, " ). Sending next paxet"), Color::White, Color::White);

                                        self.send_valid_packet(source_id, response);
                                    } 
                                    else {  //Check if all the packets are arrived correctly 
                                        if  msg.fragment_index as usize  == ack_value.len()-1
                                        {
                                            add_message(&self.message_list, "Server", &format!("{} {} {} {} {}","All ack received - Removing session- (Source id: ",source_id,", Session id: ",packet.session_id, " )."), Color::White, Color::White);
                                            self.paket_ack_manger.remove(&(source_id, packet.session_id));
                                        }
                                        else {
                                            add_message(&self.message_list, "Server", "Index of Nack not found.", Color::White, Color::Red); 
                                        }
                                    }
                                
                    
                            }
                        }
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
                                flood_packet.path_trace.push((self.id,NodeType::Server)); //TODO fixed here
                                //The Server is note a drone so when he receives a flood request he can send back a flood response with no problem
                                let new_hops = flood_packet.path_trace
                                    .iter()
                                    .cloned()
                                    .map(|(id, _)| id)
                                    .rev()
                                    .collect();
                                let srh = SourceRoutingHeader::with_first_hop(new_hops);
                                let flood_resp = FloodResponse{
                                    flood_id: flood_packet.flood_id,
                                    path_trace: flood_packet.path_trace.clone()
                                };
                                let response = Packet::new_flood_response(srh,packet.session_id,flood_resp);
                                NewWork::recive_flood_response(&mut self.graph, flood_packet.path_trace);

                                //for (id,sendr) in &self.packet_send{    //send flood response to all his neibourgh
                                    self.packet_send.get(&previous_neighbour).expect("Error while getting neighbor").send(response.clone()).expect("Server: Error while sending FloodResponse"); //TODO do match case
                                //}
                            } else {
                                add_message(&self.message_list, "Server", "Can not find neighbour who send this packet.", Color::White, Color::Red);

                            }
                        }
                }
        


        
    }

    fn sendflod_request(& self)
    {

        
        
        //Send flood request to all his neighbour
        for (node,sender) in self.packet_send.iter() {
            add_message(&self.message_list, "Server", "Sending flood request to node ", Color::White, Color::Blue);
            

            let request = FloodRequest {
                flood_id : rand::random(),
                initiator_id: self.id,
                path_trace: vec![(self.id, NodeType::Server)],
            };
            
            let p = Packet
            {
                routing_header: Default::default(),
                session_id: 0,
                pack_type: PacketType::FloodRequest(request ),
            };
            
             //send flood request to all his neibourgh
            sender.send(p.clone());
            
        }
    }
    
    
   /* fn send_shortcut(&mut self, packet: Packet) {
        if let Err(e) = self.controller_send.send(ControllerShortcut(packet)) {
        }
    }*/

  /*  fn send_packet_sent_event(&mut self, packet: Packet) {
        self.controller_send
            .send(PacketSent(packet))
            .expect("Failed to send message to simulation controller");
    }*/

    fn send_packet(& self, dest_id: NodeId, mut packet: Packet) {
        
       // let sender = self.packet_send.get(&dest_id);
        
        match NewWork::bfs_shortest_path(&self.graph, self.id, dest_id) {       
            Some(path) => { 
                packet.routing_header = path;
                let c =self.packet_send.get(&packet.routing_header.hops[1]);    //take the first node to which you need to send the messages
                match c {
                    None => {}
                    Some(x) => {x.send(packet);}
                }
            },
            None => {
                add_message(&self.message_list, "Server", "Error not found a valid path to follow", Color::White, Color::Red);
            
            }
            
        }

    }

    fn send_valid_packet(& self, dest_id: NodeId, packet: Packet) {
        self.send_packet(dest_id, packet.clone());
       // self.send_packet_sent_event(packet);
    }


}
