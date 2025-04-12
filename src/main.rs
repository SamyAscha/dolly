use std::collections::HashMap;

use anyhow::{Result, anyhow};
use dolly::{
    parser::pp::{Manifest, ResourceRef},
    resources::{Relation, Resource},
};
use petgraph::{
    acyclic::Acyclic, algo::toposort, dot::Dot, graph::NodeIndex, prelude::StableDiGraph,
    visit::EdgeRef,
};

fn main() -> Result<()> {
    println!("Hello, world!");

    let input = include_bytes!("../res/test.pp");
    let input_string = String::from_utf8_lossy(input);

    println!("{input_string}");

    let manifest = &input_string.parse::<Manifest>()?;
    for expr in manifest.0.iter() {
        println!("{:?}", expr); // Use Resource trait
    }

    let mut acyclic = StableDiGraph::<Box<dyn Resource>, Relation>::new();

    let mut resource_nodes = HashMap::new();

    for resource in manifest.resources() {
        let resource_node: Box<dyn Resource> = resource.try_into()?;
        resource_nodes.insert(resource_node.id(), acyclic.add_node(resource_node));
    }

    let mut acyclic =
        Acyclic::try_from_graph(acyclic).map_err(|_| anyhow!("Error creating acyclic graph."))?;

    for relation in manifest.relations() {
        match relation {
            dolly::parser::pp::PuppetExpr::Resource { .. } => {
                return Err(anyhow!("Got resource, when expecting relation."));
            }
            dolly::parser::pp::PuppetExpr::Relation { from, to, op } => match op {
                dolly::parser::pp::RelationOp::Provide => {
                    try_add_edges_from_relation(
                        &mut acyclic,
                        &resource_nodes,
                        from,
                        to,
                        Relation::Provide,
                    )?;
                }
                dolly::parser::pp::RelationOp::Require => {
                    try_add_edges_from_relation(
                        &mut acyclic,
                        &resource_nodes,
                        to,
                        from,
                        Relation::Provide,
                    )?;
                }
                dolly::parser::pp::RelationOp::Notify => {
                    try_add_edges_from_relation(
                        &mut acyclic,
                        &resource_nodes,
                        from,
                        to,
                        Relation::Notify,
                    )?;
                }
                dolly::parser::pp::RelationOp::Subscribe => {
                    try_add_edges_from_relation(
                        &mut acyclic,
                        &resource_nodes,
                        to,
                        from,
                        Relation::Notify,
                    )?;
                }
            },
        }
    }

    println!("{:?}", Dot::new(&acyclic.inner()));

    let sorted = toposort(acyclic.inner(), None)
        .map_err(|_| anyhow!("Error toposorting cyclic acyclic graph."))?;

    for node_index in sorted {
        let Some(node) = acyclic.node_weight(node_index) else {
            return Err(anyhow!("Node without weight"));
        };
        let edges = acyclic.edges(node_index);
        println!("{:?}", node);
        for edge in edges {
            let Some((_, dest_node_id)) = acyclic.edge_endpoints(edge.id()) else {
                return Err(anyhow!("Edge without target node"));
            };
            let Some(dest_node) = acyclic.node_weight(dest_node_id) else {
                return Err(anyhow!("Dest node without weight"));
            };
            println!(
                "  {} {}",
                match edge.weight() {
                    Relation::Notify => "~>",
                    Relation::Provide => "->",
                },
                dest_node.title()
            );
        }
    }

    Ok(())
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
