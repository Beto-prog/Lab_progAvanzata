#![allow(warnings)]

/*
This module gives the essential function that allow the server and the client to interact with the network
The principal function are :
    bfs_shortest_path (Tree graph , start_ID , goal_ID)-> gives back the shortest path from start to goal
    remove_neighbor (Tree graph , node_ID, neighbour_ID ) -> remove a neighbour from a node




Guide for implementing the servers and the clients

    First of all you have to create the network graph so you send a FloodRequest to each node DIRECTLY  conneted to the server/client
    here is an example on how the function should be


*/
use crate::message::file_system::{ChatServer, FileSystem};

pub mod net_work {
    use std::collections::{HashMap, VecDeque};
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{Fragment, NodeType};

    //It gives back the shortest path possible  BFS  complexity O(V+E) , shutout to Montresor.
    pub fn bfs_shortest_path(
        graph: &HashMap<NodeId, Vec<NodeId>>,
        start: NodeId,
        goal: NodeId,
    ) -> Option<SourceRoutingHeader> {
        let mut queue = VecDeque::new();
        let mut visited = HashMap::new();
        let mut parent = HashMap::new();

        queue.push_back(start);
        visited.insert(start, true);

        while let Some(current) = queue.pop_front() {
            if current == goal {
                // Reconstruct of the path
                let mut path = vec![goal];
                let mut node = goal;
                while let Some(&p) = parent.get(&node) {
                    path.push(p);
                    node = p;
                }
                path.reverse();
                return Some(SourceRoutingHeader {
                    hop_index: 0,
                    hops: path,
                });
            }

            if let Some(neighbors) = graph.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains_key(&neighbor) {
                        queue.push_back(neighbor);
                        visited.insert(neighbor, true);
                        parent.insert(neighbor, current);
                    }
                }
            }
        }

        None // No path found !BIG PROBLEM!
    }


    // Remove a neighbour from a node in case of a crash
    pub fn remove_neighbor(
        graph: &mut HashMap<NodeId, Vec<NodeId>>,
        node: NodeId,
        neighbor: NodeId,
    ) {         // to DO you have to remove the neighbour form the list of neighbour of the other node 
        if let Some(neighbors) = graph.get_mut(&node) {
            neighbors.retain(|&n| n != neighbor);
            
            todo!("to DO you have to remove the neighbour form the list of neighbour of the other node ")
        }
    }


    //this function start creating the graph with the flood response that it receive
    pub fn recive_flood_response(graph: &mut HashMap<NodeId, Vec<NodeId>>, lead: Vec<(NodeId, NodeType)>) {
        let path: Vec<NodeId> = lead
            .iter()
            .map(|(node_id, _)| *node_id)
            .collect();


        let mut prec: NodeId = 0;

        for (numb, &i) in path.iter().enumerate() {
            // Add the current node to the graph if it doesn't exist
            graph.entry(i).or_insert_with(Vec::new);

            // Skip the first iteration (no previous node exists)
            if numb != 0 {
                /* Add prec as a neighbor of i because the neighbours have a mirror relationship */
                if let Some(neighbors) = graph.get_mut(&i) {
                    if !neighbors.contains(&prec) {
                        neighbors.push(prec);
                    }
                }

                // Add `i` as a neighbor of `prec`
                graph.entry(prec).or_insert_with(Vec::new);
                if let Some(neighbors) = graph.get_mut(&prec) {
                    if !neighbors.contains(&i) {
                        neighbors.push(i);
                    }
                }
            }

            // Update `prec` for the next iteration
            prec = i;
        }
    }
}
pub mod packaging
{
    use std::collections::{HashMap, VecDeque};
    use std::fs;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{Fragment, NodeType};

pub struct Repackager {
        buffers: HashMap<(u64, u64), Vec<u8>>, // Maps (session_id, src_id) to a buffer
        processed_packet: HashMap<(u64, u64),u8>,
    }

    impl Repackager {
        pub fn new() -> Self {
            Repackager {
                buffers: HashMap::new(),
                processed_packet: HashMap::new(),
            }
        }


        /*

To reassemble fragments into a single packet, a client or server uses the fragment header as follows:

    1. The client or server receives a fragment.
    2. It first checks the (session_id, src_id) tuple in the header.    
    3. If it has not received a fragment with the same (session_id, src_id) tuple, then it creates a vector (Vec<u8> with capacity of total_n_fragments * 128) where to copy the data of the fragments.
    4. It would then copy length elements of the data array at the correct offset in the vector.


*/
        
        
        
