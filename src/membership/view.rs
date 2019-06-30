#[derive(Debug, Clone)]
pub struct View {
    configuration: Configuration,
}

#[derive(Debug, Clone)]
struct Configuration {
    _p: (),
}

impl View {
    pub fn new() -> Self {
        unimplemented!()
    }
}
