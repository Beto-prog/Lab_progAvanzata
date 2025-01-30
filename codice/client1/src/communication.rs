#![allow(warnings)]
use std::fs;
use std::path::Path;
use wg_2024::network::NodeId;
use crate::Client;

//Communication part related to the Client
impl Client {
    // Create a new FileSystem instance
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
    // Send message with a specific command to a dest_id
    pub fn send_request(&mut self, command: &str, dest_id: NodeId) -> String{
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
                    if id.is_empty(){
                        "Error: invalid file_id".to_string()
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
            cmd if cmd == "server_type?" =>{
                self.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd == "registration_to_chat" =>{
                self.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd == "self_list?" =>{
                self.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd.starts_with("message_for?(") =>{
                match Self::get_values(cmd){
                    Some(values) =>{
                        self.send_message(values.0,values.1);
                        "OK".to_string()
                    }
                    None =>{
                        "Error: command not formatted correctly".to_string()
                    }
                }
            }
            _ =>{"Not a valid command".to_string()}
        }
    }
    // Helper functions
    pub fn get_values(cmd: &str) -> Option<(NodeId,&str)>{
       if let Some(raw_data) = cmd.strip_prefix("message_for?(").and_then(|s|s.strip_suffix(")")) {
           let values = raw_data.split_once(",").expect("Failed to get values");
           Some((values.0.parse::<NodeId>().unwrap(), values.1))
       }else{
           None
       }
    }
}