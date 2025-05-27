pub mod logger
{
    use std::fs::{OpenOptions};
    use std::io::Write;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    
    // Mutex globale per accesso sicuro da thread multipli
    static mut LOGGER: Option<Mutex<()>> = None;

    /// Inizializza il logger
    pub fn init_logger() {
        unsafe {
            if LOGGER.is_none() {
                LOGGER = Some(Mutex::new(()));
            }
        }
    }

    /// Scrive un messaggio nel file log.txt
    pub fn write_log(message: &str) {
        unsafe {
            if let Some(lock) = &LOGGER {
                let _guard = lock.lock().unwrap();

                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open("log_Server.log")
                    .unwrap();

                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                writeln!(file, "[{}] {}", timestamp, message).unwrap();
            } else {
                panic!("Logger non inizializzato. Chiama `init_logger()` prima di usare `write_log()`.");
            }
        }
    }
/*

    pub fn write_log_from_bytes(data: &[u8]) {
        unsafe {
            if let Some(lock) = &LOGGER {
                let _guard = lock.lock().unwrap();

                // Trova la fine utile (prima di padding con 0)
                let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                let valid_slice = &data[..end];

                let decoded: String = match str::from_utf8(valid_slice) {
                    Ok(text) => text,
                    Err(_) => "<invalid UTF-8>",
                };

                let mut file = OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open("log_Server.log")
                    .unwrap();

                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                writeln!(file, "[{}] {}", timestamp, decoded).unwrap();
            } else {
                panic!("Logger non inizializzato. Chiama `init_logger()` prima di usare `write_log_from_bytes()`.");
            }
        }
    }
    */
    
    

}