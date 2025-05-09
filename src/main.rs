use core::time;
use std::{env, process::Command, sync::Arc, thread};
use udev::{Enumerator, MonitorBuilder};

#[derive(Debug)]
pub enum Errors {
    UdevSubsystem,
    UdevDeviceScan,
    UdevError,
    UdevMonitor,

    EvdevOpen,
    EvdevFetch(String),

    NotController,
    NoDevicePath,
    InvalidParams,
}

impl std::fmt::Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Errors::UdevSubsystem => write!(f, "Failed to filter devices by input subsystem."),
            Errors::UdevDeviceScan => write!(f, "Failed to scan for devices."),
            Errors::UdevError => write!(f, "Failed to udev."),
            Errors::UdevMonitor => write!(f, "Failed to monitor for new devices."),
            Errors::EvdevOpen => write!(f, "Failed to open device."),
            Errors::EvdevFetch(e) => write!(f, "Failed to fetch device events: '{e}'."),
            Errors::NotController => write!(f, "This device is not a controller."),
            Errors::NoDevicePath => write!(f, "This device does not have a path? Wtf how?"),
            Errors::InvalidParams => write!(
                f,
                "Invalid parameters. Please provide a command to execute once the home button is pressed."
            ),
        }
    }
}

fn main() -> Result<(), Errors> {
    let args: Arc<Vec<String>> = Arc::new(env::args().collect::<Vec<String>>()[1..].to_vec());
    if args.len() == 0 {
        return Err(Errors::InvalidParams);
    }

    let mut enumerator = Enumerator::new().map_err(|_| Errors::UdevError)?;
    enumerator
        .match_subsystem("input")
        .map_err(|_| Errors::UdevSubsystem)?;
    let devices = enumerator
        .scan_devices()
        .map_err(|_| Errors::UdevDeviceScan)?;
    for device in devices {
        let _ = verify_device(device, &args);
    }

    let monitor = MonitorBuilder::new()
        .and_then(|v| v.match_subsystem("input"))
        .and_then(|v| v.listen())
        .map_err(|_| Errors::UdevMonitor)?;
    let mut monitor = monitor.iter();
    loop {
        while let Some(event) = monitor.next() {
            if event.event_type() != udev::EventType::Add {
                continue;
            }
            println!("{} CONNECTED", event.sysname().to_str().unwrap());
            let _ = verify_device(event.device(), &args);
        }
        thread::sleep(time::Duration::from_secs(1));
    }
}

fn verify_device(device: udev::Device, args: &Arc<Vec<String>>) -> Result<(), Errors> {
    device
        .properties()
        .find(|v| v.name() == "ID_INPUT_JOYSTICK" && v.value() == "1")
        .ok_or(Errors::NotController)?;
    let devnode = device
        .devnode()
        .map(|v| v.to_string_lossy().to_string())
        .ok_or(Errors::NoDevicePath)?;
    println!("Device found: {}", devnode);

    let args = args.clone();
    thread::spawn(move || {
        let _ = listen_for_key(&devnode, args).map_err(|e| eprintln!("{e}"));
    });

    Ok(())
}

fn listen_for_key(device_path: &str, args: Arc<Vec<String>>) -> Result<(), Errors> {
    let mut device = evdev::Device::open(device_path).map_err(|_| Errors::EvdevOpen)?;
    let name = &device.name().unwrap_or("Nameless device").to_string();

    loop {
        let fetch_events = device
            .fetch_events()
            .map_err(|_| Errors::EvdevFetch(name.clone()))?;

        for event in fetch_events {
            let key = event.code();
            if event.event_type().0 != 1 // EventType::KEY
                || event.value() != 0
                || (key != 316 && key != 139)
            {
                continue;
            }

            println!("Pressed: {}", name);
            let _ = Command::new(&args[0])
                .args(&args[1..])
                .spawn()
                .map_err(|e| eprintln!("Error running command: {e}"));
        }

        thread::sleep(time::Duration::from_millis(250));
    }
}
