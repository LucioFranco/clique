use std::{error, fmt};

pub type Result<T> = std::result::Result<T, Error>;

type Source = Box<dyn error::Error + Send + Sync + 'static>;

pub struct Error {
    kind: ErrorKind,
    source: Option<Source>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ErrorKind {
    Start,
    Join,
    BrokenPipe,

    UuidAlreadySeen,
    NodeAlreadyInRing,
    NodeNotInRing,
    UnexpectedRequestType,
}

impl Error {
    pub(crate) fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub(crate) fn new(kind: ErrorKind, source: Option<Source>) -> Self {
        Self { kind, source }
    }

    pub(crate) fn new_broken_pipe(source: Option<Source>) -> Self {
        Self::new(ErrorKind::BrokenPipe, source)
    }

    pub(crate) fn new_join(source: Option<Source>) -> Self {
        Self::new(ErrorKind::Join, source)
    }


    pub(crate) fn new_uuid_already_seen() -> Self {
        Self::new(ErrorKind::UuidAlreadySeen, None)
    }

    pub(crate) fn new_node_already_in_ring() -> Self {
        Self::new(ErrorKind::NodeAlreadyInRing, None)
    }

    pub(crate) fn new_node_not_in_ring() -> Self {
        Self::new(ErrorKind::NodeNotInRing, None)
    }

    pub(crate) fn new_unexpected_request(source: Option<Source>) -> Self {
        Self::new(ErrorKind::UnexpectedRequestType, source)
    }
}

impl From<ErrorKind> for Error {
    fn from(t: ErrorKind) -> Self {
        Error::new(t, None)
    }
}

impl From<(ErrorKind, Source)> for Error {
    fn from(t: (ErrorKind, Source)) -> Self {
        Error::new(t.0, Some(t.1))
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut f = f.debug_tuple("Error");
        f.field(&self.kind);
        if let Some(source) = &self.source {
            f.field(source);
        }
        f.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(f, "{}: {}", self.kind, source)
        } else {
            write!(f, "{}", self.kind)
        }
    }
}

impl error::Error for Error {}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
