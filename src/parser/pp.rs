use anyhow::{Result, anyhow};
use pest::Parser;
use pest_derive::Parser;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display};
use std::str::FromStr;

#[derive(Parser)]
#[grammar = "../res/puppet.pest"]
struct PuppetParser;

#[derive(Debug)]
pub enum PuppetExpr {
    Resource {
        rtype: String,
        title: PuppetString,
        attributes: Vec<Attribute>,
    },
    Relation {
        from: Vec<ResourceRef>,
        to: Vec<ResourceRef>,
        op: RelationOp,
    },
}

// "->", "<-", "~>", "<~"
#[derive(Debug)]
pub enum RelationOp {
    Provide,
    Require,
    Notify,
    Subscribe,
}

impl FromStr for RelationOp {
    type Err = anyhow::Error;
    fn from_str(op: &str) -> Result<Self> {
        match op {
            "->" => Ok(Self::Provide),
            "<-" => Ok(Self::Require),
            "~>" => Ok(Self::Notify),
            "<~" => Ok(Self::Subscribe),
            bad => {
                unreachable!(
                    "RelationOp:from_str() should never get an unknown 'arrow' str: {bad}. Check the parser."
                )
            }
        }
    }
}

impl fmt::Display for RelationOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Provide => write!(f, "->"),
            Self::Require => write!(f, "<-"),
            Self::Notify => write!(f, "~>"),
            Self::Subscribe => write!(f, "<~"),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct PuppetString(Vec<StringContent>);

impl PuppetString {
    pub fn new() -> Self {
        Self(vec![])
    }
}

impl fmt::Display for PuppetString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for content in self.0.iter() {
            match content {
                StringContent::Literal(s) => write!(f, "{}", s)?,
                StringContent::Variable(v) => write!(f, "${{{}}}", v)?,
            };
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum StringContent {
    Literal(String),
    Variable(String),
}

impl fmt::Display for StringContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StringContent::Literal(s) => write!(f, "{}", s),
            StringContent::Variable(v) => write!(f, "${{{}}}", v),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceRef {
    pub rtype: String,
    pub title: PuppetString,
}

impl ResourceRef {
    pub fn id(&self) -> String {
        format!("{}[{}]", self.rtype, self.title)
    }
}

impl Display for ResourceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id())
    }
}

#[derive(Debug)]
pub struct Attribute {
    pub name: String,
    pub value: PuppetString,
}

#[derive(Debug)]
struct PuppetError {
    message: String,
}

impl fmt::Display for PuppetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PuppetError: {}", self.message)
    }
}

#[derive(Debug)]
pub struct Manifest(pub Vec<PuppetExpr>);

impl Manifest {
    pub fn resources(&self) -> impl Iterator<Item = &PuppetExpr> {
        self.0
            .iter()
            .filter(|s| matches!(s, PuppetExpr::Resource { .. }))
    }

    pub fn relations(&self) -> impl Iterator<Item = &PuppetExpr> {
        self.0
            .iter()
            .filter(|s| matches!(s, PuppetExpr::Relation { .. }))
    }
}

impl Display for Manifest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for expr in self.0.iter() {
            writeln!(f, "{}", expr)?;
        }
        Ok(())
    }
}

impl FromStr for Manifest {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut pairs = PuppetParser::parse(Rule::program, s)?;

        let mut expressions = Vec::new();
        let mut resources = HashMap::new();

        let Some(program) = pairs.next() else {
            return Err(anyhow!(PuppetError {
                message: "No program pair".to_owned()
            }));
        };

