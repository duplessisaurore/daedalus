//! These are all the possible errors that
//! can occur during the running of the `Daedalus`
//! capabilities.

use core::fmt::Display;

use alloc::string::String;

#[derive(Debug)]
pub enum DaedalusCapErrors {
    /// Attempted to look up a program under this name,
    /// but none could be found!
    CouldNotFindProgram { looked_up_program_name: String },

    /// A program name was expected here
    /// as per the capability, but a valid
    /// one could not be found
    ProgramNameExpected,

    /// The next phase is `end`
    ///
    /// This will be handled by finishing the boot process
    /// and jumping to the entry point from the final program,
    /// see `finish`.
    EndOfPhases,

    /// The scheduler had nothing runnable that could be found
    /// when trying to run something next...
    ///
    /// This should only happen when everything is blocked in a
    /// deadlock!
    NothingToRunDeadLock,
}

impl Display for DaedalusCapErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EndOfPhases => {
                write!(f, "daedalus reached the `end` of the boot phases!")
            }

            Self::CouldNotFindProgram {
                looked_up_program_name,
            } => {
                write!(
                    f,
                    "daedalus capability tried to look up program `{looked_up_program_name}`, but could not find any program with that name!"
                )
            }
            Self::ProgramNameExpected => write!(
                f,
                "daedalus capability expected a program name as a value, found an invalid one!"
            ),
            Self::NothingToRunDeadLock => {
                write!(
                    f,
                    "daedalus scheduler could find no runnable program, every program must be blocked in a deadlock!"
                )
            }
        }
    }
}