        pub fn process_fragment(&mut self, session_id: u64, src_id: u64, fragment: Fragment) -> Result<Option<Vec<u8>>, u8> {
            let key = (session_id, src_id);

            // Check if buffer for this session_id and src_id exists, if not, create it
            let buffer = self.buffers.entry(key).or_insert_with(|| {
                Vec::with_capacity((fragment.total_n_fragments * 128) as usize)
            });

            // Ensure buffer is large enough, I am pretty sure it useless but online is but a great enfasis on this to prevent error . Id
            // Controllo della dimensione del buffer
            if buffer.len() < (fragment.total_n_fragments * 128) as usize {
                println!(
                    "Resizing buffer: current = {}, required = {}",
                    buffer.len(),
                    fragment.total_n_fragments * 128
                );
                buffer.resize((fragment.total_n_fragments * 128) as usize, 0);
            }

            // Copy fragment data into the buffer at the correct offset. Each fragment is of size 128 except the last one 
            let start = (fragment.fragment_index * 128) as usize;
            let end = start + fragment.length as usize;

            
                // pretty useless this one too . But you never know
            if end > buffer.len() {
                return Err(1); // Indicate error for invalid fragment
            }
    
            //copy the whole fragment in the buffer 
            buffer[start..end].copy_from_slice(&fragment.data[..fragment.length as usize]);

            // Increment the processed packet counter
            *self.processed_packet.entry(key).or_insert(0) += 1;

            // Check if all fragments have been received
            if self.processed_packet[&key] == fragment.total_n_fragments as u8 {
                // All fragments received, return the reassembled data
                let complete_data = self.buffers.remove(&key).unwrap_or_default();
                self.processed_packet.remove(&key);
                Ok(Some(complete_data)) // Message reassembled successfully
            } else {
                Ok(None) // Not all fragments received yet
            }
        }

        pub fn create_fragments(initial_string: &str, file_path: Option<&str>) -> Result<Vec<Fragment>, String> {
            // Convert the initial string to bytes
            let mut message_data = initial_string.as_bytes().to_vec();

            // If a file path is provided, read the file and append its content 
            if let Some(path) = file_path {
                let file_content = fs::read(path).map_err(|e| format!("Error reading file: {}", e))?;
                message_data.extend(file_content);
                //message_data.extend(")".to_string().as_bytes().to_vec());
                
            }
        
            
            let total_size = message_data.len();
            let total_n_fragments = ((total_size + 127) / 128) as u64; // Calculate the total number of fragments
            let mut fragments = Vec::new();

            for i in 0..total_n_fragments {
                let start = (i as usize) * 128;
                let end = ((i as usize) + 1) * 128;
                let slice = &message_data[start..std::cmp::min(end, total_size)];

                let mut data = [0u8; 128];
                data[..slice.len()].copy_from_slice(slice);

                fragments.push(Fragment {
                    fragment_index: i,
                    total_n_fragments  ,
                    length: slice.len() as u8,
                    data,
                });
            }

            Ok(fragments)
        }



        pub fn assemble_string (data: Vec<u8>) -> Result<String, String> {      //use this when you know there are no file --- ONLY FOR THE SERVER 
            // Convert the vector of bytes to a string
            match String::from_utf8(data) {
                Ok(mut string) => {
                    // Remove trailing null characters
                    string = string.trim_end_matches('\0').to_string();
                    Ok(string)
                }
                Err(e) => Err(format!("Failed to convert data to string: {}", e)),
            }
        }


        pub fn assemble_string_file(data: Vec<u8>, output_path: &str) -> Result<String, String> {
            // Remove null charachter
            let clean_data = data.into_iter().take_while(|&byte| byte != 0).collect::<Vec<_>>();

            // Serch the posizion of tje first separator
            let separator_pos = clean_data.iter().position(|&b| b == b'(' );

            if let Some(pos) = separator_pos {
                // Take the initial string
                let initial_string = match String::from_utf8(clean_data[..pos].to_vec()) {
                    Ok(s) => s,
                    Err(e) => return Err(format!("Errore nella conversione della stringa iniziale: {}", e)),
                };

                // Extract file content 
                let file_data = &clean_data[pos + 1..];

                // Se il file ha dati, salvalo nella path specificata
                if !file_data.is_empty() {
                    let file_path = Path::new(output_path);
                    if let Err(e) = fs::write(file_path, file_data) {
                        return Err(format!("Errore nella scrittura del file: {}", e));
                    }
                }

                // Return the value 
                Ok(initial_string)
            } else {
                // In this case is a normal string I suggest to revise this. This is for the client user 
                match String::from_utf8(clean_data) {
                    Ok(s) => Ok(s),
                    Err(e) => Err(format!("Errore nella conversione del messaggio in stringa: {}", e)),
                }
            }
        }



    }
}


