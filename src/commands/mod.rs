pub mod completion;
pub mod init;
pub mod list;
pub mod merge;
pub mod worktree;

pub use completion::{Shell, handle_complete, handle_completion};
pub use init::handle_init;
pub use list::handle_list;
pub use merge::handle_merge;
pub use worktree::{handle_push, handle_remove, handle_switch};
