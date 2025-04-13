use anyhow::{Result, anyhow};
use indexmap::IndexMap;
use parser::pp::{Manifest, PuppetExpr, RelationOp, ResourceRef};
use petgraph::{
    acyclic::Acyclic,
    algo::toposort,
    dot::{Config, Dot},
    graph::NodeIndex,
    prelude::StableDiGraph,
    visit::NodeRef,
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
        let g = self.0.inner();
        Dot::with_attr_getters(
            g,
            &[Config::NodeNoLabel, Config::EdgeNoLabel],
            &|_g, edge| format!("label = \"{:?}\"", edge.weight()),
            &|_g, node| format!("label = \"{}\"", node.weight().id()),
        )
    }

    pub fn sorted(&self) -> Result<Vec<NodeIndex>> {
        toposort(self.0.inner(), None).map_err(|_| anyhow!("Plan is not acyclic"))
    }

    pub fn sorted_weights(&self) -> Result<IndexMap<NodeIndex, &dyn Resource>> {
        let mut weights = IndexMap::new();
        let indices = toposort(self.0.inner(), None).map_err(|_| anyhow!("Plan is not acyclic"))?;
        for index in indices {
            let Some(node) = self.0.inner().node_weight(index) else {
                return Err(anyhow!("Node without weight"));
            };
            weights.insert(index, node.as_ref());
        }
        Ok(weights)
    }
}

