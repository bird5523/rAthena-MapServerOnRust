use tracing::{info, warn};

#[derive(Debug, Clone)]
pub enum ScriptCommand {
    Mes(String),
    Next,
    Close,
    Menu(Vec<(String, usize)>), // label, target_line
    Jump(usize),
    Warp(String, u16, u16),
    Heal(u32, u32),
    GetItem(i32, u16),
    DelItem(i32, u16),
    SetZeny(i32),
}

#[derive(Debug, Clone)]
pub struct NpcScript {
    pub name: String,
    pub sprite_id: u16,
    pub commands: Vec<ScriptCommand>,
}

#[derive(Debug, Clone)]
pub struct ScriptContext {
    pub npc_id: u32,
    pub char_id: u32,
    pub current_line: usize,
    pub script: NpcScript,
}

impl ScriptContext {
    pub fn new(char_id: u32, npc_id: u32, script: NpcScript) -> Self {
        Self {
            char_id,
            npc_id,
            current_line: 0,
            script,
        }
    }

    /// Advances the script until it needs to wait for client input (Next/Close/Menu)
    /// Returns the list of packets to send to the client.
    pub fn run_until_yield(&mut self) -> Vec<ScriptCommand> {
        let mut outputs = Vec::new();

        while self.current_line < self.script.commands.len() {
            let cmd = &self.script.commands[self.current_line];
            self.current_line += 1;

            match cmd {
                ScriptCommand::Mes(text) => {
                    outputs.push(ScriptCommand::Mes(text.clone()));
                }
                ScriptCommand::Next => {
                    outputs.push(ScriptCommand::Next);
                    break; // Yield for user input
                }
                ScriptCommand::Close => {
                    outputs.push(ScriptCommand::Close);
                    break; // End of interaction
                }
                ScriptCommand::Menu(options) => {
                    outputs.push(ScriptCommand::Menu(options.clone()));
                    break; // Yield for user input
                }
                ScriptCommand::Jump(line) => {
                    self.current_line = *line;
                }
                ScriptCommand::Warp(map, x, y) => {
                    outputs.push(ScriptCommand::Warp(map.clone(), *x, *y));
                    break; // Warp ends interaction typically
                }
                ScriptCommand::Heal(hp, sp) => {
                    outputs.push(ScriptCommand::Heal(*hp, *sp));
                }
                ScriptCommand::GetItem(item, amount) => {
                    outputs.push(ScriptCommand::GetItem(*item, *amount));
                }
                ScriptCommand::DelItem(item, amount) => {
                    outputs.push(ScriptCommand::DelItem(*item, *amount));
                }
                ScriptCommand::SetZeny(amount) => {
                    outputs.push(ScriptCommand::SetZeny(*amount));
                }
            }
        }

        outputs
    }
}
