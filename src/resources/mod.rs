pub mod exec;
pub mod file;
pub mod foo_bar;
pub mod resource;
pub mod service;

pub use exec::Exec;
pub use file::File;
pub use foo_bar::FooBar;
pub use resource::Ensure;
pub use resource::Relation;
pub use resource::Resource;
pub use service::Service;

use crate::parser::pp::PuppetExpr;

use anyhow::{Result, anyhow};

impl TryFrom<&PuppetExpr> for Box<dyn Resource> {
    type Error = anyhow::Error;
    fn try_from(expr: &PuppetExpr) -> Result<Self> {
        match expr {
            PuppetExpr::Resource { rtype, title, .. } => match rtype.as_str() {
                "File" => Ok(Box::new(File {
                    title: title.to_string(),
                })),
                "Exec" => Ok(Box::new(Exec {
                    title: title.to_string(),
                })),
                "Service" => Ok(Box::new(Service {
                    title: title.to_string(),
                })),
                "Foo::Bar" => Ok(Box::new(FooBar {
                    title: title.to_string(),
                })),
                no_match => Err(anyhow!("unknown rtype: {no_match}")),
            },
            PuppetExpr::Relation { .. } => {
                Err(anyhow!("The expr is not a relation. Expected a resource."))
            }
        }
    }
}