pub mod file_system
{
    use std::{fmt, fs};
    use std::io::Read;
    use std::path::Path;
    use wg_2024::packet::Packet;


    pub enum ServerType
    {
        TextServer,
        MediaServer,
        CommunicationServer,
    }

    impl fmt::Display for ServerType {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            let serv_type = match self {
                message::file_system::ServerType::TextServer => "TextServer",
                message::file_system::ServerType::MediaServer => "MediaServer",
                message::file_system::ServerType::CommunicationServer => "CommunicationServer",
            };
            write!(f, "{}", serv_type)
        }
    }

    /*
  
      pub trait ActionServer  //list of action that a server can do
      {
          fn evaluateRequest () -> String;
      }
      
      //impl ActionServer for FileSystem | Ch
  */
    pub struct FileSystem {
        path: String, // Path to the directory where files are stored
        serv: ServerType,
    }

    impl FileSystem {
        // Create a new FileSystem instance
        pub fn new(path: &str, serv: ServerType) -> Self {
            let path_dir = Path::new(path);

            // Check if the directory exists
            if path_dir.exists() {
                if !path_dir.is_dir() {
                    panic!("Error: '{}' exists but is not a directory.", path);
                }
            } else {
                // Create the directory if it doesn't exist
                match fs::create_dir_all(path) {
                    Ok(_) => println!("Directory '{}' has been created.", path),
                    Err(err) => panic!("Error: Could not create directory '{}': {}", path, err),
                }
            }


            FileSystem {
                path: path.to_string(),
                serv,

            }
        }

        // List all files in the directory, return as a string
        fn files_list(&self) -> String {
            match fs::read_dir(&self.path) {
                Ok(entries) => {
                    let files: Vec<String> = entries
                        .filter_map(|entry| entry.ok())
                        .map(|entry| entry.file_name().into_string().unwrap_or_default())
                        .collect();
                    format!("files_list!({:?})", files)
                }
                Err(err) => format!("Error: Could not read directory - {}", err),
            }
        }

        // Return the content and size of a specific file
        fn file(&self, file_name: &str) -> String {
            let file_path = Path::new(&self.path).join(file_name);

            match fs::File::open(&file_path) {
                Ok(mut file) => {
                    let mut content = String::new();
                    match file.read_to_string(&mut content) {
                        Ok(size) => format!("({},{})", size, content),
                        Err(err) => format!("error_requested_not_found!(Problem with the conversion in string) - Error: {}", err),
                    }
                }
                Err(err) => "error_requested_not_found!(File not found) ".to_string(),
            }
        }


        pub fn processRequest(&mut self, command: String) -> String
        {
            match command {
                cmd if cmd.starts_with("server_type?") => {
                    self.serv.to_string()
                }
                cmd if cmd.starts_with("files_list?") => {
                    self.files_list()
                }
                cmd if cmd.starts_with("file?") => {
                    if let Some(file_id) = cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")")) {
                        self.file(file_id)
                    } else {
                        "error_requested_not_found!".to_string()
                    }
                }
                cmd if cmd.starts_with("media?") => {
                    if let Some(media_id) = cmd.strip_prefix("media?(").and_then(|s| s.strip_suffix(")")) {
                        self.file(media_id)
                    } else {
                        "error_requested_not_found!".to_string()
                    }
                }
                _ => {
                    "error_unsupported_request!".to_string()
                }
            }
        }
    }


    use std::collections::HashMap;
    use crate::message;

    pub struct ChatServer {
        chats: HashMap<(u32, u32), Vec<String>>,        //CURRENTLY NOT IN USE  -I had a problem reading the documentation now. I thought that the server kept the message inside hime and the client connected to him to retrive the information
        list_of_client: Vec<u32>,
        serv: ServerType,
    }

    impl ChatServer {
        pub fn new() -> Self {
            ChatServer {
                chats: HashMap::new(),
                list_of_client: Vec::new(),
                serv: ServerType::CommunicationServer,      //it's useless but if in the future I add another type of chat server....
            }
        }

        pub fn add_client(&mut self, client: u32) {
            if !self.list_of_client.contains(&client) {
                self.list_of_client.push(client);
            }
        }

        // Function to return the list of all unique client IDs
        pub fn get_client_ids(&mut self, source_id: u32) -> String {
            self.add_client(source_id);
            let mut ids = self.list_of_client.clone();
            ids.sort();         // it's easier when we debug 
            format!("client_list!{:?}", ids)
        }


        //CURRENTLY NOT IN USE 
        // Function to write a message to a specific destination ID
        pub fn write_message(&mut self, source_id: u32, destination_id: u32, message: String) -> String {
            self.add_client(source_id);
            if !self.list_of_client.contains(&destination_id) {
                return "err".to_string();
            }
            let key = (source_id, destination_id);
            self.chats.entry(key).or_insert_with(Vec::new).push(message);
            "ok".to_string()
        }


        //CURRENTLY NOT IN USE 
        // Function to retrieve messages for a given source and destination ID
        pub fn get_messages(&self, source_id: u32, destination_id: u32) -> String {             //in this function there isn't self.add_client(source_id) because there is no point in putting it 
            let key = (source_id, destination_id);
            if let Some(messages) = self.chats.get(&key) {
                messages.join("\n")
            } else {
                "error_wrong_client_id!".to_string()
            }
        }


        pub fn processRequest(&mut self, command: String, source_id: u32) -> String
        {
            match command {
                cmd if cmd.starts_with("server_type?") => {         //it's
                    self.serv.to_string()
                }
                cmd if cmd.starts_with("client_list?") => {
                    self.get_client_ids(source_id)
                }

                cmd if cmd.starts_with("message_for?") => {
                    if let Some(content) = cmd.strip_prefix("message_for?(").and_then(|s| s.strip_suffix(")")) {
                        // Divide il contenuto in id e messaggio
                        let parts: Vec<&str> = content.splitn(2, ',').collect();
                        if parts.len() == 2 {
                            let id = parts[0];
                            let message = parts[1];
                            
                            format!("!{}!message_from!({}, {})", id, source_id, message)        // I use this format for identify this special message (I have to send the message to a different client not the original one)
                        } else {
                            "error_unsupported_request!".to_string()
                        }
                    } else {
                        "error_unsupported_request!".to_string()
                    }
                }

                _ => {
                    "error_wrong_client_id!".to_string()
                }
            }
        }
    }
}



