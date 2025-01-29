#![allow(warnings)]
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use wg_2024::network::NodeId;
use wg_2024::packet::Fragment;


// Struct and functions to handle fragment reassembly and collection
// FragmentReassembler to manage reassembly of fragments
pub struct FragmentReassembler {
    pub buffer: HashMap<(u64, NodeId), Vec<u8>>, // Map of (session_id, source_id) to message buffer
    pub processed_fragments: HashMap<(u64, NodeId), u8>, // Number of received fragments per (session_id, source_id)
}

impl FragmentReassembler {
    pub fn new() -> Self {
        Self {
            buffer: HashMap::new(),
            processed_fragments: HashMap::new(),
        }
    }
    pub fn add_fragment(&mut self, session_id: u64, source_id: NodeId, fragment: Fragment) -> Result<Option<Vec<u8>>,u8> {
        let key = (session_id, source_id);

        // Initialize tracking structures for this (session_id, source_id) pair if needed
        if !self.buffer.contains_key(&key) {
            self.buffer.insert(key, Vec::with_capacity((fragment.total_n_fragments * 128) as usize));
        }
        // Get the buffer for this (session_id, source_id)
        let buffer = self.buffer.get_mut(&key).unwrap();

        // Copy the fragment's data into the correct position in the buffer
        let start = (fragment.fragment_index * 128) as usize;
        let end = start + fragment.length as usize;
        buffer[start..end].copy_from_slice(&fragment.data[..fragment.length as usize]);

        // Update count of received fragments
        *self.processed_fragments.entry(key).or_insert(0) +=1;
        if self.processed_fragments[&key] == fragment.total_n_fragments as u8 {
            // Reassemble the message
            let total_length = ((fragment.total_n_fragments - 1) * 128 + fragment.length as u64) as usize;
            let message = buffer[..total_length].to_vec();

            // Clean up tracking structures
            self.buffer.remove(&key);
            self.processed_fragments.remove(&key);

            // Return reassembled message
            Ok(Some(message))
        } else {
            // If not all fragments received, return None
            Ok(None)
        }
    }
    pub fn create_fragments(str: &str) -> Result<Vec<Fragment>, String> {
        // Convert the initial string to bytes
        let mut message_data = str.as_bytes().to_vec();

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
    pub fn assemble_string_file(data: Vec<u8>, output_path: &str) -> Result<String, String> {
        // Remove null character
        let clean_data = data.into_iter().take_while(|&byte| byte != 0).collect::<Vec<_>>();

        // Search position of first separator
        let separator_pos = clean_data.iter().position(|&b| b == b'(' );

        if let Some(pos) = separator_pos {
            // Take the initial string
            let initial_string = match String::from_utf8(clean_data[..pos].to_vec()) {
                Ok(s) => s,
                Err(e) => return Err(format!("Error while converting initial string: {}", e)),
            };

            // Extract file content
            let file_data = &clean_data[pos + 1..];

            // If file has data save it in the specified path
            if !file_data.is_empty() {
                let file_path = Path::new(output_path);
                if let Err(e) = fs::write(file_path, file_data) {
                    return Err(format!("Error while writing file: {}", e));
                }
            }
            // Return  value
            Ok(initial_string)
        } else {
            // In case of a normal string
            match String::from_utf8(clean_data) {
                Ok(s) => Ok(s),
                Err(e) => Err(format!("Error while converting message to string: {}", e)),
            }
        }
    }
}

