#![allow(warnings)]
use std::fs;
use std::path::Path;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Packet, PacketType};
use crate::Client;
use crate::fragment_reassembler::FragmentReassembler;
//TODO differentiate message to send based on the server type. Check Alberto's GithHub for it

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
                sender.send(packet_sent).expect("Failed to send message");
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
    pub fn handle_msg(&mut self, received_msg: String, session_id: u64, src_id: u8){
        match received_msg{
            msg if msg.starts_with("server_type!(") && msg.ends_with(")") =>{
                self.server.0 = src_id; //TODO idk if this can be done/it's correct. Check if some more fields in Client are necessary based on serverType
                match msg.strip_prefix("server_type!(").and_then(|s|s.strip_suffix(")")){
                    Some(serverType) =>{
                        self.server.1 = serverType.to_string();
                    }
                    None =>{println!("Not valid server type")}
                }
            }
            msg if msg.starts_with("files_list!([") && msg.ends_with("])") =>{
                match Client::get_file_vec(msg){
                    Some(val) =>{
                        for e in val{
                            self.files_names.push(e);
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
            msg if msg.starts_with("media!(") && msg.ends_with(")") =>{
                match msg.strip_prefix("media!(").and_then(|s|s.strip_suffix(")")){
                    Some(clean_data) =>{
                        if !clean_data.is_empty(){
                            let hops = Self::bfs_compute_path(&self.network,self.node_id,src_id);
                            let new_pack = Packet::new_ack(
                                SourceRoutingHeader::with_first_hop(hops[0]),session_id,0);
                            self.sender_channels.get(&hops[1]).expect("Didn't find neighbor").send(new_pack).expect("Error while sending packet");
                        }
                        if let Err(e) = fs::write(self.path.as_mut(), clean_data) {
                            format!("Error while writing file: {}", e);
                        }
                    }
                    None =>{println!("No media in the message")}
                }
            }
            msg if msg == "error_requested_not_found!"=>{
                println!("error_requested_not_found!");
                //TODO check
            }
            msg if msg == "error_unsupported_request!"=>{
                println!("error_unsupported_request!");
                //TODO check
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
            msg if msg.starts_with("message_from!(") && msg.ends_with(")") =>{
                match msg.strip_prefix("message_from!").and_then(|s|s.strip_suffix(")")){
                    Some(raw_data) =>{
                        match raw_data.split_once(","){
                            Some(values) =>{
                                if !values.0.is_empty(){
                                    let src_id = values.0.parse::<NodeId>().ok().unwrap();
                                    let hops = Self::bfs_compute_path(&self.network,self.node_id,src_id).unwrap();
                                    let neighbor = hops[1];
                                    let new_pack = Packet::new_ack(
                                        SourceRoutingHeader::with_first_hop(hops),session_id,0);
                                    self.sender_channels.get(&neighbor).expect("Didn't find neighbor").send(new_pack).expect("Error while sending packet");
                                }
                                //TODO
                            }
                            None =>{
                                println!("Failed to get message content");
                            }
                        }
                    }
                    None =>{println!("Message is empty")}
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
    pub fn get_file_vec(cmd: String) -> Option<Vec<String>>{
        if let Some(raw_data) = cmd.strip_prefix("file!(").and_then(|s| s.strip_suffix(")")){
             Some(raw_data.split(",").filter_map(|s|s.trim().parse::<String>().ok()).collect())
        } else {
            None
        }
    }
}
