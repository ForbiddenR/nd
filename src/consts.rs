pub const SIDEBAR_ITEMS: [&str; 5] = ["Containers", "Images", "Build", "Tasks", "Logs"];

pub const HELP_CONTAINERS: &str = "q quit | r refresh | tab switch | ↑/↓ select | s start | x stop | R restart | d remove | p prune";
pub const HELP_IMAGES: &str =
    "q quit | r refresh | tab switch | ↑/↓ select | d remove | p prune | s push";
pub const HELP_BUILD: &str = "q quit | r refresh | tab switch | type path | enter build | backspace delete | ctrl-u clear | p prune builder";
pub const HELP_TASKS: &str = "q quit | tab switch | ↑/↓ select task | d remove task | c clear finished output | esc cancel running task";
pub const HELP_LOGS: &str = "q quit | r refresh | tab switch | c clear logs";
pub const HELP_MODAL: &str = "y confirm | n cancel | esc cancel";
