#![allow(warnings)]

use std::collections::{HashMap, HashSet};
use std::fs;
use crossbeam_channel::unbounded;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Packet, PacketType};
use crate::Client;
use crate::fragment_reassembler::FragmentReassembler;

//Communication part related to the Client
impl Client {
    // Handle user input received and send command to a dest_id (e.g. a server)
    pub fn handle_command(&mut self, command: &str, dest_id: NodeId) -> String{
        // Check for serverType in order to send only the correct set of messages
        match &self.server.1{
            server_type if server_type.eq("CommunicationServer") =>{
                match command{
                    cmd if cmd == "server_type?" =>{
                        self.send_message(dest_id,cmd);
                        "OK".to_string()
                    }
                    cmd if cmd == "client_list?" =>{
                        self.send_message(dest_id,cmd);
                        "OK".to_string()
                    }
                    cmd if cmd.starts_with("message_for?(") =>{
                        match Self::get_values(cmd){
                            Some(values) =>{
                                if self.other_client_ids.contains(&values.0){
                                    self.send_message(values.0,values.1);
                                    "OK".to_string()
                                }
                                else{
                                    "Error: invalid dest_id".to_string()
                                }
                            }
                            None =>{
                                "Error: command not formatted correctly".to_string()
                            }
                        }
                    }
                    _=> "Not a valid communication command".to_string()
                }
            }
            // In the other cases more messages can be sent
            _=>{
                match command{
                    cmd if cmd == "files_list?" =>{
                        self.send_message(dest_id,cmd);
                        "OK".to_string()
                    }
                    cmd if cmd.starts_with("file?(") && cmd.ends_with(")")  =>{
                        if let Some(name) = cmd.strip_prefix("file?(").and_then(|s|s.strip_suffix(")")){
                            if self.files_names.contains(&name.parse::<String>().ok().unwrap()){ //TODO fix
                                self.send_message(dest_id,cmd);
                                "OK".to_string()
                            }
                            else{
                                "Error: invalid file_id".to_string()
                            }
                        }
                        else{
                            "Error: command not formatted correctly".to_string()
                        }
                    }
                    cmd if cmd.starts_with("media?(") && cmd.ends_with(")") =>{
                        if let Some(id) = cmd.strip_prefix("media?(").and_then(|s|s.strip_suffix(")")){
                            if id.is_empty(){
                                "Error: invalid media_id".to_string()
                            }
                            else{
                                self.send_message(dest_id,cmd);
                                "OK".to_string()
                            }
                        }
                        else{
                            "Error: command not formatted correctly".to_string()
                        }
                    }
                    cmd if cmd == "registration_to_chat" =>{
                        self.send_message(dest_id,cmd);
                        "OK".to_string()
                    }
                    cmd if cmd == "client_list?" =>{
                        self.send_message(dest_id,cmd);
                        "OK".to_string()
                    }
                    _ =>{"Not a valid command".to_string()}
                }
            }
        }
    }
    // Send message (fragmented data) to a dest_id using bfs to compute the path
    pub fn send_message(&mut self, dest_id: NodeId, data: &str) {
        let fragments = FragmentReassembler::generate_fragments(data).expect("Error while creating fragments");
        let session_id =  Self::generate_session_id();
        for fragment in fragments {
            if let Some(sender) = self.sender_channels.get(&dest_id) {
                let packet_sent = Packet {
                    routing_header: SourceRoutingHeader::with_first_hop(Self::bfs_compute_path(&self.network,self.node_id,dest_id).unwrap()),
                    pack_type: PacketType::MsgFragment(fragment.clone()),
                    session_id
                };
                sender.send(packet_sent).expect("Failed to send message");
                // After sending a fragment wait until an Ack returns back. If Nack received, proceed to send again the fragment with updated network and new route
                'internal: loop {
                    match self.receiver_channel.recv(){
                        Ok(packet) =>{
                            match packet.pack_type{
                                PacketType::Ack(_) =>{
                                    // In case I receive an Ack with same session_id message arrived correctly: restored normal course of the program
                                    if packet.session_id == session_id{break 'internal}
                                },
                                PacketType::Nack(_) =>{
                                    if packet.session_id == session_id{
                                        // In case I receive a Nack with same session_id I need to send again the message.
                                        //self.discover_network(); // I hope it works with the new route
                                        let packet = Packet {
                                            routing_header: SourceRoutingHeader::with_first_hop(Self::bfs_compute_path(&self.network,self.node_id,dest_id).unwrap()),
                                            pack_type: PacketType::MsgFragment(fragment.clone()),
                                            session_id
                                        };
                                        sender.send(packet.clone()).unwrap();
                                    }
                                }
                                _=> ()
                            }
                        }
                        Err(e) => panic!("{e}")
                    }
                }
            }
        }
    }
    // Handle a received message (e.g. from a server) with eventual parameters
    pub fn handle_msg(&mut self, received_msg: String, session_id: u64, src_id: NodeId) -> String{
        match received_msg{
            msg if msg.starts_with("server_type!(") && msg.ends_with(")") =>{
                self.server.0 = src_id;
                match msg.strip_prefix("server_type!(").and_then(|s|s.strip_suffix(")")){
                    Some(serverType) =>{
                        self.server.1 = serverType.to_string();
                        "OK".to_string()
                    }
                    None =>{"Not valid server type".to_string()}
                }
            }
            msg if msg.starts_with("files_list!([") && msg.ends_with("])") =>{
                match Client::get_file_vec(msg){
                    Some(val) =>{
                        for e in val{
                            self.files_names.push(e);
                        }
                        "OK".to_string()
                    }
                    None => "There are no file_IDs in the message".to_string()
                }
            }
            msg if msg.starts_with("file!(") && msg.ends_with(")") =>{
                match Client::get_file_values(msg){
                    Some(res) =>{
                        if !res.is_empty(){
                            let hops = Self::bfs_compute_path(&self.network, self.node_id, src_id).unwrap();
                            let first_hop = hops[1].clone();
                            let new_pack = Packet::new_ack(
                                SourceRoutingHeader::with_first_hop(hops), session_id, 0);
                            self.received_files.push(res.clone());
                            self.sender_channels.get(&first_hop).expect("Didn't find neighbor").send(new_pack).expect("Error while sending packet");
                            res.to_string()
                        }
                        else{
                            "Error: no file".to_string()
                        }
                    }
                    None => "Error while extracting file from message".to_string()
                }
            }
            msg if msg.starts_with("media!(") && msg.ends_with(")") =>{
                match msg.strip_prefix("media!(").and_then(|s|s.strip_suffix(")")){
                    Some(clean_data) =>{
                        if !clean_data.is_empty(){
                            let hops = Self::bfs_compute_path(&self.network,self.node_id,src_id).unwrap();
                            let first_hop = hops[1].clone();
                            let new_pack = Packet::new_ack(
                                SourceRoutingHeader::with_first_hop(hops),session_id,0);
                            self.sender_channels.get(&first_hop).expect("Didn't find neighbor").send(new_pack).expect("Error while sending packet");
                            clean_data.to_string()
                        }
                        else{
                            "Error: no media".to_string()
                        }
                    }
                    None => "No media in the message".to_string()
                }
            }
            msg if msg == "error_requested_not_found!(Problem opening the file)"=>{
                println!("{}", msg);
                msg.to_string()
            }
            msg if msg == "error_requested_not_found!(File not found)"=>{
                println!("{}", msg);
                msg.to_string()
            }
            msg if msg == "error_requested_not_found!"=>{
                println!("{}", msg);
                msg.to_string()
            }
            msg if msg == "error_unsupported_request!"=>{
                println!("{}", msg);
                "error_unsupported_request!".to_string()
            }
            msg if msg.starts_with("client_list!([") && msg.ends_with("])") =>{
                match Client::get_ids(msg){
                    Some(val) =>{
                        for e in val{
                            self.other_client_ids.push(e);
                        }
                        "OK".to_string()
                    }
                    None => "There are no other clients in the network right now".to_string()
                }
            }
            msg if msg.starts_with("message_from!(") && msg.ends_with(")") =>{
                match Self::get_values(&msg){
                    Some(values) =>{
                        let src_id = values.0;
                        let hops = Self::bfs_compute_path(&self.network,self.node_id,src_id).unwrap();
                        let neighbor = hops[1];
                        let new_pack = Packet::new_ack(
                            SourceRoutingHeader::with_first_hop(hops),session_id,0);
                        self.sender_channels.get(&neighbor).expect("Didn't find neighbor").send(new_pack).expect("Error while sending packet");
                        "OK".to_string()
                    }
                    None => "Failed to get message content".to_string()
                }
            }
            _=> "Error".to_string()
        }
    }
    // Helper functions
    pub fn get_values(cmd: &str) -> Option<(NodeId,&str)>{
        if cmd.starts_with("message_for?(") && cmd.ends_with(")"){
            if let Some(raw_data) = cmd.strip_prefix("message_for?(").and_then(|s|s.strip_suffix(")")) {
                let values = raw_data.split_once(",").expect("Failed to get values");
                Some((values.0.parse::<NodeId>().unwrap(), values.1))
            }
            else{
                None
            }
        }
        else if cmd.starts_with("message_from!(") && cmd.ends_with(")"){
            if let Some(raw_data) = cmd.strip_prefix("message_from!(").and_then(|s|s.strip_suffix(")")) {
                let values = raw_data.split_once(",").expect("Failed to get values");
                Some((values.0.parse::<NodeId>().unwrap(), values.1))
            }
            else{
                None
            }
        }
        else{
            None
        }
    }
    pub fn get_ids(msg: String) -> Option<Vec<NodeId>>{
        if let Some(raw_data) = msg.strip_prefix("client_list!([").and_then(|s|s.strip_suffix("])")){
            if !raw_data.is_empty(){
                Some(raw_data.split(",").filter_map(|s|s.trim().parse::<NodeId>().ok()).collect())
            }
            else{
                None
            }
        }
        else{
           None
        }
    }
    pub fn get_file_values(cmd: String) -> Option<String>{
        if let Some(raw_data) = cmd.strip_prefix("file!(").and_then(|s| s.strip_suffix(")")){
            let values = raw_data.split_once(",").expect("Failed to get values");
            if !values.1.is_empty(){
                Some(values.1.to_string())
            }
            else{
                None
            }
        } else {
            None
        }
    }
    pub fn get_file_vec(cmd: String) -> Option<Vec<String>>{
        if let Some(raw_data) = cmd.strip_prefix("files_list!([").and_then(|s| s.strip_suffix("])")){
            if !raw_data.is_empty(){
                Some(raw_data.split(",").filter_map(|s|s.trim().parse::<String>().ok()).collect())
            }
            else{
                None
            }
        } else {
            None
        }
    }
}
#[cfg(test)]
mod test{
    use super::*;
    #[test]
    fn test_get_ids(){
        let valid_command = "client_list!([1,2,3,4])".to_string();
        let invalid_command = "client_list!([])".to_string();
        assert!([1,2,3,4].to_vec().eq(&Client::get_ids(valid_command).unwrap()));
        assert!(Client::get_ids(invalid_command).is_none());
    }
    #[test]
    fn test_get_file_values(){
        let valid_command = "file!(4,file.txt)".to_string();
        let invalid_command = "file!(4,)".to_string();
        assert!("file.txt".eq(&Client::get_file_values(valid_command).unwrap()));
        assert!(&Client::get_file_values(invalid_command).is_none());
    }
    #[test]
    fn test_get_file_vec(){
        let valid_command = "files_list!([file1,file2,file3])".to_string();
        let invalid_command = "files_list!([])".to_string();
        assert!(["file1".to_string(),"file2".to_string(),"file3".to_string()].to_vec().eq(&Client::get_file_vec(valid_command).unwrap()));
        assert!(&Client::get_file_vec(invalid_command).is_none());
    }
    // Test of returned values
    #[test]
    fn test_handle_msg_received(){
        // Initialize dummy client
        let (snd,rcv) = unbounded::<Packet>();
        let mut cl = Client::new(1, HashSet::new(), HashMap::new(), rcv);
        cl.sender_channels.insert(2,snd);
        cl.network.insert(1,vec![2]);
        cl.other_client_ids.push(2);

        // Tests
        let test_msg1 = "server_type!(CommunicationServer)".to_string() ;
        let test_msg2 = "files_list!([file1.txt,file2.txt])".to_string() ;
        assert_eq!(cl.handle_msg(test_msg1,3,2),"OK");
        assert_eq!(cl.handle_msg(test_msg2,3,2),"OK");

        let file_txt = fs::read("src/test/file1").unwrap();
        let file_txt2 = FragmentReassembler::assemble_string_file(file_txt,&mut cl.received_files).unwrap();
        let mut msg= String::from("file!(6,");
        msg.push_str(&file_txt2);
        msg.push_str(")");
        assert_eq!(cl.handle_msg(msg,3,2),file_txt2);

        let file_media = fs::read("src/test/testMedia.mp3").unwrap();
        let file_media2 = FragmentReassembler::assemble_string_file(file_media,&mut cl.received_files).unwrap();
        let mut msg= String::from("media!(");
        msg.push_str(&file_media2);
        msg.push_str(")");
        assert_eq!(cl.handle_msg(msg,3,2),file_media2);

        let test_msg1 = "client_list!([1,2,3,4])".to_string();
        assert_eq!(cl.handle_msg(test_msg1,3,2),"OK");

        let test_msg2 = "message_from!(2,file.txt)".to_string();
        assert_eq!(cl.handle_msg(test_msg2,3,2),"OK");

        let test_msg4 = "error_requested_not_found!(File not found)".to_string() ;
        assert_eq!(cl.handle_msg(test_msg4.clone(),3,2),test_msg4);

        let test_msg5 = "error_requested_not_found!(Problem opening the file)".to_string() ;
        assert_eq!(cl.handle_msg(test_msg5.clone(),3,2),test_msg5);

        let test_msg6 = "error_requested_not_found!".to_string() ;
        assert_eq!(cl.handle_msg(test_msg6.clone(),3,2),test_msg6);

        let test_msg7 = "error_unsupported_request!".to_string() ;
        assert_eq!(cl.handle_msg(test_msg7.clone(),3,2),test_msg7);

        let test_msg8 = "test_error".to_string();
        assert_eq!(cl.handle_msg(test_msg8,3,2),"Error");
    }
    /*
    #[test]
    fn test_handle_user_commands(){}
    
     */ //TODO woip
}


