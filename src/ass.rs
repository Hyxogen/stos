use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssError {
    UnbalancedBrackets,
    NotEnoughParts,
}

impl fmt::Display for AssError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssError::UnbalancedBrackets => write!(
                f,
                "The ass event contained style brackets that were not opened/closed"
            ),
            AssError::NotEnoughParts => {
                write!(f, "The ass event did not contain all the required fields")
            }
        }
    }
}

impl std::error::Error for AssError {}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssText {
    pub text: String,
    pub dialogue: String,
}

impl FromStr for AssText {
    type Err = AssError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        let mut escaped = false;
        let mut brackets: u64 = 0;
        let mut dialogue = String::new();

        while let Some(ch) = chars.next() {
            if ch == '{' {
                brackets += 1;
            } else if ch == '}' {
                if brackets > 0 {
                    brackets -= 1;
                } else {
                    return Err(AssError::UnbalancedBrackets);
                }
            } else if brackets == 0 {
                if escaped {
                    if ch == 'n' {
                        dialogue.push('n');
                    } else {
                        dialogue.push('\\');
                        dialogue.push(ch);
                    }
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else {
                    dialogue.push(ch);
                }
            }
        }
        Ok(Self {
            text: s.to_string(),
            dialogue,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogueEvent {
    pub name: String,
    pub text: AssText,
}

impl FromStr for DialogueEvent {
    type Err = AssError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(9, ',').skip(3);

        let name = parts.next().ok_or(AssError::NotEnoughParts)?.to_string();
        let text = parts
            .skip(4)
            .next()
            .ok_or(AssError::NotEnoughParts)?
            .parse()?;
        Ok(Self { name, text })
    }
}

impl TryFrom<libav::subtitle::Ass<'_>> for DialogueEvent {
    type Error = <DialogueEvent as std::str::FromStr>::Err;

    fn try_from(ass: libav::subtitle::Ass<'_>) -> Result<Self, Self::Error> {
        ass.get().parse()
    }
}
