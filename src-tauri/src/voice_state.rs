#[derive(Debug, Clone, Copy)]
pub enum VoiceState {
    Off,
    WaitingForWakeWord,
    Listening,
    Analyzing,
    Error,
}