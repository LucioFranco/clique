use crate::{
    cluster::Cluster,
    common::{Endpoint, NodeId},
    error::{Error, Result},
    transport::{proto, Message, Transport2},
};
use std::collections::HashMap;

pub struct Join<T, Target> {
    transport: T,
    endpoint: Endpoint,
    node_id: NodeId,
    listen_target: Target,
}

impl<T, Target> Join<T, Target>
where
    T: Transport2 + Send,
    Target: Into<Endpoint> + Send + Clone,
{
    async fn join_attempt(&mut self, seed_addr: Endpoint) -> Result<Cluster<T>> {
        let req = proto::RequestKind::PreJoin(proto::PreJoinMessage {
            sender: self.endpoint.clone(),
            node_id: self.node_id.clone(),
            ring_number: vec![],
            config_id: None,
        });

        self.transport
            .send_to(seed_addr.into(), req.into())
            .await
            .map_err(|e| Error::new_broken_pipe(Some(e.into())))?;

        let join_res = match self
            .transport
            .recv()
            .await
            .map_err(|e| Error::new_broken_pipe(Some(e.into())))?
        {
            // TODO: check if this is coming from the correct seed_addr
            (_, Message::Response(proto::ResponseKind::Join(res))) => res,
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

        let res = self.send_join_phase2(join_res).await?;

        Cluster::from_join(self.listen_target.clone().into(), res)
    }

    async fn send_join_phase2(
        &mut self,
        join_res: proto::JoinResponse,
    ) -> Result<proto::JoinResponse> {
        let mut ring_num_per_obs = HashMap::new();

        for (ring_num, obs) in join_res.endpoints.iter().enumerate() {
            ring_num_per_obs
                .entry(obs)
                .or_insert_with(Vec::new)
                .push(ring_num as i32);
        }

        for (endpoint, ring_nums) in ring_num_per_obs {
            let join = proto::RequestKind::Join(proto::JoinMessage {
                sender: self.endpoint.clone(),
                node_id: self.node_id.clone(),
                ring_number: ring_nums,
                config_id: join_res.config_id,
                // TODO: add metadata to the cluster
                metadata: None,
            });

            self.transport
                .send_to(endpoint.clone(), join.into())
                .await
                .map_err(Into::into)
                .unwrap();
        }

        loop {
            let (from, msg) = self.transport.recv().await.map_err(Into::into).unwrap();

            if let Message::Response(proto::ResponseKind::Join(join)) = msg {
                if join.status == proto::JoinStatus::SafeToJoin
                    && join.config_id != join_res.config_id
                {
                    continue;
                }

                return Ok(join);
            }
        }
    }
}
