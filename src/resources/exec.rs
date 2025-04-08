use super::resource::{Ensure, Resource};

pub struct Exec {
    pub title: String,
}

impl Exec {
    fn ensure_present(&self) {
        println!("Ensure present: {}", self.title);
    }
    fn ensure_absent(&self) {
        println!("Ensure absent: {}", self.title);
    }
}

impl Resource for Exec {
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
