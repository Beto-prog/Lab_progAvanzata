#![allow(warnings)]
use std::fs;
use std::path::Path;
use wg_2024::network::NodeId;
use crate::Client;

// Struct with a path field where received files will be stored and a Client field that handles the rest
pub struct FileSystem {
    path: String,
    client: Client ,
}


impl FileSystem {
    // Create a new FileSystem instance
    pub fn new(path: &str, client: Client) -> Self {
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
        FileSystem {path: path.to_string(), client }
    }
    // Send message with a specific command to a dest_id
    pub fn send_request(mut client:Client, command: &str, dest_id: NodeId) -> String{
        match command{
            cmd if cmd == "server_type?" =>{
                client.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd == "files_list?" =>{
                client.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd.starts_with("file?(") && cmd.ends_with(")")  =>{
                if let Some(id) = cmd.strip_prefix("file?(").and_then(|s|s.strip_suffix(")")){
                    if id.is_empty(){
                        "Error: invalid file_id".to_string()
                    }
                    else{
                        client.send_message(dest_id,cmd);
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
                        client.send_message(dest_id,cmd);
                        "OK".to_string()
                    }
                }
                else{
                    "Error: command not formatted correctly".to_string()
                }
            }
            cmd if cmd == "server_type?" =>{
                client.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd == "registration_to_chat" =>{
                client.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd == "client_list?" =>{
                client.send_message(dest_id,cmd);
                "OK".to_string()
            }
            cmd if cmd.starts_with("message_for?(") =>{
                match Self::get_values(cmd){
                    Some(values) =>{
                        client.send_message(values.0,values.1);
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