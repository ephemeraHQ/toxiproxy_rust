//! Main client for communicating with the Toxiproxy server.

use serde_json;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::{collections::HashMap, io::Read};
use tokio::sync::{Mutex, RwLock};

use super::http_client::*;
use super::proxy::*;

/// Server client.
#[derive(Clone)]
pub struct Client {
    client: Arc<RwLock<HttpClient>>,
}

impl Client {
    /// Creates a new client. There is also a prepopulated client, `toxiproxy_rust::TOXIPROXY`
    /// connected to the server's default address.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toxiproxy_rust::client::Client;
    /// let client = Client::new("127.0.0.1:8474");
    /// ```
    pub fn new<U: ToSocketAddrs>(toxiproxy_addr: U) -> Self {
        Self {
            client: Arc::new(RwLock::new(HttpClient::new(toxiproxy_addr))),
        }
    }

    /// Establish a set of proxies to work with.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toxiproxy_rust::client::Client;
    /// # use toxiproxy_rust::proxy::ProxyPack;
    /// let client = Client::new("127.0.0.1:8474");
    /// let proxies = client.populate(vec![ProxyPack::new(
    ///     "socket".into(),
    ///     "localhost:2001".into(),
    ///     "localhost:2000".into(),
    /// )]).expect("populate has completed");
    /// ```
    ///
    /// ```
    /// let proxies = toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    ///     "socket".into(),
    ///     "localhost:2001".into(),
    ///     "localhost:2000".into(),
    /// )]).expect("populate has completed");
    /// ```
    pub async fn populate(&self, proxies: Vec<ProxyPack>) -> Result<Vec<Proxy>, String> {
        let proxies_json = serde_json::to_string(&proxies).unwrap();
        let response = self
            .client
            .read()
            .await
            .post_with_data("populate", proxies_json)
            .await?;

        let mut json = response
            .json::<HashMap<String, Vec<ProxyPack>>>()
            .await
            .map_err(|err| format!("json deserialize failed: {}", err))?;
        let Some(proxy_packs) = json.remove("proxies") else {
            return Ok(vec![]);
        };

        let proxy_packs = proxy_packs
            .into_iter()
            .map(|proxy_pack| Proxy::new(proxy_pack, self.client.clone()))
            .collect::<Vec<Proxy>>();

        Ok(proxy_packs)
    }

    /// Enable all proxies and remove all active toxics.
    ///
    /// # Examples
    ///
    /// ```
    /// # use toxiproxy_rust::client::Client;
    /// # use toxiproxy_rust::proxy::ProxyPack;
    /// let client = Client::new("127.0.0.1:8474");
    /// client.reset();
    /// ```
    ///
    /// ```
    /// toxiproxy_rust::TOXIPROXY.reset();
    /// ```
    pub async fn reset(&self) -> Result<(), String> {
        self.client.read().await.post("reset").await.map(|_| ())
    }

    /// Returns all registered proxies and their toxics.
    ///
    /// # Examples
    ///
    /// ```
    /// let proxies = toxiproxy_rust::TOXIPROXY.all().expect("all proxies were fetched");
    /// ```
    pub async fn all(&self) -> Result<HashMap<String, Proxy>, String> {
        let response = self.client.read().await.get("proxies").await?;

        response
            .json()
            .await
            .map(|proxy_map: HashMap<String, ProxyPack>| {
                proxy_map
                    .into_iter()
                    .map(|(name, proxy_pack)| (name, Proxy::new(proxy_pack, self.client.clone())))
                    .collect()
            })
            .map_err(|err| format!("json deserialize failed: {}", err))
    }

    /// Health check for the Toxiproxy server.
    ///
    /// # Examples
    ///
    /// ```
    /// if !toxiproxy_rust::TOXIPROXY.is_running() {
    ///     /* signal the problem */
    /// }
    /// ```
    pub async fn is_running(&self) -> bool {
        self.client.read().await.is_alive()
    }

    /// Version of the Toxiproxy server.
    ///
    /// # Examples
    ///
    /// ```
    /// let version = toxiproxy_rust::TOXIPROXY.version().expect("version is returned");
    /// ```
    pub async fn version(&self) -> Result<String, String> {
        let response = self.client.read().await.get("version").await?;

        Ok(response.text().await.map_err(|err| format!("{err:?}"))?)
    }

    /// Fetches a proxy a resets its state (remove active toxics). Usually a good way to start a test and to start setting up
    /// toxics fresh against the proxy.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]).unwrap();
    /// let proxy = toxiproxy_rust::TOXIPROXY.find_and_reset_proxy("socket").expect("proxy returned");
    /// ```
    pub async fn find_and_reset_proxy(&self, name: &str) -> Result<Proxy, String> {
        let proxy = self.find_proxy(name).await?;

        proxy.delete_all_toxics().await?;
        proxy.enable().await?;

        Ok(proxy)
    }

    /// Fetches a proxy. Useful to fetch a proxy for a test where more fine grained control is required
    /// over a proxy and its toxics.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]).unwrap();
    /// let proxy = toxiproxy_rust::TOXIPROXY.find_proxy("socket").expect("proxy returned");
    /// ```
    pub async fn find_proxy(&self, name: &str) -> Result<Proxy, String> {
        let path = format!("proxies/{}", name);

        let response = self.client.read().await.get(&path).await?;

        let proxy_pack: ProxyPack = response
            .json()
            .await
            .map_err(|err| format!("json deserialize failed: {}", err))?;
        Ok(Proxy::new(proxy_pack, self.client.clone()))
    }
}