pub fn parse_puppet_manifest(manifest: &Manifest) -> Result<Plan> {
    let mut resource_nodes = HashMap::new();

    let mut acyclic = StableDiGraph::<Box<dyn Resource>, Relation>::new();

    for resource in manifest.resources() {
        let resource_node: Box<dyn Resource> = resource.try_into()?;
        let id = resource_node.id();
        resource_nodes.insert(id.clone(), acyclic.add_node(resource_node));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // 0. Tmp Cases

    // 1. Simple Cases
    #[test]
    fn test_empty_manifest() -> Result<()> {
        let input = "";
        let manifest = Manifest::from_str(input)?;
        assert_eq!(
            manifest.0.len(),
            0,
            "Empty input should produce empty manifest"
        );
        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            0,
            "Empty manifest should produce empty graph"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            0,
            "Empty manifest should have no edges"
        );
        Ok(())
    }

    #[test]
    fn test_single_file_resource() -> Result<()> {
        let input = r#"
            file { "/tmp/one":
            }
        "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(manifest.0.len(), 1, "Should have one resource");

        if let Some(PuppetExpr::Resource {
            rtype,
            title,
            attributes,
        }) = manifest.0.first()
        {
            assert_eq!(rtype, "File", "Resource type should be 'File' (uc_first)");
            assert_eq!(title.to_string(), "/tmp/one", "Title should be '/tmp/one'");
            assert!(attributes.is_empty(), "Attributes should be empty");
        } else {
            return Err(anyhow!("Expected a Resource variant"));
        }

        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            1,
            "Plan should have one node"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            0,
            "Plan should have no edges"
        );

        let weights = plan.sorted_weights()?;
        assert_eq!(weights.len(), 1, "Sorted weights should have one node");
        if let Some((_, node)) = weights.first() {
            assert_eq!(
                node.rtype(),
                "File",
                "Node rtype should be 'File' (uc_first)"
            );
            assert_eq!(node.title(), "/tmp/one", "Node title should be '/tmp/one'");
        }

        Ok(())
    }

    #[test]
    fn test_single_relation() -> Result<()> {
        let input = r#"
            file { "/tmp/one": }
            service { "nginx": }
            File["/tmp/one"] -> Service["nginx"]
        "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(
            manifest.0.len(),
            3,
            "Should have two resources and one relation"
        );

        let resources: Vec<_> = manifest.resources().collect();
        assert_eq!(resources.len(), 2, "Should have two resources");
        if let PuppetExpr::Resource { rtype, title, .. } = resources[0] {
            assert_eq!(rtype, "File", "First resource is File (uc_first");
            assert_eq!(title.to_string(), "/tmp/one");
        }
        if let PuppetExpr::Resource { rtype, title, .. } = resources[1] {
            assert_eq!(rtype, "Service", "Second resource is Service (uc_first");
            assert_eq!(title.to_string(), "nginx");
        }

        let relations: Vec<_> = manifest.relations().collect();
        assert_eq!(relations.len(), 1, "Should have one relation");
        if let PuppetExpr::Relation { from, to, op } = relations[0] {
            assert_eq!(from.len(), 1, "From should have one ref");
            assert_eq!(from[0].rtype, "File");
            assert_eq!(from[0].title.to_string(), "/tmp/one");
            assert_eq!(to.len(), 1, "To should have one ref");
            assert_eq!(to[0].rtype, "Service");
            assert_eq!(to[0].title.to_string(), "nginx");
            assert!(matches!(op, RelationOp::Provide), "Op should be ->");
        }

        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            2,
            "Plan should have two nodes"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            1,
            "Plan should have one edge"
        );

        let weights = plan.sorted_weights()?;
        assert_eq!(weights.len(), 2, "Sorted weights should have two nodes");
        let titles: Vec<_> = weights.values().map(|node| node.title()).collect();
        assert!(titles.contains(&"/tmp/one".to_string()));
        assert!(titles.contains(&"nginx".to_string()));

        Ok(())
    }

    // 2. Moderate Complexity
    #[test]
    fn test_multiple_resources_with_variables() -> Result<()> {
        let input = r#"
            file { "/tmp/${var}/test":
                mode => "0644",
            }
            exec { "/root/${script}/run.sh": }
            service { "nginx": }
        "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(manifest.0.len(), 3, "Should have three resources");

        let resources: Vec<_> = manifest.resources().collect();
        assert_eq!(resources.len(), 3, "Should have three resources");

        // File resource
        if let PuppetExpr::Resource {
            rtype,
            title,
            attributes,
        } = resources[0]
        {
            assert_eq!(rtype, "File");
            assert_eq!(title.to_string(), "/tmp/${var}/test");
            assert_eq!(attributes.len(), 1, "Should have one attribute");
            assert_eq!(attributes[0].name, "mode");
            assert_eq!(attributes[0].value.to_string(), "0644");
        }

        // Exec resource
        if let PuppetExpr::Resource {
            rtype,
            title,
            attributes,
        } = resources[1]
        {
            assert_eq!(rtype, "Exec");
            assert_eq!(title.to_string(), "/root/${script}/run.sh");
            assert!(attributes.is_empty());
        }

        // Service resource
        if let PuppetExpr::Resource {
            rtype,
            title,
            attributes,
        } = resources[2]
        {
            assert_eq!(rtype, "Service");
            assert_eq!(title.to_string(), "nginx");
            assert!(attributes.is_empty());
        }

        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            3,
            "Plan should have three nodes"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            0,
            "Plan should have no edges"
        );

        Ok(())
    }

    #[test]
    fn test_chained_relations() -> Result<()> {
        let input = r#"
            file { "/tmp/one": }
            file { "/tmp/two": }
            service { "nginx": }
            File["/tmp/one"] -> File["/tmp/two"] ~> Service["nginx"]
        "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(
            manifest.0.len(),
            5,
            "Should have three resources and two relations"
        );

        let relations: Vec<_> = manifest.relations().collect();
        assert_eq!(relations.len(), 2, "Should have two relations");

        // First relation: File["/tmp/one"] -> File["/tmp/two"]
        if let PuppetExpr::Relation { from, to, op } = relations[0] {
            assert_eq!(from[0].id(), "File[/tmp/one]");
            assert_eq!(to[0].id(), "File[/tmp/two]");
            assert!(matches!(op, RelationOp::Provide));
        }

        // Second relation: File["/tmp/two"] ~> Service["nginx"]
        if let PuppetExpr::Relation { from, to, op } = relations[1] {
            assert_eq!(from[0].id(), "File[/tmp/two]");
            assert_eq!(to[0].id(), "Service[nginx]");
            assert!(matches!(op, RelationOp::Notify));
        }

        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            3,
            "Plan should have three nodes"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            2,
            "Plan should have two edges"
        );

        let weights = plan.sorted_weights()?;
        assert_eq!(weights.len(), 3, "Sorted weights should have three nodes");
        let titles: Vec<_> = weights.values().map(|node| node.id()).collect();
        assert!(titles.contains(&"File[/tmp/one]".to_string()));
        assert!(titles.contains(&"File[/tmp/two]".to_string()));
        assert!(titles.contains(&"Service[nginx]".to_string()));

        Ok(())
    }

    // 3. Edge Cases
    #[test]
    fn test_undefined_resource_reference() -> Result<()> {
        let input = r#"
            file { "/tmp/one": }
            File["/tmp/missing"] -> Service["nginx"]
        "#;
        let manifest = Manifest::from_str(input);
        assert!(manifest.is_err(), "Should fail due to undefined resource");
        if let Err(e) = manifest {
            assert!(
                e.to_string().contains("Undefined resource reference"),
                "Error should mention undefined reference"
            );
        }
        Ok(())
    }

    #[test]
    fn test_empty_quoted_string() -> Result<()> {
        let input = r#"
            file { "":
            }
        "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(manifest.0.len(), 1, "Should have one resource");
        if let PuppetExpr::Resource { rtype, title, .. } = &manifest.0[0] {
            assert_eq!(rtype, "File");
            assert_eq!(title.to_string(), "");
        }

        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            1,
            "Plan should have one node"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            0,
            "Plan should have no edges"
        );
        Ok(())
    }

    #[test]
    fn test_whitespace_variations() -> Result<()> {
        let input = r#"
            file  { "/tmp/one" : } file{ "/tmp/two" : } foo::bar  { "/tmp/two" : } service { "nginx" : } [File [ "/tmp/one" ], Foo::Bar [ "/tmp/two" ]] -> Service  [ "nginx" ]
        "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(
            manifest.0.len(),
            5,
            "Should have 3 resources and 2 relations"
        );

        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            4,
            "Plan should have 4 nodes"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            2,
            "Plan should have 2 edges"
        );
        Ok(())
    }

    #[test]
    fn test_reverse_relations() -> Result<()> {
        let input = r#"
            file { "/tmp/one": }
            service { "ssh": }
            Service["ssh"] <~ File["/tmp/one"]
        "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(
            manifest.0.len(),
            3,
            "Should have two resources and one relation"
        );

        let relations: Vec<_> = manifest.relations().collect();
        assert_eq!(relations.len(), 1, "Should have one relation");
        if let PuppetExpr::Relation { from, to, op } = relations[0] {
            assert_eq!(from[0].id(), "Service[ssh]");
            assert_eq!(to[0].id(), "File[/tmp/one]");
            assert!(matches!(op, RelationOp::Subscribe));
        }

        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            2,
            "Plan should have two nodes"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            1,
            "Plan should have one edge"
        );

        let weights = plan.sorted_weights()?;
        assert_eq!(weights.len(), 2, "Sorted weights should have two nodes");
        Ok(())
    }

    #[test]
    fn test_complex_test_pp() -> Result<()> {
        let input = include_str!("../res/test.pp");
        let manifest = Manifest::from_str(input)?;
        assert_eq!(
            manifest.0.len(),
            13,
            "Should have 8 resources and 5 relations"
        );

        let resources: Vec<_> = manifest.resources().collect();
        assert_eq!(resources.len(), 8, "Should have 8 resources");
        let resource_ids: Vec<_> = resources
            .iter()
            .map(|r| {
                if let PuppetExpr::Resource { rtype, title, .. } = r {
                    format!("{}[{}]", rtype, title)
                } else {
                    unreachable!()
                }
            })
            .collect();
        assert!(resource_ids.contains(&"File[/tmp/one]".to_string()));
        assert!(resource_ids.contains(&"File[/tmp/two]".to_string()));
        assert!(resource_ids.contains(&"File[/tmp/two/three]".to_string()));
        assert!(resource_ids.contains(&"File[/tmp/two/four]".to_string()));
        assert!(resource_ids.contains(&"Service[nginx]".to_string()));
        assert!(resource_ids.contains(&"Service[ssh]".to_string()));
        assert!(resource_ids.contains(&"Exec[/root/${scripts}/yo.sh]".to_string()));

        let relations: Vec<_> = manifest.relations().collect();
        assert_eq!(relations.len(), 5, "Should have 5 relations");

        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(
            plan.plan().inner().node_count(),
            8,
            "Plan should have 8 nodes"
        );
        assert_eq!(
            plan.plan().inner().edge_count(),
            7,
            "Plan should have 7 edges"
        );

        Ok(())
    }

    #[test]
    fn test_invalid_uc_first_ref() -> Result<()> {
        let input = r#"
        service { "nginx": }
        service["nginx"] -> File["/tmp/one"]
    "#;
        assert!(Manifest::from_str(input).is_err());
        Ok(())
    }

    #[test]
    fn test_uc_first_and_namespaced_refs() -> Result<()> {
        let input = r#"
        service { "nginx": }
        Service["nginx"] -> File["/tmp/one"]
        file { "/tmp/one": }
        foo::bar { "test": }
        Foo::Bar["test"] -> File["/tmp/one"]
    "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(manifest.0.len(), 5); // 3 resources, 2 relations
        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(plan.plan().inner().node_count(), 3);
        Ok(())
    }

    #[test]
    fn test_uc_first_namespaced_refs() -> Result<()> {
        let input = r#"
            service { "nginx": }
            Service["nginx"] -> File["/tmp/one"]
            file { "/tmp/one": }
            foo::bar { "test": }
            Foo::Bar["test"] -> File["/tmp/one"]
        "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(manifest.0.len(), 5);
        Ok(())
    }

    #[test]
    fn test_single_ref_ref_list() -> Result<()> {
        let input = r#"
        service { "nginx": }
        file { "/tmp/one": }
        [Service["nginx"]] -> File["/tmp/one"]
    "#;
        let manifest = Manifest::from_str(input)?;
        assert_eq!(manifest.0.len(), 3); // 2 resources, 1 relations
        let plan = parse_puppet_manifest(&manifest)?;
        assert_eq!(plan.plan().inner().node_count(), 2);
        Ok(())
    }
}
