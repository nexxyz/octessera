use super::agent::{Agent, AgentState};
use super::{Bluez, DeviceRecord, WorkerEvent};
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

const BLUEZ: &str = "org.bluez";
const ADAPTER_PATH: &str = "/org/bluez/hci0";
const ADAPTER_IFACE: &str = "org.bluez.Adapter1";
const DEVICE_IFACE: &str = "org.bluez.Device1";
const AGENT_PATH: &str = "/org/octessera/bluetooth/agent";

type Properties = HashMap<String, OwnedValue>;
type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, Properties>>;

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

pub(super) struct SystemBluez {
    connection: Connection,
    agent: AgentState,
}

impl SystemBluez {
    pub(super) fn connect() -> Result<Self, String> {
        let connection = Connection::system().map_err(error)?;
        let agent = AgentState::default();
        connection
            .object_server()
            .at(AGENT_PATH, Agent::new(agent.clone()))
            .map_err(error)?;
        let manager = Proxy::new(&connection, BLUEZ, "/org/bluez", "org.bluez.AgentManager1")
            .map_err(error)?;
        let path = ObjectPath::try_from(AGENT_PATH).map_err(error)?;
        manager
            .call_method("RegisterAgent", &(&path, "KeyboardDisplay"))
            .map_err(error)?;
        manager
            .call_method("RequestDefaultAgent", &(&path,))
            .map_err(error)?;
        Ok(Self { connection, agent })
    }

    fn adapter(&self) -> Result<Proxy<'static>, String> {
        Proxy::new(&self.connection, BLUEZ, ADAPTER_PATH, ADAPTER_IFACE).map_err(error)
    }
}

fn device_path(address: &str) -> String {
    format!("{ADAPTER_PATH}/dev_{}", address.replace(':', "_"))
}

fn device(connection: &Connection, address: &str) -> Result<Proxy<'static>, String> {
    Proxy::new(connection, BLUEZ, device_path(address), DEVICE_IFACE).map_err(error)
}

fn connect_device(connection: &Connection, address: &str) -> Result<(), String> {
    device(connection, address)?
        .call_method("Connect", &())
        .map(|_| ())
        .map_err(error)
}

fn pair_device(connection: &Connection, address: &str) -> Result<(), String> {
    let device = device(connection, address)?;
    if let Err(failure) = device.call_method("Pair", &()) {
        if !failure.to_string().contains("AlreadyExists") {
            return Err(error(failure));
        }
    }
    device.set_property("Trusted", true).map_err(error)
}

fn text(properties: &Properties, key: &str) -> Option<String> {
    properties
        .get(key)
        .and_then(|value| <&str>::try_from(&**value).ok())
        .map(str::to_owned)
}

fn flag(properties: &Properties, key: &str) -> bool {
    properties
        .get(key)
        .and_then(|value| bool::try_from(&**value).ok())
        .unwrap_or(false)
}

fn uuids(properties: &Properties) -> Vec<String> {
    match properties.get("UUIDs").map(|value| &**value) {
        Some(Value::Array(array)) => array
            .iter()
            .filter_map(|uuid| <&str>::try_from(uuid).ok().map(str::to_owned))
            .collect(),
        _ => Vec::new(),
    }
}

fn record(properties: &Properties) -> Option<DeviceRecord> {
    Some(DeviceRecord {
        address: text(properties, "Address")?,
        name: text(properties, "Name"),
        icon: text(properties, "Icon"),
        class: properties
            .get("Class")
            .and_then(|value| u32::try_from(&**value).ok()),
        uuids: uuids(properties),
        paired: flag(properties, "Paired"),
        connected: flag(properties, "Connected"),
    })
}

impl Bluez for SystemBluez {
    fn set_powered(&mut self, on: bool) -> Result<(), String> {
        self.adapter()?.set_property("Powered", on).map_err(error)
    }

    fn powered(&mut self) -> Result<bool, String> {
        self.adapter()?.get_property("Powered").map_err(error)
    }

    fn set_discovering(&mut self, on: bool) -> Result<(), String> {
        let method = if on {
            "StartDiscovery"
        } else {
            "StopDiscovery"
        };
        self.adapter()?
            .call_method(method, &())
            .map(|_| ())
            .map_err(error)
    }

    fn devices(&mut self) -> Result<Vec<DeviceRecord>, String> {
        let manager = Proxy::new(
            &self.connection,
            BLUEZ,
            "/",
            "org.freedesktop.DBus.ObjectManager",
        )
        .map_err(error)?;
        let objects: ManagedObjects = manager.call("GetManagedObjects", &()).map_err(error)?;
        let prefix = format!("{ADAPTER_PATH}/dev_");
        Ok(objects
            .iter()
            .filter(|(path, _)| path.as_str().starts_with(&prefix))
            .filter_map(|(_, interfaces)| record(interfaces.get(DEVICE_IFACE)?))
            .collect())
    }

    fn disconnect(&mut self, address: &str) -> Result<(), String> {
        device(&self.connection, address)?
            .call_method("Disconnect", &())
            .map(|_| ())
            .map_err(error)
    }

    fn forget(&mut self, address: &str) -> Result<(), String> {
        let path = ObjectPath::try_from(device_path(address)).map_err(error)?;
        self.adapter()?
            .call_method("RemoveDevice", &(&path,))
            .map(|_| ())
            .map_err(error)
    }

    fn cancel_pairing(&mut self, address: &str) -> Result<(), String> {
        self.agent.finish();
        device(&self.connection, address)?
            .call_method("CancelPairing", &())
            .map(|_| ())
            .map_err(error)
    }

    fn spawn_pair(&mut self, address: &str, done: Sender<WorkerEvent>) {
        self.agent.begin();
        let connection = self.connection.clone();
        let agent = self.agent.clone();
        let address = address.to_string();
        std::thread::spawn(move || {
            let result = pair_device(&connection, &address);
            agent.finish();
            let paired = result.is_ok();
            let _ = done.send(WorkerEvent::PairDone {
                address: address.clone(),
                result,
            });
            if paired {
                let result = connect_device(&connection, &address);
                let _ = done.send(WorkerEvent::ConnectDone { address, result });
            }
        });
    }

    fn spawn_connect(&mut self, address: &str, done: Sender<WorkerEvent>) {
        let connection = self.connection.clone();
        let address = address.to_string();
        std::thread::spawn(move || {
            let result = connect_device(&connection, &address);
            let _ = done.send(WorkerEvent::ConnectDone { address, result });
        });
    }

    fn pairing_code(&mut self) -> Option<String> {
        self.agent.code()
    }
}