        for pair in program.into_inner() {
            match pair.as_rule() {
                Rule::resource => {
                    let mut rtype = String::new();
                    let mut title = PuppetString::new();
                    let mut attributes = Vec::new();

                    for inner in pair.into_inner() {
                        match inner.as_rule() {
                            Rule::rtype => {
                                rtype = inner.as_str().to_string();
                            }
                            Rule::title => {
                                for tp in inner.into_inner() {
                                    match tp.as_rule() {
                                        Rule::quoted_string => {
                                            title = parse_quoted_string(tp)?;
                                        }
                                        no_match => {
                                            println!("Nothing matches on: {no_match:#?}");
                                        }
                                    }
                                }
                            }
                            Rule::attributes => {
                                let mut attr_name = String::new();
                                let mut attr_value = PuppetString::new();
                                for attr_pair in inner.into_inner() {
                                    match attr_pair.as_rule() {
                                        Rule::attribute => {
                                            for ap in attr_pair.into_inner() {
                                                match ap.as_rule() {
                                                    Rule::attr_name => {
                                                        attr_name = ap.as_str().to_string();
                                                    }
                                                    Rule::attr_value => {
                                                        for avp in ap.into_inner() {
                                                            match avp.as_rule() {
                                                                Rule::quoted_string => {
                                                                    attr_value =
                                                                        parse_quoted_string(avp)?;
                                                                }
                                                                no_match => {
                                                                    println!(
                                                                        "Nothing matches on: {no_match:#?}"
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                    no_match => {
                                                        println!(
                                                            "Nothing matches on: {no_match:#?}"
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        no_match => {
                                            println!("Nothing matches on: {no_match:#?}");
                                        }
                                    }
                                }
                                attributes.push(Attribute {
                                    name: attr_name,
                                    value: attr_value,
                                });
                            }
                            no_match => {
                                println!("Nothing matches on: {no_match:#?}");
                            }
                        }
                    }
                    let resource_ref = ResourceRef {
                        rtype: rtype.to_lowercase(),
                        title: title.clone(),
                    };
                    resources.insert(resource_ref, ());
                    expressions.push(PuppetExpr::Resource {
                        rtype: rtype.to_lowercase(),
                        title,
                        attributes,
                    });
                }
                Rule::relation => {
                    let mut relation_parts = Vec::new();
                    let mut current_refs = Vec::new();

                    for inner in pair.into_inner() {
                        match inner.as_rule() {
                            Rule::ref_arg => {
                                for rl_rr in inner.into_inner() {
                                    match rl_rr.as_rule() {
                                        Rule::resource_ref => {
                                            let mut ref_rtype = String::new();
                                            let mut ref_title = PuppetString::new();
                                            for ref_inner in rl_rr.into_inner() {
                                                match ref_inner.as_rule() {
                                                    Rule::rtype => {
                                                        ref_rtype =
                                                            ref_inner.as_str().to_lowercase();
                                                    }
                                                    Rule::quoted_string => {
                                                        ref_title = parse_quoted_string(ref_inner)?;
                                                    }
                                                    no_match => {
                                                        println!(
                                                            "ref_list: Nothing matches on: {no_match:#?}"
                                                        );
                                                    }
                                                }
                                            }
                                            current_refs.push(ResourceRef {
                                                rtype: ref_rtype,
                                                title: ref_title,
                                            });
                                        }
                                        Rule::ref_list => {
                                            let mut refs = Vec::new();
                                            for ref_pair in rl_rr.into_inner() {
                                                if ref_pair.as_rule() == Rule::resource_ref {
                                                    let mut ref_rtype = String::new();
                                                    let mut ref_title = PuppetString::new();
                                                    for ref_inner in ref_pair.into_inner() {
                                                        match ref_inner.as_rule() {
                                                            Rule::rtype => {
                                                                ref_rtype = ref_inner
                                                                    .as_str()
                                                                    .to_lowercase();
                                                            }
                                                            Rule::quoted_string => {
                                                                ref_title =
                                                                    parse_quoted_string(ref_inner)?;
                                                            }
                                                            no_match => {
                                                                println!(
                                                                    "ref_list: Nothing matches on: {no_match:#?}"
                                                                );
                                                            }
                                                        }
                                                    }
                                                    refs.push(ResourceRef {
                                                        rtype: ref_rtype,
                                                        title: ref_title,
                                                    });
                                                }
                                            }
                                            current_refs = refs;
                                        }
                                        no_match => {
                                            println!("rel_op: Nothing matches on: {no_match:#?}");
                                        }
                                    }
                                }
                            }
                            Rule::rel_op => {
                                if !current_refs.is_empty() {
                                    relation_parts
                                        .push((current_refs.clone(), inner.as_str().to_string()));
                                    current_refs = Vec::new();
                                }
                            }
                            no_match => {
                                println!("rel_op: Nothing matches on: {no_match:#?}");
                            }
                        }
                    }

                    // Add the final ref_arg
                    if !current_refs.is_empty() {
                        relation_parts.push((current_refs.clone(), "".to_string())); // Dummy op for last refs
                    }

                    // Create relations from consecutive ref_args
                    for i in 0..relation_parts.len().saturating_sub(1) {
                        let (from, op_str) = &relation_parts[i];
                        let (to, _) = &relation_parts[i + 1];
                        if !op_str.is_empty() {
                            expressions.push(PuppetExpr::Relation {
                                from: from.clone(),
                                to: to.clone(),
                                op: op_str.parse()?,
                            });
                        }
                    }
                }
                no_match => {
                    println!("Nothing matches on: {no_match:#?}");
                }
            }
        }

        // Second pass: Validate references
        for expr in &expressions {
            if let PuppetExpr::Relation { from, to, .. } = expr {
                for r in from.iter().chain(to.iter()) {
                    if !resources.contains_key(r) {
                        return Err(anyhow!(PuppetError {
                            message: format!(
                                "Undefined resource reference: {}['{}']",
                                r.rtype, r.title
                            ),
                        }));
                    }
                }
            }
        }
        Ok(Manifest(expressions))
    }
}

impl Error for PuppetError {}

impl fmt::Display for PuppetExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PuppetExpr::Resource {
                rtype,
                title,
                attributes,
            } => {
                write!(f, "{} {{\n  '", rtype)?;
                write!(f, "{title}")?;
                writeln!(f, "':")?;
                for attr in attributes {
                    write!(f, "    {} => ", attr.name)?;
                    write!(f, "{}", attr.value)?;
                    writeln!(f, ",")?;
                }
                write!(f, "}}")
            }
            PuppetExpr::Relation { from, to, op } => {
                write!(f, "[")?;
                for (i, r) in from.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}['", r.rtype)?;
                    write!(f, "{}", r.title)?;
                    write!(f, "']")?;
                }
                write!(f, "] {} [", op)?;
                for (i, r) in to.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}['", r.rtype)?;
                    write!(f, "{}", r.title)?;
                    write!(f, "']")?;
                }
                write!(f, "]")
            }
        }
    }
}

fn parse_quoted_string(pair: pest::iterators::Pair<Rule>) -> Result<PuppetString> {
    let mut content = Vec::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::single_quoted => {
                content.push(StringContent::Literal(
                    inner.as_str().trim_matches('\'').to_string(),
                ));
            }
            Rule::double_quoted => {
                for content_pair in inner.into_inner() {
                    match content_pair.as_rule() {
                        Rule::double_quoted_content => {
                            for inner_content in content_pair.into_inner() {
                                match inner_content.as_rule() {
                                    Rule::variable => {
                                        let var = inner_content.into_inner().next().unwrap();
                                        content.push(StringContent::Variable(
                                            var.as_str().to_string(),
                                        ));
                                    }
                                    Rule::plain => {
                                        content.push(StringContent::Literal(
                                            inner_content.as_str().to_string(),
                                        ));
                                    }
                                    no_match => {
                                        println!("Nothing matches on: {no_match:#?}");
                                    }
                                }
                            }
                        }
                        no_match => {
                            println!("Nothing matches on: {no_match:#?}");
                        }
                    }
                }
            }
            no_match => {
                println!("Nothing matches on: {no_match:#?}");
            }
        }
    }
    Ok(PuppetString(content))
}
