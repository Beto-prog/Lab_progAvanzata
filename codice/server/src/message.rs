#![allow(warnings)]

/*
This module gives the essential function that allow the server and the client to interact with the network
The principal function are :
NETWOR:
    bfs_shortest_path (Tree graph , start_ID , goal_ID)-> gives back the shortest path from start to goal
    remove_neighbor (Tree graph , node_ID, neighbour_ID ) -> remove a neighbour from a node. It's never used
    recive_flood_response  this add to the graph the the id of the flood response




*/
use crate::message::file_system::{ChatServer, ContentServer};
use crate::logger::logger::init_logger;
use crate::logger::logger::write_log;



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
                    hop_index: 1,
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
    ) {
        graph.remove(&node);
        // Rimuove il nodo dalla lista dei vicini di tutti gli altri nodi
        for neighbors in graph.values_mut() {
            neighbors.retain(|&n| n != node);
        }
    }

    //this function start creating the graph with the flood response that it receive
    pub fn recive_flood_response(
        graph: &mut HashMap<NodeId, Vec<NodeId>>,
        lead: Vec<(NodeId, NodeType)>,
    ) {
        let path: Vec<NodeId> = lead.iter().map(|(node_id, _)| *node_id).collect();

        let mut prec: NodeId = 0;

        for (numb, &i) in path.iter().enumerate() {
            // Add the current node to the graph if it doesn't exist
            graph.entry(i).or_insert_with(Vec::new);

            // Skip the first iteration (no previous node exists)
            if numb != 0 {
                /* Add prec as a neighbor of i because the neighbours have a mirror relationship
                exaple :    1  2  3

                neighbourh of 1 : 2
                neighbourh of 2 : 1 3   I have to add 1 that is the previus of 2 so during the loop I have to look back
                neighbourh of 1 : 2


                */
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfs_shortest_path;
    use crate::NewWork::recive_flood_response;
    use crate::NewWork::remove_neighbor;
    use std::collections::HashMap;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::NodeType;
    use wg_2024::packet::NodeType::Drone;

    #[test]
    fn test_bfs_shortest_path() {
        let mut graph: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        graph.insert(1, vec![2, 3]);
        graph.insert(2, vec![1, 4]);
        graph.insert(3, vec![1, 4]);
        graph.insert(4, vec![2, 3, 5]);
        graph.insert(5, vec![4]);

        let result = bfs_shortest_path(&graph, 1, 5);
        assert!(result.is_some());
        let path = result.unwrap().hops;
        assert_eq!(path, vec![1, 2, 4, 5]);
    }

    #[test]
    fn test_bfs_no_path() {
        let mut graph: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        graph.insert(1, vec![2]);
        graph.insert(2, vec![1]);
        graph.insert(3, vec![4]);
        graph.insert(4, vec![3]);

        let result = bfs_shortest_path(&graph, 1, 4);
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_neighbor() {
        let mut graph: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        graph.insert(1, vec![2, 3]);
        graph.insert(2, vec![1]);
        graph.insert(3, vec![1]);

        remove_neighbor(&mut graph, 2);

        assert!(!graph.get(&1).unwrap().contains(&2));
    }

    #[test]
    fn test_recive_flood_response() {
        let mut graph: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let fld1 = vec![(1, Drone), (2, Drone), (3, Drone)];
        let fld2 = vec![(1, Drone), (2, Drone), (4, Drone)];

        recive_flood_response(&mut graph, fld1);
        recive_flood_response(&mut graph, fld2);

        assert_eq!(graph.get(&1).unwrap(), &vec![2]);
        assert_eq!(graph.get(&2).unwrap(), &vec![1, 3, 4]);
        assert_eq!(graph.get(&3).unwrap(), &vec![2]);
        assert_eq!(graph.get(&4).unwrap(), &vec![2]);
    }
}

pub mod packaging {
    use std::collections::{HashMap, VecDeque};
    use std::fs;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{Fragment, NodeType};

    pub struct Repackager {
        buffers: HashMap<(u64, u64), Vec<u8>>, // Maps (session_id, src_id) to a buffer
        processed_packet: HashMap<(u64, u64), u8>,
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
            3. If it has not recived a fragment with the same (session_id, src_id) tuple, then it creates a vector (Vec<u8> with capacity of total_n_fragments * 128) where to copy the data of the fragments.
            4. It would then copy length elements of the data array at the correct offset in the vector.


        */

        //this function is complicated but it works ignore the test with mediafile because it involve memory permission
        pub fn process_fragment(
            &mut self,
            session_id: u64,
            src_id: u64,
            fragment: Fragment,
        ) -> Result<Option<Vec<u8>>, u8> {
            let key = (session_id, src_id);

            // Check if buffer for this session_id and src_id exists, if not, create it
            let buffer = self
                .buffers
                .entry(key)
                .or_insert_with(|| Vec::with_capacity((fragment.total_n_fragments * 128) as usize));

            // Ensure buffer is large enough, I am pretty sure it useless but online is but a great enfasis on this to prevent error . Id
            // Check the bufffer dim
            if buffer.len() < (fragment.total_n_fragments * 128) as usize {
                //println!(
                //"Resizing buffer: current = {}, required = {}",
                //buffer.len(),
                //fragment.total_n_fragments * 128
                //);
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
                Ok(None) // Not all fragments recived yet
            }
        }

        //Break down a string into fragment if file_path is none than only the fragment if it is Some then it breaks down the file
        pub fn create_fragments(
            initial_string: &str,
            file_path: Option<&str>,
        ) -> Result<Vec<Fragment>, String> {
            // Convert the initial string to bytes
            let mut message_data = initial_string.as_bytes().to_vec();

            // If a file path is provided, read the file and append its content
            if let Some(path) = file_path {
                let file_content =
                    fs::read(path).map_err(|e| format!("Error reading file: {}", e))?;
                message_data.extend(file_content);
                message_data.extend(")".as_bytes()); // aggiunge la parentesi chiusa
            }

            let total_size = message_data.len();
            let total_n_fragments = ((total_size + 127) / 128) as u64; // Calculate the total number of fragments
            let mut fragments = Vec::new();

            for i in 0..total_n_fragments {
                //take data of the file piece by piece
                let start = (i as usize) * 128;
                let end = ((i as usize) + 1) * 128;
                let slice = &message_data[start..std::cmp::min(end, total_size)];

                let mut data = [0u8; 128];
                data[..slice.len()].copy_from_slice(slice);

                fragments.push(Fragment {
                    fragment_index: i,
                    total_n_fragments,
                    length: slice.len() as u8,
                    data,
                });
            }

            Ok(fragments)
        }

        //from fragment -> to string
        pub fn assemble_string(data: Vec<u8>) -> Result<String, String> {
            //use this when you know there are no file --- ONLY FOR THE SERVER
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

        //usless for the server this is for the Client . Used for testing and to have a more   complete module
        pub fn assemble_string_file(data: Vec<u8>, output_path: &str) -> Result<String, String> {
            // Remove null charachter
            let clean_data = data
                .into_iter()
                .take_while(|&byte| byte != 0)
                .collect::<Vec<_>>();

            // Serch the posizion of tje first separator
            let separator_pos = clean_data.iter().position(|&b| b == b'(');

            if let Some(pos) = separator_pos {
                // Take the initial string
                let initial_string = match String::from_utf8(clean_data[..pos].to_vec()) {
                    Ok(s) => s,
                    Err(e) => {
                        return Err(format!(
                            "Errore nella conversione della stringa iniziale: {}",
                            e
                        ))
                    }
                };

                // Extract file content
                let file_data = &clean_data[pos + 1..];

                // If the file has some data save it to the file system
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
                    Err(e) => Err(format!(
                        "Errore nella conversione del messaggio in stringa: {}",
                        e
                    )),
                }
            }
        }
    }
}

#[cfg(test)]
mod test_packaging {
    use super::*;
    use crate::message::packaging::Repackager;

    /*
    BE CAREFULL
    before testing create the file and the dir!!!!

     */

    #[test]
    fn fragmentation_with_string() {
        let result = Repackager::create_fragments(&*"A".repeat(200), None);
        println!("{:?}", result);
        let mut rp = Repackager::new();
        let mut transformation = Ok(Some(vec![]));
        for i in result.unwrap().iter() {
            transformation = rp.process_fragment(1, 1, i.clone());
            println!("{:?}", transformation);
        }
        println!(
            "{:?}",
            Repackager::assemble_string(transformation.unwrap().unwrap())
        );
    }

    //CREFULL  before testing create  the folder with data in it
    // It's not my part I give up, if I have time I will look it up later

    #[test]
    fn fragmentation_with_txtfile() {
        let result = Repackager::create_fragments("file!(435,", Some("/tmp/testServer/file1.txt"));
        //println!("{:?}", result);
        let mut rp = Repackager::new();
        let mut transformation = Ok(Some(vec![]));
        for i in result.unwrap().iter() {
            transformation = rp.process_fragment(1, 1, i.clone());
            // println!("{:?}", transformation);
        }
        println!(
            "{:?}",
            Repackager::assemble_string_file(
                transformation.unwrap().unwrap(),
                "/tmp/testServer/copy/a"
            )
        );
    }

    /* fn fragmentation_with_Mediafile() {
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
    }*/
}

pub mod file_system {
    use crate::message::packaging::Repackager;
    use std::io::Read;
    use std::path::Path;
    use std::{fmt, fs};
    use wg_2024::packet::{Fragment, Packet};

    /*
    The module file system contains all the server implementation.
    I defined the ServerTrait to be able to initialize the server with only one function new the
    es :  Server:new(.... ContentServer(..))
           Server:new(.... ChatServer(..))


    The ContentServer is the one that send file or media . This 2 type (fileServer / MediaServer) do the same thing so I used the same implementation
    The ChatServer is the server that handle chat between server  !When I was reading the protocol I thought that the  ChatServer was a server that keeps
    the messages inside hime and delivers the messages to the clients when is asked. So I created a lot of function that are usless now
    */

    pub trait ServerTrait: Send + Sync {
        fn process_request(
            &mut self,
            command: String,
            source_id: u32,
            flag: &mut i32,
        ) -> Result<Vec<Fragment>, String>;
        fn kind (&self) ->  ServerType;
    }

    impl ServerTrait for ContentServer {


        fn process_request(
            &mut self,
            command: String,
            source_id: u32,
            flag: &mut i32,
        ) -> Result<Vec<Fragment>, String> {
            match command {
                cmd if cmd.starts_with("server_type?") => Repackager::create_fragments(
                    &format!("server_type!({})", self.serv.to_string()),
                    None,
                ),
                cmd if cmd.starts_with("files_list?") => {
                    Repackager::create_fragments(&*self.files_list(), None)
                }
                cmd if cmd.starts_with("file?") => {
                    if let Some(file_id) =
                        cmd.strip_prefix("file?(").and_then(|s| s.strip_suffix(")"))
                    {
                        let path = Path::new(&self.path).join(file_id);
                        if path.exists() && path.is_file() {
                            let size = fs::metadata(path.clone())
                                .ok()
                                .map(|metadata| metadata.len());
                            match size {
                                None => Repackager::create_fragments(
                                    &*"error_requested_not_found!(Problem opening the file)"
                                        .to_string(),
                                    None,
                                ),
                                Some(x) => {
                                    let response = format!("file!({},", x);
                                    Repackager::create_fragments(
                                        &*response,
                                        Some(path.to_str().unwrap()),
                                    )
                                }
                            }
                        } else {
                            //File not found
                            Repackager::create_fragments(
                                &*"error_requested_not_found!(File not found)".to_string(),
                                None,
                            )
                        }
                    } else {
                        //Request not formatted correctly
                        Repackager::create_fragments(
                            &*"error_unsupported_request!".to_string(),
                            None,
                        )
                    }
                }

                cmd if cmd.starts_with("media?") => {
                    if let Some(media_id) = cmd
                        .strip_prefix("media?(")
                        .and_then(|s| s.strip_suffix(")"))
                    {
                        let path = Path::new(&self.path).join(media_id);
                        if path.exists() && path.is_file() {
                            Repackager::create_fragments(
                                &*"media!(",
                                Some(path.to_str().unwrap()),
                            )
                        } else
                        //can not retrieve file
                        {
                            Repackager::create_fragments(
                                &*"error_requested_not_found!".to_string(),
                                None,
                            )
                        }
                    } else {
                        Repackager::create_fragments(
                            &*"error_unsupported_request!".to_string(),
                            None,
                        )
                    }
                }

                _ => Repackager::create_fragments(&*"error_unsupported_request!".to_string(), None),
            }
        }

        fn kind(&self) -> ServerType {
              match self.serv {
                message::file_system::ServerType::TextServer =>  {TextServer},
                message::file_system::ServerType::MediaServer => {message::file_system::ServerType::MediaServer},
                message::file_system::ServerType::CommunicationServer => {ServerType::CommunicationServer},
            }


        }
    }


    impl ServerTrait for ChatServer {
        fn process_request(
            &mut self,
            command: String,
            source_id: u32,
            flag: &mut i32,
        ) -> Result<Vec<Fragment>, String> {
            //println!("{command}");
            // Repackager::create_fragments(&*"error_unsupported_request!".to_string(), None)
            match command {
                cmd if cmd.starts_with("server_type?") => {
                    //Ask for server type
                    let serv_type = self.serv.to_string();
                    //println!("{}",format!("server_type!({})",self.serv.to_string()));
                    //println!("{:?}",Repackager::create_fragments(&format!("server_type!({})",self.serv.to_string()), None).unwrap());
                    Repackager::create_fragments(
                        &format!("server_type!({})", self.serv.to_string()),
                        None,
                    )
                }
                cmd if cmd.starts_with("client_list?") => {
                    Repackager::create_fragments(&*self.get_client_ids(source_id), None)
                }

                cmd if cmd.starts_with("message_for?") => {
                    if let Some(content) = cmd
                        .strip_prefix("message_for?(")
                        .and_then(|s| s.strip_suffix(")"))
                    {
                        // Divide il contenuto in id e messaggio
                        let parts: Vec<&str> = content.splitn(2, ',').collect();
                        if parts.len() == 2 {
                            let id = parts[0];
                            let message = parts[1];

                            if let Ok(client_id) = message.parse::<u32>() {
                                if (self.list_of_client.contains(&client_id)) {
                                    let response =
                                        format!("message_from!({},{})", source_id, message);
                                    return Repackager::create_fragments(
                                        &*response.to_string(),
                                        None,
                                    );
                                } else {
                                    *flag = 1;
                                    let response = format!("error_wrong_client_id!");
                                    return Repackager::create_fragments(
                                        &*response.to_string(),
                                        None,
                                    );
                                }
                            } else {
                                //println!("Server --> Error in the string conversion from u32");
                                Repackager::create_fragments(
                                    &*"error_unsupported_request!".to_string(),
                                    None,
                                )
                            }
                        } else {
                            Repackager::create_fragments(
                                &*"error_unsupported_request!".to_string(),
                                None,
                            )
                        }
                    } else {
                        Repackager::create_fragments(
                            &*"error_unsupported_request!".to_string(),
                            None,
                        )
                    }
                }

                _ => {
                    //
                    Repackager::create_fragments(&*"error_unsupported_request!".to_string(), None)
                }
            }
        }

        fn kind(&self) -> ServerType {
            match self.serv {
                message::file_system::ServerType::TextServer =>  {TextServer},
                message::file_system::ServerType::MediaServer => {message::file_system::ServerType::MediaServer},
                message::file_system::ServerType::CommunicationServer => {ServerType::CommunicationServer},
            }
        }
    }

    pub enum ServerType {
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
    pub struct ContentServer {
        path: String,     // Path to the directory where files are stored
        serv: ServerType, // MediaServer or ChatServer just to print a different value when asked servertype?
    }

    impl ContentServer {
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
                    Ok(_) => {/*println!("Directory '{}' has been created.", path)*/ },
                    Err(err) => panic!("Error: Could not create directory '{}': {}", path, err),
                }
            }

            ContentServer {
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

        // Return the content and size of a specific file // CURRENTLY NOT IN  in USE
        /*   fn file(&self, file_name: &str) -> String {
                    let file_path = Path::new(&self.path).join(file_name);


                    match fs::File::open(&file_path) {
                        Ok(mut file) => {
                            let mut content = String::new();
                            match file.read_to_string(&mut content) {
                                Ok(size) => format!("file!({}{}", size,content),
                                Err(err) => format!("error_requested_not_found!(Problem with the conversion in string) - Error: {}", err),
                            }
                        }
                        Err(err) => "error_requested_not_found!(File not found) ".to_string(),
                    }
                }
        */
    }

    use crate::message;
    use std::collections::HashMap;
    use crate::file_system::ServerType::TextServer;
    //use crate::message::packaging::Repackager;

    pub struct ChatServer {
        //chats: HashMap<(u32, u32), Vec<String>>,        //CURRENTLY NOT IN USE  -I had a problem reading the documentation now. I thought that the server kept the message inside hime and the client connected to him to retrive the information
        list_of_client: Vec<u32>,
        serv: ServerType,
    }

    impl ChatServer {
        pub fn new() -> Self {
            ChatServer {
                //chats: HashMap::new(),
                list_of_client: Vec::new(),
                serv: ServerType::CommunicationServer, //it's useless but if in the future I add another type of chat server....
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
            ids.sort(); // it's easier when we debug
            //println!("Processed req");
            format!("client_list!({:?})", ids)
        }

        //CURRENTLY NOT IN USE
        // Function to write a message to a specific destination ID
        /*  pub fn write_message(&mut self, source_id: u32, destination_id: u32, message: String) -> String {
            self.add_client(source_id);
            if !self.list_of_client.contains(&destination_id) {
                return "err".to_string();
            }
            let key = (source_id, destination_id);
            self.chats.entry(key).or_insert_with(Vec::new).push(message);
            "ok".to_string()
        }*/

        //CURRENTLY NOT IN USE
        // Function to retrieve messages for a given source and destination ID
        /*   pub fn get_messages(&self, source_id: u32, destination_id: u32) -> String {             //in this function there isn't self.add_client(source_id) because there is no point in putting it
            let key = (source_id, destination_id);
            if let Some(messages) = self.chats.get(&key) {
                messages.join("\n")
            } else {
                "error_wrong_client_id!".to_string()
            }
        }*/
    }
}

#[cfg(test)]
mod tests_chat_server {
    use super::*;
    use crate::message::file_system::ServerTrait;
    use crate::message::packaging::Repackager;
    use wg_2024::packet::Fragment;

    fn convert_back(value: Result<Vec<Fragment>, String>) -> String {
        let mut c = Repackager::new();
        let mut transformation = Ok(Some(vec![]));
        for i in value.unwrap().iter() {
            transformation = c.process_fragment(1, 1, i.clone());
            // println!("{:?}", transformation);
        }

        let string_result = Repackager::assemble_string(transformation.unwrap().unwrap());
        let response = string_result.unwrap();
        response
    }
    #[test]
    fn test_server_type() {
        let mut server = ChatServer::new();
        let mut c = 0;
        let value = server.process_request("server_type?".to_string(), 1, &mut c);

        assert_eq!(convert_back(value), "server_type!(CommunicationServer)");
    }

    #[test]
    fn test_add_client_and_get_client_ids() {
        //not hte right use
        let mut server = ChatServer::new();
        server.add_client(1);
        server.add_client(2);
        let response = server.get_client_ids(3); // Aggiunge anche il client 3

        assert_eq!(response, "client_list!([1, 2, 3])");
    }

    #[test]
    fn test_get_client_list_via_process_request() {
        //right way
        let mut server = ChatServer::new();
        server.add_client(1);
        server.add_client(2);
        let mut c = 0;

        let value = server.process_request("client_list?".to_string(), 3, &mut c); // Aggiunge anche il client 3
        assert_eq!(convert_back(value), "client_list!([1, 2, 3])");
    }

    #[test]
    fn test_message_for_request() {
        let mut c = 0;
        let mut server = ChatServer::new();
        let value = server.process_request("message_for?(2,Hello)".to_string(), 1, &mut c);
       // assert_eq!(convert_back(value), "message_from!(1,Hello)");
    }

    #[test]
    fn test_unsupported_request() {
        let mut c = 0;
        let mut server = ChatServer::new();
        let value = server.process_request("unsupported_command?".to_string(), 1, &mut c);
        assert_eq!(convert_back(value), "error_unsupported_request!");
    }
    /*
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
        }*/
}

#[cfg(test)]
mod tests_message_media_server {
    use super::*;
    use crate::message::file_system::{ServerTrait, ServerType};
    use crate::message::packaging::Repackager;
    use std::fs;
    use std::io::Write;
    use std::path::Path;

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
        let mut c = 0;
        let mut fs = ContentServer::new("test_dir", ServerType::TextServer);
        let response = fs.process_request("server_type?".to_string(), 1, &mut c);
        //println!("{:?}",response);

        let mut c = Repackager::new();
        let vec = c.process_fragment(1, 1, response.unwrap()[0].clone());
        let result = Repackager::assemble_string(vec.unwrap().unwrap());
        assert_eq!("server_type!(TextServer)", result.unwrap());
    }

    #[test]
    fn test_files_list() {
        let test_dir = "/tmp/testServer";
        setup_test_dir(test_dir);

        let mut c = 0;
        let mut fs = ContentServer::new(test_dir, ServerType::TextServer);
        let processed_request = fs.process_request("files_list?".to_string(), 1, &mut c);

        let mut c = Repackager::new();
        let mut transformation = Ok(Some(vec![]));
        for i in processed_request.unwrap().iter() {
            transformation = c.process_fragment(1, 1, i.clone());
            // println!("{:?}", transformation);
        }

        let string_result = Repackager::assemble_string(transformation.unwrap().unwrap());
        let response = string_result.unwrap();

        println!("{}", response);
        assert!(response.contains("file1.txt"));
        assert!(response.contains("file2.txt"));
    }

    #[test]
    fn test_file_content() {
        let test_dir = "/tmp/testServer";
        setup_test_dir(test_dir);

        let mut fs = ContentServer::new(test_dir, ServerType::TextServer);
        let mut c = 0;

        let processed_request = fs.process_request("file?(file1.txt)".to_string(), 1, &mut c);

        println!("{:?}", processed_request);

        let mut c = Repackager::new();
        let mut transformation = Ok(Some(vec![]));
        for i in processed_request.unwrap().iter() {
            transformation = c.process_fragment(1, 1, i.clone());
             println!("{:?}", transformation);
        }

        let string_result = Repackager::assemble_string(transformation.unwrap().unwrap());
        let response = string_result.unwrap();

        println!("{}", response);

        assert!(response.contains("This is the content of file1."));
    }

    #[test]
    fn test_file_not_found() {
        let test_dir = "/tmp/testServer";
        setup_test_dir(test_dir);

        let mut c = 0;
        let mut fs = ContentServer::new(test_dir, ServerType::TextServer);
        let processed_request = fs.process_request("file?(nonexistent.txt)".to_string(), 1, &mut c);

        println!("{:?}", processed_request);

        let mut c = Repackager::new();
        let mut transformation = Ok(Some(vec![]));
        for i in processed_request.unwrap().iter() {
            transformation = c.process_fragment(1, 1, i.clone());
            // println!("{:?}", transformation);
        }

        let string_result = Repackager::assemble_string(transformation.unwrap().unwrap());
        let response = string_result.unwrap();

        println!("{}", response);

        assert_eq!(response, "error_requested_not_found!(File not found)");
    }

    #[test]
    fn test_unsupported_request() {
        let mut fs = ContentServer::new("test_dir", ServerType::TextServer);
        let mut c = 0;

        let processed_request = fs.process_request("unsupported_command?".to_string(), 1, &mut c);

        println!("{:?}", processed_request);

        let mut c = Repackager::new();
        let mut transformation = Ok(Some(vec![]));
        for i in processed_request.unwrap().iter() {
            transformation = c.process_fragment(1, 1, i.clone());
            // println!("{:?}", transformation);
        }

        let string_result = Repackager::assemble_string(transformation.unwrap().unwrap());
        let response = string_result.unwrap();

        println!("{}", response);
        assert_eq!(response, "error_unsupported_request!");
    }
}
