use core::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WmProcessError {
    message: String,
}

impl WmProcessError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for WmProcessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for WmProcessError {}
