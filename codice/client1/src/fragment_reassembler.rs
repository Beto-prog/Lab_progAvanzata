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
    pub fn assemble_file(data: Vec<u8>, output_path: &str) -> Result<String, String> {
        /*
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
            Ok("Succesful conversion".to_string())
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
         */
        let start = data.iter().position(|&b| b == b'(').map(|p| p + 1).unwrap_or(0);
        //let end = data.iter().position(|&b| b == b')').map(|p| p + 1).unwrap_or(data.len());
        let mut file = File::create(Path::new(output_path)).expect("Failed to create file");
        file.write_all(&data[start..]).map_err(|e| format!("Error during file writing: {e}"))?;
        Ok("Message successfully converted".to_string())
        /*
        // Cerca il primo '('
        let payload_start = data.iter().position(|&b| b == b'(').map(|p| p + 1).unwrap_or(0);
        let file_data = &data[payload_start..];

        fs::write_all(output_path, file_data)
            .map_err(|e| format!("Errore nella scrittura del file: {e}"))?;

         */
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





