use crate::{Index, LogEntry, Message, Result, StateMachine, Term};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::marker::PhantomData;
use trillium_client::Client;
use trillium_smol::ClientConfig;
use url::Url;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ClientResponse<R> {
    pub result: R,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientRequest<M> {
    pub message: M,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct VoteResponse {
    pub term: Term,
    pub vote_granted: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct VoteRequest {
    pub term: Term,
    pub candidate_id: String,
    pub last_log_index: Option<Index>,
    pub last_log_term: Option<Term>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AppendResponse {
    pub term: Term,
    pub success: bool,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct AppendRequest<M> {
    pub term: Term,
    pub leader_id: String,
    pub previous_log_index: Option<Index>,
    pub previous_log_term: Option<Term>,
    pub entries: Option<Vec<LogEntry<M>>>,
    pub leader_commit_index: Index,
}

#[derive(Debug)]
pub struct RaftClient<S>(Client, PhantomData<S>);

impl<S> Clone for RaftClient<S> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}

impl<S> Default for RaftClient<S> {
    fn default() -> Self {
        Self(Client::new(ClientConfig::new()), PhantomData)
    }
}

impl<S: StateMachine> RaftClient<S> {
    async fn post<T>(&self, url: Url, body: &impl serde::Serialize) -> Result<T>
    where
        T: DeserializeOwned,
    {
        self.0
            .post(url)
            .with_json_body(body)?
            .await?
            .success()?
            .response_json()
            .await
            .map_err(Into::into)
    }

    pub async fn remove(&self, url: &Url, id: &str) -> Result<()> {
        let req_url = url
            .join(&format!("/servers/{}", urlencoding::encode(id)))
            .unwrap();
        let _ = self.0.delete(req_url).await?.success()?;
        Ok(())
    }

    pub async fn ping(&self, url: &Url) -> Result<String> {
        self.0
            .get(url.clone())
            .await?
            .response_body()
            .await
            .map_err(Into::into)
    }

    pub async fn add(&self, url: &Url, id: &str) -> Result<()> {
        let url = url
            .join(&format!("/servers/{}", urlencoding::encode(id)))
            .unwrap();
        let _ = self.0.put(url).await?.success()?;
        Ok(())
    }

    pub async fn client_append(
        &self,
        server: &Url,
        message: &ClientRequest<S::MessageType>,
    ) -> Result<ClientResponse<S::ApplyResult>> {
        self.post(server.join("/client").unwrap(), message).await
    }

    pub async fn request_vote(
        &self,
        server: &str,
        vote_request: &VoteRequest,
    ) -> Result<VoteResponse> {
        let url = Url::parse(server).unwrap().join("/vote").unwrap();
        self.post(url, vote_request).await
    }

    pub async fn append(
        &self,
        server: &str,
        append_request: &AppendRequest<impl Message>,
    ) -> Result<AppendResponse> {
        let url = Url::parse(server).unwrap().join("/append").unwrap();
        self.post(url, append_request).await
    }

    pub fn new() -> Self {
        Self(Client::new(ClientConfig::new()), PhantomData)
    }
}
