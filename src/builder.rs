use crate::{cluster::Cluster, error::Result, transport::Transport};
use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct Builder<T, Target> {
    _pd: PhantomData<(T, Target)>,
}

impl<T, Target> Builder<T, Target>
where
    T: Transport<Target>,
{
    pub fn finish(self) -> Result<Cluster<T, Target>> {
        unimplemented!()
    }
}
