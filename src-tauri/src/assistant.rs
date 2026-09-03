use crate::command_parser;

#[derive(Debug)]
pub enum AssistantAction {
    Respond(String),
    OpenApplication(String),
    OpenWorkspace,
    Unknown(String),
}

pub fn process_command(text: &str) -> AssistantAction {
    command_parser::parse_command(text)
}