#[cfg(test)]
mod test_packaging
{
    use super::*;
    use crate::message::packaging::Repackager;



    #[test]
    fn fragmentation_with_string() {
        let result = Repackager::create_fragments(&*"A".repeat(200), None);
        println!("{:?}", result);
        let mut rp = Repackager::new();
        let mut  transformation = Ok(Some(vec![]));
        for i in result.unwrap().iter() {
             transformation = rp.process_fragment(1,1,i.clone());
            println!("{:?}", transformation);
        }
        println!("{:?}", Repackager::assemble_string(transformation.unwrap().unwrap()));
        

    }    
    
    #[test]
    fn fragmentation_with_txtfile() {
        let result = Repackager::create_fragments("file!(435,",Some("/tmp/testServer/file1.txt"));
        //println!("{:?}", result);  
        let mut rp = Repackager::new();
        let mut  transformation = Ok(Some(vec![]));
        for i in result.unwrap().iter() {
            transformation = rp.process_fragment(1,1,i.clone());
           // println!("{:?}", transformation);
        }
        println!("{:?}", Repackager::assemble_string_file(transformation.unwrap().unwrap(),"/tmp/testServer/copy/a"));


    }

    #[test]
    fn fragmentation_with_Mediafile() {
        let result = Repackager::create_fragments("media!(,",Some("/tmp/testServer/3 Nights.mp3"));
        println!("{:?}", result);
        let mut rp = Repackager::new();
        let mut transformation =Ok(Some(vec![]));
        for i in result.unwrap().iter() {
             transformation = rp.process_fragment(1,1,i.clone());
        }
        match transformation {
            Ok(Some(trans)) => {
                println!(
                    "{:?}",
                    Repackager::assemble_string_file(trans, "/tmp/testServer/copy/a.mp3")
                );
            }
            _ => panic!("Trasformazione fallita."),
        }
    }
  
}


    #[cfg(test)]
    mod tests_chat_server {
        use super::*;

        #[test]
        fn test_server_type() {
            let mut server = ChatServer::new();
            let response = server.processRequest("server_type?".to_string(), 1);
            assert_eq!(response, "CommunicationServer");
        }

        #[test]
        fn test_add_client_and_get_client_ids() {
            let mut server = ChatServer::new();
            server.add_client(1);
            server.add_client(2);
            let response = server.get_client_ids(3); // Aggiunge anche il client 3
            assert_eq!(response, "client_list![1, 2, 3]");
        }

        #[test]
        fn test_get_client_list_via_process_request() {
            let mut server = ChatServer::new();
            server.add_client(1);
            server.add_client(2);
            let response = server.processRequest("client_list?".to_string(), 3); // Aggiunge anche il client 3
            assert_eq!(response, "client_list![1, 2, 3]");
        }

        #[test]
        fn test_message_for_request() {
            let mut server = ChatServer::new();
            let response = server.processRequest("message_for?(2,He,llo)".to_string(), 1);
            //assert_eq!(response, "message_from!(Hello)");
            println!("{}", response);
        }

        #[test]
        fn test_unsupported_request() {
            let mut server = ChatServer::new();
            let response = server.processRequest("unsupported_command?".to_string(), 1);
            assert_eq!(response, "error_wrong_client_id!");
        }

        #[test]
        fn test_write_message_not_in_use() {
            let mut server = ChatServer::new();
            server.add_client(1);
            server.add_client(2);
            let response = server.write_message(1, 2, "Hello, Client 2!".to_string());
            assert_eq!(response, "ok");

            // Retrieve messages (even if not in use)
            let messages = server.get_messages(1, 2);
            assert_eq!(messages, "Hello, Client 2!");
        }

        #[test]
        fn test_get_messages_error() {
            let server = ChatServer::new();
            let messages = server.get_messages(1, 99); // Client 99 non esiste
            assert_eq!(messages, "error_wrong_client_id!");
        }
    }

    
    
    
    
    
    
