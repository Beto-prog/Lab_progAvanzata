#![allow(warnings)]

use std::fs::File;
use base64::{engine::general_purpose, Engine as _};
use std::collections::{HashMap};
use std::fs;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use wg_2024::packet::{Fragment};

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
3. If it has not recived a fragment with the same (session_id, src_id) tuple, then it creates a vector (Vec<u8> with capacity of total_n_fragments * 128) where to copy the data of the fragments.
4. It would then copy length elements of the data array at the correct offset in the vector.


*/


    //this function is complicated but it works ignore the test with mediafile because it involve memory permission
    pub fn process_fragment(&mut self, session_id: u64, src_id: u64, fragment: Fragment) -> Result<Option<Vec<u8>>, u8> {
        let key = (session_id, src_id);

        // Check if buffer for this session_id and src_id exists, if not, create it
        let buffer = self.buffers.entry(key).or_insert_with(|| {
            Vec::with_capacity((fragment.total_n_fragments * 128) as usize)
        });

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

        for i in 0..total_n_fragments {         //take data of the file piece by piece
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


    //from fragment -> to string
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


    //usless for the server this is for the Client . Used for testing and to have a more   complete module
    pub fn assemble_string_file(data: Vec<u8>) -> Result<String, String> {
        match String::from_utf8(data) {
            Ok(mut string) => {
                // Remove trailing null characters
                string = string.trim_end_matches('\0').to_string();
                Ok(string)
            }
            Err(e) => Err(format!("Failed to convert data to string: {}", e)),
        }
    }
    pub fn assemble_image_file(data: Vec<u8>) -> Result<String, String> {
        Ok(general_purpose::STANDARD.encode(data))
    }
    pub fn assemble_file(data: Vec<u8>, output_path: &str) -> Result<String, String> {
        // Trova il primo '('
        let payload_start = data.iter()
            .position(|&b| b == b'(')
            .ok_or("Missing '(' in message")? + 1;

        // Trova l'ultima ')' DOPO l'inizio del payload — ma solo se esiste
        let mut payload_end = data.len();
        if let Some(last_paren) = data.iter().rposition(|&b| b == b')') {
            if last_paren > payload_start {
                payload_end = last_paren;
            }
        }

        if payload_end <= payload_start {
            return Err("Invalid payload range".to_string());
        }

        let payload = &data[payload_start..payload_end];

        // Scrive il file
        let mut file = File::create(Path::new(output_path))
            .map_err(|e| format!("Error while creating file: {e}"))?;
        file.write_all(payload)
            .map_err(|e| format!("Error while writing file: {e}"))?;

        Ok("File saved correctly".into())
    }
}