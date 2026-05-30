use crate::prelude::*;

/// HTML checkbox shim: browsers only POST the field when checked, so a missing
/// value means "off". Wrap `Option<String>` so handlers can treat the absence
/// as a real boolean rather than a parse error.
#[derive(Default, Deserialize, Serialize, Clone)]
pub struct Toggle(Option<String>);

impl Toggle {
    pub fn new(toggle: Option<String>) -> Self {
        Self(toggle)
    }

    pub fn value(&self) -> Option<String> {
        self.0.clone()
    }

    pub fn as_bool(&self) -> bool {
        self.0.is_some()
    }
}

impl Display for Toggle {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if bool::from(self) {
            return write!(f, "true");
        }
        write!(f, "false")
    }
}

impl From<&Toggle> for bool {
    fn from(toggle: &Toggle) -> Self {
        toggle.value().is_some()
    }
}