mod tests_message_media_server {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use crate::message::file_system::ServerType;

    // Helper function to set up a temporary test directory
    fn setup_test_dir(test_dir: &str) {
        let _ = fs::remove_dir_all(test_dir); // Cleanup if the directory exists
        fs::create_dir(test_dir).unwrap();
        let mut file1 = fs::File::create(format!("{}/file1.txt", test_dir)).unwrap();
        writeln!(file1, "This is the content of file1.").unwrap();
        let mut file2 = fs::File::create(format!("{}/file2.txt", test_dir)).unwrap();
        writeln!(file2, "This is the content of file2.").unwrap();
    }

    #[test]
    fn test_server_type() {
        let mut fs = FileSystem::new("test_dir", ServerType::TextServer);
        let response = fs.processRequest("server_type?".to_string());
        assert_eq!(response, "TextServer");
    }

    #[test]
    fn test_files_list() {
        let test_dir = "/tmp/testServer";
        //setup_test_dir(test_dir);

        let mut fs = FileSystem::new(test_dir, ServerType::TextServer);
        let response = fs.processRequest("files_list?".to_string());
        println!("{}", response);
        assert!(response.contains("file1.txt"));
        assert!(response.contains("file2.txt"));
    }

    #[test]
    fn test_file_content() {
        let test_dir = "/tmp/testServer";
        //setup_test_dir(test_dir);

        let mut fs = FileSystem::new(test_dir, ServerType::TextServer);
        let response = fs.processRequest("file?(3 Nights.mp3)".to_string());
        println!("{}", response);
        //assert!(response.contains("This is the content of file1."));
    }

    #[test]
    fn test_file_not_found() {
        let test_dir = "/tmp/testServer";
        //setup_test_dir(test_dir);

        let mut fs = FileSystem::new(test_dir, ServerType::TextServer);
        let response = fs.processRequest("file?(nonexistent.txt)".to_string());
        assert_eq!(response, "error_requested_not_found!(File not found) ");
    }

    #[test]
    fn test_unsupported_request() {
        let mut fs = FileSystem::new("test_dir", ServerType::TextServer);
        let response = fs.processRequest("unsupported_command?".to_string());
        assert_eq!(response, "error_unsupported_request!");
    }
}










