use std::{collections::HashMap, error::Error, fs::File, io::Read, sync::{Arc, Mutex, mpsc::{channel, Sender}}, thread::JoinHandle, time::{SystemTime, UNIX_EPOCH}};
use enocean::{packet::{Address, Packet, RadioErp1}, port::Port, enocean::Rorg};
use toml::Value;


type DeviceName = String;
type Temperature = f64;
type Timestamp = SystemTime;

#[derive(Debug,Default)]
struct TemperatureStore {
    devices: HashMap<Address, (Option<DeviceName>, Option<(Temperature, Timestamp)>)>,
}

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

impl TemperatureStore {
    pub fn new() -> Self { Self::default() }

    pub fn with_devices(config_devices: &toml::value::Table) -> Result<Self> {
        let mut devices = HashMap::new();
        for (address, name) in config_devices.iter() {
            let name = name.as_str().ok_or("device name was not string")?.to_owned();            
            let address = address.parse()?;
            devices.insert(address, (Some(name), None));
        }
        Ok(Self { devices })
    }

    pub fn insert(&mut self, address: Address, temperature: Temperature, timestamp: SystemTime) {
        self.devices.entry(address)
            .or_insert((None,None))
            .1.replace((temperature, timestamp));
    }

    pub fn scrape(&mut self) -> String {
        let mut scrape = format!("# HELP enocean_temperature_celsius Temperature reported by an EnOcean sensor, in°C\n");
        scrape += &format!("# TYPE enocean_temperature_celsius gauge\n");

        for (address, (name, point)) in self.devices.iter() {
            if let Some((temp, time)) = point {
                let time = time.duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis();
                let address = address.to_string();
                scrape += &
                    if let Some(name) = name {
                        format!("enocean_temperature_celsius{{address=\"{address}\", name=\"{name}\"}} {temp} {time}")
                    } else {
                        format!("enocean_temperature_celsius{{address=\"{address}\"}} {temp} {time}")
                    }
            }
        }

        scrape
    }

}

fn main() -> Result<()> {
    let mut config_file = String::new();
    File::open("temperature_exporter.toml")?.read_to_string(&mut config_file)?;
    let config: Value = config_file.parse()?;
    let config = config.as_table().ok_or("config wasn't a table")?;

    let port_name = config["port"].as_str().ok_or("port name not found in config")?;
    let listen = config["listen"].as_str().ok_or("listen was not a string")?;
    let devices = config["devices"].as_table().ok_or("devices is not a table")?;

    let store = TemperatureStore::with_devices(devices)?;
    let store = Arc::new(Mutex::new(store));
    
    let port = Port::open(port_name)?;

    let driver_store = store.clone();
    let driver = std::thread::spawn(move || { serial_driver_thread(port, driver_store)});

    let mut tick = String::new();

    loop {
        std::io::stdin().read_line(&mut tick)?;
        println!("{}", store.lock().unwrap().scrape());
    }

    Ok(())
}

fn serial_driver_thread(mut port: Port, store: Arc<Mutex<TemperatureStore>>) {
    loop {
        let Ok(frame) = port.read_frame() else { continue };
        eprintln!("Frame: {frame:?}");

        let Ok(pkt) = Packet::decode(frame.as_ref())
            .map_err(|e| eprintln!("Cannot decode: {e}")) else { continue };

        if let Packet::RadioErp1(erp) = pkt {
            if erp.choice == Rorg::Bs4 {
                let temperature = decode_temperature(erp.user_data[2]);
                let address = erp.sender_id;
                let timestamp = SystemTime::now();
                store.lock().unwrap().insert(address, temperature, timestamp);
            }
        }
    }
}

fn decode_temperature(byte: u8) -> f64 {
    40f64 - (byte as f64 * 40f64 / 255f64)
}