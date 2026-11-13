//! The failure codes of the `jumpgate` command. Each code has the form
//! `jumpgate::<area>::<name>`, where the area is a noun from jumpgate's
//! domain (CLI-3). The library's record codes, `jumpgate::record::*`, reach
//! the command once a command reads records.

use crumbkit::Code;

/// The command exists in the command tree, and this version of jumpgate
/// does not do its work yet.
pub const COMMAND_NOT_BUILT: Code = Code::new("jumpgate", "command", "not_built");

/// Every code above.
pub const ALL: &[&Code] = &[&COMMAND_NOT_BUILT];
