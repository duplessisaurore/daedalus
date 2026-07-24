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
}

impl Display for DaedalusCapErrors {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::CouldNotFindProgram {
                looked_up_program_name,
            } => {
                write!(
                    f,
                    "daedalus capability tried to look up program {looked_up_program_name}, but could not find any program with that name!"
                )
            }
            Self::ProgramNameExpected => write!(
                f,
                "daedalus capability expected a program name as a value, found an invalid one!"
            ),
        }
    }
}
