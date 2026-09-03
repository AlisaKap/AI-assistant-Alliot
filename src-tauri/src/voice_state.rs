use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceState {
    Off,
    WaitingForWakeWord,
    Listening,
    Analyzing,
    Error,
}

#[derive(Debug)]
pub struct VoiceStateManager {
    state: Mutex<VoiceState>,
}

impl VoiceStateManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(VoiceState::Off),
        }
    }

    pub fn get(&self) -> VoiceState {
        *self.state.lock().unwrap()
    }

    pub fn set(&self, state: VoiceState) {
        *self.state.lock().unwrap() = state;
    }
}