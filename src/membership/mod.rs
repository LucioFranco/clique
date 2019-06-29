mod view;

use crate::transport::Client;
use view::View;

#[derive(Debug, Clone)]
pub struct Membership<C> {
    client: C,
    view: View,
}

impl<C: Client + Clone> Membership<C> {
    pub fn new(client: C) -> Self {
        unimplemented!()
    }

    pub fn view(&self) -> View {
        self.view.clone()
    }

    pub async fn handle_message(req: Request) -> 
}
