use super::resource::{Ensure, Resource};

#[derive(Debug, Clone)]
pub struct FooBar {
    pub title: String,
}

impl FooBar {
    fn ensure_present(&self) {
        println!("Ensure present: {}", self.title);
    }
    fn ensure_absent(&self) {
        println!("Ensure absent: {}", self.title);
    }
}

impl Resource for FooBar {
    fn rtype(&self) -> &str {
        "Foo::Bar"
    }

    fn title(&self) -> String {
        self.title.clone()
    }

    fn ensure(&self, ensure: super::resource::Ensure) {
        match ensure {
            Ensure::Present => {
                self.ensure_present();
            }
            Ensure::Absent => {
                self.ensure_absent();
            }
        }
    }
}
