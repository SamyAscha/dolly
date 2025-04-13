use super::resource::{Ensure, Resource};

#[derive(Debug, Clone)]
pub struct File {
    pub title: String,
}

impl File {
    fn ensure_present(&self) {
        println!("Ensure present: {}", self.title);
    }
    fn ensure_absent(&self) {
        println!("Ensure absent: {}", self.title);
    }
}

impl Resource for File {
    fn rtype(&self) -> &str {
        "File"
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
