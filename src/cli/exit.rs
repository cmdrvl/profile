#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Success,
    IssuesFound,
    Refusal,
}

impl Outcome {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::Success => EXIT_SUCCESS,
            Self::IssuesFound => EXIT_ISSUES_FOUND,
            Self::Refusal => EXIT_REFUSAL,
        }
    }
}

pub const EXIT_SUCCESS: u8 = 0;
pub const EXIT_ISSUES_FOUND: u8 = 1;
pub const EXIT_REFUSAL: u8 = 2;

pub const SUCCESS: u8 = EXIT_SUCCESS;
pub const ISSUES_FOUND: u8 = EXIT_ISSUES_FOUND;
pub const REFUSAL: u8 = EXIT_REFUSAL;
