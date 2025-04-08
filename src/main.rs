use dolly::resources::{Ensure, Exec, File, Resource};

fn main() {
    println!("Hello, world!");
    let resources: Vec<Box<dyn Resource>> = vec![
        Box::new(File {
            title: "Yo".to_string(),
        }),
        Box::new(Exec {
            title: "Oy".to_string(),
        }),
    ];
    for r in resources {
        r.ensure(Ensure::Present);
    }
}
