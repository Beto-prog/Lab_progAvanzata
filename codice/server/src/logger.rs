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
                    .open("log.txt")
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

}