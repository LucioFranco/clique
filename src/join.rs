use crate::{
    cluster::Cluster,
    common::{Endpoint, NodeId},
    error::{Error, Result},
    transport::{proto, Request, Response, Transport},
};
use futures::future;
use std::collections::HashMap;

pub struct Join<T, Target> {
    transport: T,
    endpoint: Endpoint,
    node_id: NodeId,
    listen_target: Target,
}

impl<T, Target> Join<T, Target>
where
    T: Transport<Target> + Send,
    Target: Into<Endpoint> + Send + Clone,
{
    async fn join_attempt(&mut self, seed_addr: Endpoint) -> Result<Cluster<T, Target>> {
        let req = proto::RequestKind::PreJoin(proto::PreJoinMessage {
            sender: self.endpoint.clone(),
            node_id: self.node_id.clone(),
            ring_number: vec![],
            config_id: None,
        });

        let join_res = match self
            .transport
            .send(Request::new(seed_addr, req))
            .await
            .map_err(|e| Error::new_broken_pipe(Some(Box::new(e))))?
            .into_inner()
        {
            proto::ResponseKind::Join(res) => res,
            _ => return Err(Error::new_join_phase1()),
        };

        if join_res.status != proto::JoinStatus::SafeToJoin
            && join_res.status != proto::JoinStatus::HostnameAlreadyInRing
        {
            return Err(Error::new_join_phase1());
        }

        let config_to_join = if join_res.status == proto::JoinStatus::HostnameAlreadyInRing {
            -1
        } else {
            join_res.config_id
        };

        let res = self
            .send_join_phase2(join_res)
            // TODO: probably want to make this a stream
            .await?
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|r| {
                if let proto::ResponseKind::Join(res) = r.into_inner() {
                    Some(res)
                } else {
                    None
                }
            })
            .filter(|r| r.status == proto::JoinStatus::SafeToJoin)
            .filter(|r| r.config_id != config_to_join)
            .take(1)
            .next();

        if let Some(join) = res {
            Cluster::from_join(self.listen_target.clone().into(), join)
        } else {
            Err(Error::new_join_phase2())
        }
    }

    async fn send_join_phase2(
        &mut self,
        join_res: proto::JoinResponse,
    ) -> Result<Vec<Result<Response>>> {
        let mut ring_num_per_obs = HashMap::new();

        for (ring_num, obs) in join_res.endpoints.iter().enumerate() {
            ring_num_per_obs
                .entry(obs)
                .or_insert_with(Vec::new)
                .push(ring_num as i32);
        }

        let mut in_flight_futs = Vec::new();

        for (endpoint, ring_nums) in ring_num_per_obs {
            let join = proto::RequestKind::Join(proto::JoinMessage {
                sender: self.endpoint.clone(),
                node_id: self.node_id.clone(),
                ring_number: ring_nums,
                config_id: join_res.config_id,
                // TODO: add metadata to the cluster
                metadata: None,
            });

            let fut = self.transport.send(Request::new(endpoint.clone(), join));
            in_flight_futs.push(fut);
        }

        let responses = future::join_all(in_flight_futs)
            .await
            .into_iter()
            .map(|r| match r {
                Ok(r) => Ok(r),
                Err(e) => Err(Error::new_broken_pipe(Some(Box::new(e)))),
            })
            .collect();

        Ok(responses)
    }
}
