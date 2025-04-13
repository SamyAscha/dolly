use anyhow::{Result, anyhow};
use dolly::{parse_puppet_manifest, parser::pp::Manifest, resources::Relation};
use petgraph::visit::EdgeRef;

fn main() -> Result<()> {
    let input = String::from_utf8_lossy(include_bytes!("../res/test.pp"));
    println!("{input}");

    let manifest = &input.parse::<Manifest>()?;
    println!("{manifest}");

    let plan = parse_puppet_manifest(manifest)?;

    println!("{:?}", plan.dot());

    //let sorted = plan.sorted()?;

    for (index, node) in plan.sorted_weights()? {
        let edges = plan.plan().edges(index);
        print!("{:?}", node);
        for edge in edges {
            let Some((_, dest_node_id)) = plan.plan().edge_endpoints(edge.id()) else {
                return Err(anyhow!("Edge without target node"));
            };
            let Some(dest_node) = plan.plan().node_weight(dest_node_id) else {
                return Err(anyhow!("Dest node without weight"));
            };
            print!(
                " ({} {})",
                match edge.weight() {
                    Relation::Notify => "~>",
                    Relation::Provide => "->",
                },
                dest_node.title()
            );
        }
        println!();
    }

    Ok(())
}
