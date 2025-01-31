#![allow(warnings)]
use std::fs;
use std::path::Path;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Packet, PacketType};
use crate::Client;
use crate::fragment_reassembler::FragmentReassembler;

//Communication part related to the Client
impl Client {
    pub fn new_path(path: &str) -> String{
        let path_dir = Path::new(path);

        // Check if the directory exists
        if path_dir.exists() {
            if !path_dir.is_dir() {
                panic!("Error: '{}' exists but is not a directory.", path);
            }
        } else {
            // Create the directory if it doesn't exist
            match fs::create_dir_all(path) {
                Ok(_) => println!("Directory '{}' created.", path),
                Err(err) => panic!("Error: Could not create directory '{}': {}", path, err),
            }
        }
        path.to_string()
    }
    // Handle user input received and send command to a dest_id (e.g. a server)
    pub fn handle_command(&mut self, command: &str, dest_id: NodeId) -> String{
        match command{
            cmd if cmd == "server_type?" =>{
                self.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd == "files_list?" =>{
                self.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd.starts_with("file?(") && cmd.ends_with(")")  =>{
                if let Some(id) = cmd.strip_prefix("file?(").and_then(|s|s.strip_suffix(")")){
                    if self.files_ids.contains(&id.parse::<NodeId>().ok().unwrap()){
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
            // Before sending check if dest_ id (e.g. other client) exists
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
            _ =>{"Not a valid command".to_string()}
        }
    }
    // Send message (fragmented data) to a dest_id using bfs
    pub fn send_message(&mut self, dest_id: NodeId, data: &str) { //TODO check for high level message sent
        let fragments = FragmentReassembler::generate_fragments(data).expect("Error while creating fragments");
        let session_id =  Self::generate_session_id();
        for fragment in fragments {
            if let Some(sender) = self.sender_channels.get(&dest_id) {
                let packet_sent = Packet {
                    routing_header: SourceRoutingHeader::with_first_hop(Self::bfs_compute_path(&self.network,self.node_id,dest_id).unwrap()),
                    pack_type: PacketType::MsgFragment(fragment),
                    session_id
                };
                sender.send(packet_sent).unwrap();
                // After sending a fragment wait until an Ack returns back. If Nack received, proceed to send again the fragment with updated network and new route
                'internal: loop {
                    match self.receiver_channel.recv(){
                        Ok(packet) =>{
                            match packet.pack_type{
                                PacketType::Ack(_) => break 'internal,
                                PacketType::Nack(_) =>{
                                    //self.discover_network();
                                    let packet = Packet {
                                        routing_header: SourceRoutingHeader::with_first_hop(Self::bfs_compute_path(&self.network,self.node_id,dest_id).unwrap()),
                                        pack_type: packet.pack_type,
                                        session_id
                                    };
                                    sender.send(packet.clone()).unwrap();
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
    pub fn handle_msg(&mut self, received_msg: String){
        match received_msg{
            msg if msg.starts_with("server_type!(") && msg.ends_with(")") =>{
                //TODO
            }
            msg if msg.starts_with("client_list!(") && msg.ends_with(")") =>{
                match Client::get_ids(msg){
                    Some(val) =>{
                        for e in val{
                            self.other_client_ids.push(e);
                        }
                    }
                    None =>{println!("There are no other clients in the network right now")}
                }
            }
            msg if msg.starts_with("files_list!(") && msg.ends_with(")") =>{
                match Client::get_ids(msg){
                    Some(val) =>{
                        for e in val{
                            self.files_ids.push(e);
                        }
                    }
                    None =>{println!("There are no file_IDs in the message")}
                }
            }
            msg if msg.starts_with("file!(") && msg.ends_with(")") =>{
                match Client::get_file_values(msg){
                    Some(res) =>{
                        if let Err(e) = fs::write(self.path.as_mut(), res) {
                            format!("Error while writing file: {}", e);
                        }
                    }
                    None => println!("Error while extracting file from message")
                }
            }
            _=>()
        }
    }
    // Helper functions
    pub fn get_values(cmd: &str) -> Option<(NodeId,&str)>{
       if let Some(raw_data) = cmd.strip_prefix("message_for?(").and_then(|s|s.strip_suffix(")")) {
           let values = raw_data.split_once(",").expect("Failed to get values");
           Some((values.0.parse::<NodeId>().unwrap(), values.1))
       }
       else{
           None
       }
    }
    pub fn get_ids(msg: String) -> Option<Vec<NodeId>>{
        if let Some(raw_data) = msg.strip_prefix("client_list!(").and_then(|s|s.strip_suffix(")")){
            Some(raw_data.split(",").filter_map(|s|s.trim().parse::<NodeId>().ok()).collect())
        }
        else{
           None
        }
    }
    pub fn get_file_values(cmd: String) -> Option<String>{
        if let Some(raw_data) = cmd.strip_prefix("file!(").and_then(|s| s.strip_suffix(")")){
            let values = raw_data.split_once(",").expect("Failed to get values");
            Some(values.1.to_string())
        } else {
            None
        }
    }
}
