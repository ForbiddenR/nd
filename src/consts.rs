pub const SIDEBAR_ITEMS: [&str; 6] = [
    "Containers",
    "Images",
    "Build",
    "Build Status",
    "Push Status",
    "Logs",
];

pub const HELP_CONTAINERS: &str = "q quit | r refresh | tab switch | ↑/↓ select | s start | x stop | R restart | d remove | p prune";
pub const HELP_IMAGES: &str =
    "q quit | r refresh | tab switch | ↑/↓ select | d remove | p prune | s push";
pub const HELP_BUILD: &str = "q quit | r refresh | tab switch | type path | enter build | backspace delete | ctrl-u clear | p prune builder";
pub const HELP_BUILD_STATUS: &str =
    "q quit | tab switch | c clear finished build output | esc cancel running build";
pub const HELP_PUSH_STATUS: &str =
    "q quit | tab switch | c clear finished push output | esc cancel running push";
pub const HELP_LOGS: &str = "q quit | r refresh | tab switch | c clear logs";
pub const HELP_MODAL: &str = "y confirm | n cancel | esc cancel";
