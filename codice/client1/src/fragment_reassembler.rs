
use std::collections::{HashMap};
use std::fs;
use crossbeam_channel::{unbounded};
use wg_2024::network::NodeId;
use wg_2024::packet::{Fragment, Packet};
use crate::Client1;

// Struct FragmentReassembler to manage reassembly of fragments
pub struct FragmentReassembler {
    pub buffer: HashMap<(u64, NodeId), Vec<u8>>, // Map of (session_id, source_id) to message buffer
    pub processed_fragments: HashMap<(u64, NodeId), u8>, // Number of received fragments per (session_id, source_id)
}

impl FragmentReassembler {
    // Create a new FragmentReassembler
    pub fn new() -> Self {
        Self {
            buffer: HashMap::new(),
            processed_fragments: HashMap::new(),
        }
    }
    // Add a fragment in the buffer and proceed to assemble the message as a Vec is all fragments received
    pub fn add_fragment(&mut self, session_id: u64, source_id: NodeId, fragment: Fragment) -> Option<Vec<u8>> {
        let key = (session_id, source_id);

        // Initialize tracking structures for this (session_id, source_id) pair if needed
        if !self.buffer.contains_key(&key) {
            self.buffer.insert(key, Vec::with_capacity((fragment.total_n_fragments * 128) as usize));
        }
        // Get the buffer for this (session_id, source_id)
        let buffer = self.buffer.get_mut(&key).expect("Failed to get buffer");
        if buffer.len() < (fragment.total_n_fragments * 128) as usize {
            buffer.resize((fragment.total_n_fragments * 128) as usize, 0);
        }
        // Copy the fragment's data into the correct position in the buffer
        let start = (fragment.fragment_index * 128) as usize;
        let end = start + fragment.length as usize;
        buffer[start..end].copy_from_slice(&fragment.data[..fragment.length as usize]);

        // Update count of received fragments
        *self.processed_fragments.entry(key).or_insert(0) += 1;
        if self.processed_fragments[&key] == fragment.total_n_fragments as u8 {
            // Reassemble the message
            let total_length = ((fragment.total_n_fragments - 1) * 128 + fragment.length as u64) as usize;
            let message = buffer[..total_length].to_vec();

            // Clean up tracking structures
            self.buffer.remove(&key);
            self.processed_fragments.remove(&key);

            // Return reassembled message
            //println!("{:?}",message);
            Some(message)
        } else {
            // If not all fragments received, return Error
            None
        }
    }
    // Given a &str create the fragments from it
    pub fn generate_fragments(str: &str) -> Result<Vec<Fragment>, String> {
        // Convert the initial string to bytes
        //println!("{}",str);
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
                total_n_fragments,
                length: slice.len() as u8,
                data,
            });
        }
        Ok(fragments)
    }
    // Assemble the Vec and save the result
    pub fn assemble_string_file(data: Vec<u8>, mut received_files: &mut Vec<String>) -> Result<String, String> {
        match String::from_utf8(data) {
            Ok(mut string) => {
                // Remove trailing null characters
                string = string.trim_end_matches('\0').to_string();
                Ok(string)
            }
            Err(e) => Err(format!("Failed to convert data to string: {}", e)),
        }
    }
}
//Some tests about different files fragmented and reconstructed
#[cfg(test)]
mod test{
    use super::*;
    #[test]
    fn test_fragment_string_assembled_correctly(){
        let (_,rcv) = unbounded::<Packet>();
        let mut client_test = Client1::new(1, HashMap::new(), rcv);
        let mut fr = FragmentReassembler::new();
        let test_data = &"A".repeat(200);
        let test_result = FragmentReassembler::generate_fragments(test_data);
        let mut fragm_vec = Ok(Some(vec![]));
        for e in test_result.unwrap().iter(){
            fragm_vec = fr.add_fragment(1,1,e.clone());
        }
        let res = FragmentReassembler::assemble_string_file(fragm_vec.unwrap().unwrap(),&mut client_test.received_files);
        assert!(test_data.eq(&res.unwrap()));
    }
    #[test]
    fn test_fragment_txt_assembled_correctly(){
        let (_,rcv) = unbounded::<Packet>();
        let mut client_test = Client1::new(1, HashMap::new(), rcv);
        let test_text_content = fs::read("src/test/file1");
        let test_result = FragmentReassembler::assemble_string_file(test_text_content.unwrap(),&mut client_test.received_files);
        assert_eq!(test_result.unwrap(),"test 123456 advanced_programming");
        //need to check where the file is written lol
    }
    #[test]
    fn test_fragment_mediaFile_assembled_correctly(){
        let (_,rcv) = unbounded::<Packet>();
        let mut client_test = Client1::new(1, HashMap::new(), rcv);
        let test_text_content = fs::read("src/test/testMedia.mp3");
        let test_result = FragmentReassembler::assemble_string_file(test_text_content.unwrap(),&mut client_test.received_files);
        match test_result{
            Ok(_) => (),
            Err(_) => panic!("Error")
        }
    }
}





