use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use zbus::interface;
use zbus::zvariant::ObjectPath;

/// The code to show while a pairing that Octessera started is in progress.
#[derive(Clone, Default)]
pub(super) struct AgentState(Arc<Mutex<PairingCode>>);

#[derive(Default)]
struct PairingCode {
    active: bool,
    code: Option<String>,
}

impl AgentState {
    pub(super) fn begin(&self) {
        self.update(|state| {
            state.active = true;
            state.code = None;
        });
    }

    pub(super) fn finish(&self) {
        self.update(|state| *state = PairingCode::default());
    }

    pub(super) fn code(&self) -> Option<String> {
        self.0.lock().ok().and_then(|state| state.code.clone())
    }

    /// Records the code if our own pairing is running; incoming pairing
    /// attempts are rejected.
    fn show(&self, code: String) -> Result<(), AgentError> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| AgentError::Rejected("agent state unavailable".into()))?;
        if !state.active {
            return Err(AgentError::Rejected("no pairing in progress".into()));
        }
        state.code = Some(code);
        Ok(())
    }

    fn update(&self, change: impl FnOnce(&mut PairingCode)) {
        if let Ok(mut state) = self.0.lock() {
            change(&mut state);
        }
    }
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum AgentError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Rejected(String),
}

pub(super) struct Agent {
    state: AgentState,
}

impl Agent {
    pub(super) fn new(state: AgentState) -> Self {
        Self { state }
    }
}

fn random_pin() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.subsec_nanos())
        .unwrap_or_default();
    format!("{:06}", nanos % 1_000_000)
}

#[interface(name = "org.bluez.Agent1")]
impl Agent {
    fn release(&self) {}

    fn request_pin_code(&self, _device: ObjectPath<'_>) -> Result<String, AgentError> {
        let pin = random_pin();
        self.state.show(pin.clone())?;
        Ok(pin)
    }

    fn display_pin_code(&self, _device: ObjectPath<'_>, pincode: String) -> Result<(), AgentError> {
        self.state.show(pincode)
    }

    fn request_passkey(&self, _device: ObjectPath<'_>) -> Result<u32, AgentError> {
        Err(AgentError::Rejected("no keypad to enter a passkey".into()))
    }

    fn display_passkey(
        &self,
        _device: ObjectPath<'_>,
        passkey: u32,
        _entered: u16,
    ) -> Result<(), AgentError> {
        self.state.show(format!("{passkey:06}"))
    }

    fn request_confirmation(
        &self,
        _device: ObjectPath<'_>,
        passkey: u32,
    ) -> Result<(), AgentError> {
        self.state.show(format!("{passkey:06}"))
    }

    fn request_authorization(&self, _device: ObjectPath<'_>) -> Result<(), AgentError> {
        Err(AgentError::Rejected(
            "incoming pairing is not accepted".into(),
        ))
    }

    fn authorize_service(&self, _device: ObjectPath<'_>, _uuid: String) -> Result<(), AgentError> {
        Err(AgentError::Rejected("untrusted device".into()))
    }

    fn cancel(&self) {
        self.state.update(|state| state.code = None);
    }
}
