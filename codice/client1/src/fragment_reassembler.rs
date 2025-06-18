use base64::{engine::general_purpose, Engine as _};
use std::collections::{HashMap};
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use crossbeam_channel::{unbounded};
use wg_2024::network::NodeId;
use wg_2024::packet::{Fragment, Packet};
use crate::Client1;

// Struct FragmentReassembler to manage reassembly of fragments
pub struct FragmentReassembler {
    pub buffer: HashMap<(u64, NodeId), Vec<u8>>, // Map of (session_id, source_id) to message buffer
    pub processed_fragments: HashMap<(u64, NodeId), u64>, // Number of received fragments per (session_id, source_id)
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
    pub fn add_fragment(&mut self, session_id: u64, source_id: NodeId, fragment: Fragment) -> Result<Option<Vec<u8>>,String> {
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
        if self.processed_fragments.get(&key).expect("Failed to get value").eq(&(fragment.total_n_fragments)) {
            // Reassemble the message
            let message = self.buffer.remove(&key).unwrap_or_default();

            //Clean tracking structures
            self.processed_fragments.remove(&key);

            //let total_length = ((fragment.total_n_fragments - 1) * 128 + fragment.length as u64) as usize;
            //let message = buffer[..total_length].to_vec();
            // Return reassembled message
            //println!("{:?}",message);
            Ok(Some(message))
        } else {
            // If not all fragments received, return Error
            Ok(None)
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
    pub fn assemble_file(data: Vec<u8>, output_path: &str) -> Result<String, String> {          //Attenzione gli mp3 non hanno problemi ma le immagini e i video sono molto sensibil. Basta un byte fuori posto e non va piu 
        // 1. dopo la prima '('
        let after_paren = data.iter()
            .position(|&b| b == b'(')
            .ok_or("Not a valid format: '(' missing")? + 1;

        // 2. se esiste una virgola subito dopo (caso file!(size, ...)), non
        //    salta anche <size>, fino alla prima ','
        let payload_start = match data[after_paren..].iter().position(|&b| b == b',') {
            Some(idx) => after_paren + idx + 1, // byte dopo la virgola
            None      => after_paren,           // messaggio media!(...) ‑ niente size
        };

        // 3. ultima ')' (esclude pure il padding 0x00 finale) !!IMPORTANTE
        let mut payload_end = data.iter()
            .rposition(|&b| b == b')')
            .ok_or("Not a valid format: ')' missing")?;

        while payload_end > payload_start && data[payload_end] == 0 {
            payload_end -= 1;        // togli eventuali 0x00 dopo la ')'
        }

        let payload = &data[payload_start..payload_end]; // ')' esclusa

        let mut file = File::create(Path::new(output_path))
            .map_err(|e| format!("Error while creating file: {e}"))?;
        file.write_all(payload)
            .map_err(|e| format!("Error while writing file: {e}"))?;

        Ok("File saved correctly".into())
    }

}
//Some tests about different files fragmented and reconstructed
#[cfg(test)]
mod test{
    use super::*;
    #[test]
    fn test_fragment_string_assembled_correctly(){
        let (_,rcv) = unbounded::<Packet>();
        let (_,rcv_id) = unbounded::<NodeId>();
        let mut client_test = Client1::new(1, HashMap::new(), rcv,rcv_id);
        let mut fr = FragmentReassembler::new();
        let test_data = &"A".repeat(200);
        let test_result = FragmentReassembler::generate_fragments(test_data);
        let mut fragm_vec = Ok(Some(vec![]));
        for e in test_result.unwrap().iter(){
            fragm_vec = fr.add_fragment(1,1,e.clone());
        }
        let res = FragmentReassembler::assemble_string_file(fragm_vec.unwrap().unwrap());
        assert!(test_data.eq(&res.unwrap()));
    }
    #[test]
    fn test_fragment_txt_assembled_correctly(){
        let (_,rcv_id) = unbounded::<NodeId>();
        let (_,rcv) = unbounded::<Packet>();
        let mut client_test = Client1::new(1, HashMap::new(), rcv,rcv_id);
        let test_text_content = fs::read("src/test/file1");
        let test_result = FragmentReassembler::assemble_string_file(test_text_content.unwrap());
        assert_eq!(test_result.unwrap(),"test 123456 advanced_programming");
        //need to check where the file is written lol
    }
    #[test]
    fn test_fragment_mediaFile_assembled_correctly(){
        let (_,rcv_id) = unbounded::<NodeId>();
        let (_,rcv) = unbounded::<Packet>();
        let mut client_test = Client1::new(1, HashMap::new(), rcv,rcv_id);
        let test_text_content = fs::read("src/test/testMedia.mp3");
        let test_result = FragmentReassembler::assemble_string_file(test_text_content.unwrap());
        match test_result{
            Ok(_) => (),
            Err(_) => panic!("Error")
        }
    }
}





