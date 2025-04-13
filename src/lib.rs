use anyhow::{Result, anyhow};
use indexmap::IndexMap;
use parser::pp::{Manifest, PuppetExpr, RelationOp, ResourceRef};
use petgraph::{
    acyclic::Acyclic, algo::toposort, dot::Dot, graph::NodeIndex, prelude::StableDiGraph,
};
use resources::{Relation, Resource};
use std::collections::HashMap;

pub mod parser;
pub mod resources;

type Unchecked = StableDiGraph<Box<dyn Resource>, Relation>;
type Checked = Acyclic<Unchecked>;

pub struct Plan(Checked);

impl Plan {
    pub fn plan(&self) -> &Checked {
        &self.0
    }

    pub fn dot(&self) -> petgraph::dot::Dot<'_, &Unchecked> {
        Dot::new(self.0.inner())
    }

    pub fn sorted(&self) -> Result<Vec<NodeIndex>> {
        toposort(self.0.inner(), None).map_err(|_| anyhow!("Plan is not acyclic"))
    }

    pub fn sorted_weights(&self) -> Result<IndexMap<NodeIndex, &Box<dyn Resource>>> {
        let mut weights = IndexMap::new();
        let indices = toposort(self.0.inner(), None).map_err(|_| anyhow!("Plan is not acyclic"))?;
        for index in indices {
            let Some(node) = self.0.inner().node_weight(index) else {
                return Err(anyhow!("Node without weight"));
            };
            weights.insert(index, node);
        }
        Ok(weights)
    }
}

pub fn parse_puppet_manifest(manifest: &Manifest) -> Result<Plan> {
    let mut resource_nodes = HashMap::new();

    let mut acyclic = StableDiGraph::<Box<dyn Resource>, Relation>::new();

    for resource in manifest.resources() {
        let resource_node: Box<dyn Resource> = resource.try_into()?;
        resource_nodes.insert(resource_node.id(), acyclic.add_node(resource_node));
    }

    let mut acyclic =
        Acyclic::try_from_graph(acyclic).map_err(|_| anyhow!("Error creating acyclic graph."))?;

    for relations in manifest.relations() {
        add_relations(&mut acyclic, &resource_nodes, relations)?;
    }
    Ok(Plan(acyclic))
}

fn add_relations(
    acyclic: &mut Acyclic<StableDiGraph<Box<dyn Resource>, Relation>>,
    resource_nodes: &HashMap<String, NodeIndex>,
    relations: &PuppetExpr,
) -> Result<()> {
    match relations {
        PuppetExpr::Resource { .. } => Err(anyhow!("Got resource, when expecting relation.")),
        PuppetExpr::Relation { from, to, op } => match op {
            RelationOp::Provide => {
                try_add_edges_from_relation(acyclic, resource_nodes, from, to, Relation::Provide)
            }
            RelationOp::Require => {
                try_add_edges_from_relation(acyclic, resource_nodes, to, from, Relation::Provide)
            }
            RelationOp::Notify => {
                try_add_edges_from_relation(acyclic, resource_nodes, from, to, Relation::Notify)
            }
            RelationOp::Subscribe => {
                try_add_edges_from_relation(acyclic, resource_nodes, to, from, Relation::Notify)
            }
        },
    }
}

fn try_add_edges_from_relation(
    graph: &mut Acyclic<StableDiGraph<Box<dyn Resource>, Relation>>,
    resource_nodes: &HashMap<String, NodeIndex>,
    froms: &Vec<ResourceRef>,
    tos: &Vec<ResourceRef>,
    relation: Relation,
) -> Result<()> {
    for from in froms {
        for to in tos {
            let Some(f) = resource_nodes.get(&from.id()) else {
                return Err(anyhow!("Unknown resource: {}", from.id()));
            };
            let Some(t) = resource_nodes.get(&to.id()) else {
                return Err(anyhow!("Unknown resource: {}", to.id()));
            };

            graph
                .try_add_edge(f.to_owned(), t.to_owned(), relation.clone())
                .map_err(|_| {
                    anyhow!("Error creating edge in acyclic graph: {from} {relation} {to}")
                })?;
        }
    }
    Ok(())
}
