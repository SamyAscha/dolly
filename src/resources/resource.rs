use core::fmt::Debug as FmtDebug;
use petgraph::{acyclic::Acyclic, prelude::StableDiGraph};
use std::fmt;

pub trait Resource {
    fn rtype(&self) -> &str;

    fn title(&self) -> String;

    fn ensure(&self, ensure: Ensure);

    fn id(&self) -> String {
        format!("{}[{}]", self.rtype(), self.title())
    }
}

#[derive(Debug, Default)]
pub enum Ensure {
    #[default]
    Present,
    Absent,
}

#[derive(Debug, Clone)]
pub enum Relation {
    Provide,
    Notify,
}

impl fmt::Display for Relation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provide => write!(f, "->"),
            Self::Notify => write!(f, "~>"),
        }
    }
}

impl FmtDebug for dyn Resource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.title())
    }
}

pub struct Plan {
    pub graph: Acyclic<StableDiGraph<Box<dyn Resource>, Relation>>,
}
