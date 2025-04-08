pub trait Resource {
    fn title(&self) -> String;
    fn ensure(&self, ensure: Ensure);
}

#[derive(Debug, Default)]
pub enum Ensure {
    #[default]
    Present,
    Absent,
}
