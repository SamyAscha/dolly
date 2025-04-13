use anyhow::{Result, anyhow};
use pest::Parser;
use pest_derive::Parser;
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};
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
            bad => Err(anyhow!("Invalid relation operator: {bad}")),
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
    Literal(Cow<'static, str>),
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

#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Hash for ResourceRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
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

impl Error for PuppetError {}

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
                    let expr = parse_resource(pair)?;
                    if let PuppetExpr::Resource { rtype, title, .. } = &expr {
                        let resource_ref = ResourceRef {
                            rtype: to_uc_first(rtype),
                            title: PuppetString(title.0.clone()),
                        };
                        resources.insert(resource_ref, ());
                    }
                    expressions.push(expr);
                }
                Rule::relation => {
                    expressions.extend(parse_relation(pair)?);
                }
                _ => {} // Silently ignore unknown rules (e.g., EOI)
            }
        }

        validate_references(&expressions, &resources)?;
        Ok(Manifest(expressions))
    }
}

fn parse_resource(pair: pest::iterators::Pair<Rule>) -> Result<PuppetExpr> {
    let mut rtype = String::new();
    let mut title = PuppetString::new();
    let mut attributes = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::rtype => {
                rtype = to_uc_first(inner.as_str());
            }
            Rule::title => {
                title = parse_quoted_string(inner.into_inner().next().ok_or_else(|| {
                    anyhow!(PuppetError {
                        message: "Missing title".to_string()
                    })
                })?)?;
            }
            Rule::attributes => {
                attributes = parse_attributes(inner)?;
            }
            _ => {}
        }
    }

    Ok(PuppetExpr::Resource {
        rtype,
        title,
        attributes,
    })
}

fn parse_attributes(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Attribute>> {
    let mut attributes = Vec::new();
    for attr_pair in pair.into_inner() {
        if attr_pair.as_rule() == Rule::attribute {
            let mut attr_name = String::new();
            let mut attr_value = PuppetString::new();
            for ap in attr_pair.into_inner() {
                match ap.as_rule() {
                    Rule::attr_name => {
                        attr_name = ap.as_str().to_string();
                    }
                    Rule::attr_value => {
                        attr_value =
                            parse_quoted_string(ap.into_inner().next().ok_or_else(|| {
                                anyhow!(PuppetError {
                                    message: "Missing attribute value".to_string()
                                })
                            })?)?;
                    }
                    _ => {}
                }
            }
            attributes.push(Attribute {
                name: attr_name,
                value: attr_value,
            });
        }
    }
    Ok(attributes)
}

fn parse_relation(pair: pest::iterators::Pair<Rule>) -> Result<Vec<PuppetExpr>> {
    let mut relation_parts = Vec::new();
    let mut current_refs = Vec::new();

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ref_arg => {
                current_refs = parse_ref_arg(inner)?;
            }
            Rule::rel_op => {
                if !current_refs.is_empty() {
                    relation_parts.push((current_refs.clone(), inner.as_str().to_string()));
                    current_refs = Vec::new();
                }
            }
            _ => {}
        }
    }

    if !current_refs.is_empty() {
        relation_parts.push((current_refs, "".to_string()));
    }

    let mut expressions = Vec::new();
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

    Ok(expressions)
}

fn parse_ref_arg(pair: pest::iterators::Pair<Rule>) -> Result<Vec<ResourceRef>> {
    let mut refs = Vec::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::resource_ref => {
                refs.push(parse_resource_ref(inner)?);
            }
            Rule::ref_list => {
                for ref_pair in inner.into_inner() {
                    if ref_pair.as_rule() == Rule::resource_ref {
                        refs.push(parse_resource_ref(ref_pair)?);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(refs)
}

fn parse_resource_ref(pair: pest::iterators::Pair<Rule>) -> Result<ResourceRef> {
    let mut rtype = String::new();
    let mut title = PuppetString::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::ref_rtype => {
                rtype = inner.as_str().to_string();
            }
            Rule::quoted_string => {
                title = parse_quoted_string(inner)?;
            }
            _ => {}
        }
    }
    Ok(ResourceRef { rtype, title })
}

fn validate_references(
    expressions: &[PuppetExpr],
    resources: &HashMap<ResourceRef, ()>,
) -> Result<()> {
    for expr in expressions {
        if let PuppetExpr::Relation { from, to, .. } = expr {
            for r in from.iter().chain(to.iter()) {
                eprintln!("Key: {r:#?}");
                if !resources.contains_key(r) {
                    return Err(anyhow!(PuppetError {
                        message: format!("Undefined resource reference: {}", r.id()),
                    }));
                }
            }
        }
    }
    Ok(())
}

fn parse_quoted_string(pair: pest::iterators::Pair<Rule>) -> Result<PuppetString> {
    let mut content = Vec::new();
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::single_quoted => {
                content.push(StringContent::Literal(
                    inner.as_str().trim_matches('\'').to_string().into(),
                ));
            }
            Rule::double_quoted => {
                for content_pair in inner.into_inner() {
                    if content_pair.as_rule() == Rule::double_quoted_content {
                        for inner_content in content_pair.into_inner() {
                            match inner_content.as_rule() {
                                Rule::variable => {
                                    let var = inner_content
                                        .into_inner()
                                        .next()
                                        .ok_or_else(|| anyhow!("Missing variable name"))?;
                                    content.push(StringContent::Variable(var.as_str().to_string()));
                                }
                                Rule::plain => {
                                    content.push(StringContent::Literal(
                                        inner_content.as_str().to_string().into(),
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    Ok(PuppetString(content))
}

fn to_uc_first(s: &str) -> String {
    s.split("::")
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<String>>()
        .join("::")
}
