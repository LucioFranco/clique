use std::{error, fmt};

pub type Result<T> = std::result::Result<T, Error>;

type Source = Box<dyn error::Error + Send + 'static>;

pub struct Error {
    kind: ErrorKind,
    source: Option<Source>,
}

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ErrorKind {
    Start,
    Join,
    BrokenPipe,

    UuidAlreadySeen,
    NodeAlreadyInRing,
    NodeNotInRing,
    UnexpectedRequestType,

    VoteAlreadyReceived,
    AlreadyReachedconsensus,
    FastRoundFailure,

    JoinPhase1,
    JoinPhase2,

    UnableToReachDecision,
}

impl Error {
    #[allow(unused)]
    pub(crate) fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    pub(crate) fn new_join_phase1() -> Self {
        Self::new(ErrorKind::JoinPhase1, None)
    }

    pub(crate) fn new(kind: ErrorKind, source: Option<Source>) -> Self {
        Self { kind, source }
    }

    pub(crate) fn new_broken_pipe(source: Option<Source>) -> Self {
        Self::new(ErrorKind::BrokenPipe, source)
    }

    #[allow(dead_code)]
    pub(crate) fn new_join(source: Option<Source>) -> Self {
        Self::new(ErrorKind::Join, source)
    }

    #[allow(unused)]
    pub(crate) fn new_uuid_already_seen() -> Self {
        Self::new(ErrorKind::UuidAlreadySeen, None)
    }

    #[allow(unused)]
    pub(crate) fn new_node_already_in_ring() -> Self {
        Self::new(ErrorKind::NodeAlreadyInRing, None)
    }

    pub(crate) fn new_node_not_in_ring() -> Self {
        Self::new(ErrorKind::NodeNotInRing, None)
    }

    pub(crate) fn vote_already_received() -> Self {
        Self::new(ErrorKind::VoteAlreadyReceived, None)
    }

    pub(crate) fn already_reached_consensus() -> Self {
        Self::new(ErrorKind::AlreadyReachedconsensus, None)
    }

    pub(crate) fn fast_round_failure() -> Self {
        Self::new(ErrorKind::FastRoundFailure, None)
    }

    pub(crate) fn new_unexpected_request(source: Option<Source>) -> Self {
        Self::new(ErrorKind::UnexpectedRequestType, source)
    }

    pub(crate) fn new_join_phase2() -> Self {
        Self::new(ErrorKind::JoinPhase2, None)
    }

    pub(crate) fn new_unable_to_reach_decision() -> Self {
        Self::new(ErrorKind::UnableToReachDecision, None)
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
