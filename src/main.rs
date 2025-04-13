use anyhow::Result;
use dolly::{parse_puppet_manifest, parser::pp::Manifest};
use petgraph::visit::EdgeRef;

fn main() -> Result<()> {
    let input = String::from_utf8_lossy(include_bytes!("../res/test.pp"));
    let manifest = &input.parse::<Manifest>()?;
    let plan = parse_puppet_manifest(manifest)?;

    println!("{:?}", plan.dot());

    print!("# Execution plan debug:");
    for (index, node) in plan.sorted_weights()? {
        let edges = plan.plan().edges(index);
        print!("# {}", node.id());
        for edge in edges {
            print!(
                " ({} {})",
                edge.weight(),
                plan.sorted_weights()?.get(&edge.target()).unwrap().id()
            );
        }
        println!();
    }
    Ok(())
}
