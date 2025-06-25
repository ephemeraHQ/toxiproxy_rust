//! Represents a [Proxy] - a connection to a service. Connection reliability can be set by
//! specifying a [`Toxic`] on it.
//!
//! [Proxy]: https://github.com/Shopify/toxiproxy#2-populating-toxiproxy
//! [`Toxic`]: toxic.ToxicPack.html

use super::consts::*;
use super::http_client::*;
use super::toxic::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

/// Raw info about a Proxy.
#[derive(Serialize, Deserialize, Debug)]
pub struct ProxyPack {
    pub name: String,
    pub listen: String,
    pub upstream: String,
    pub enabled: bool,
    pub toxics: Vec<ToxicPack>,
}

impl ProxyPack {
    /// Create a new Proxy configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// let proxy_pack = toxiproxy_rust::proxy::ProxyPack::new(
    ///     "socket".into(),
    ///     "localhost:2001".into(),
    ///     "localhost:2000".into(),
    /// );
    /// ```
    pub async fn new(name: String, listen: String, upstream: String) -> Self {
        Self {
            name,
            listen,
            upstream,
            enabled: true,
            toxics: vec![],
        }
    }
}

/// Client handler of the Proxy object.
#[derive(Debug)]
pub struct Proxy {
    pub proxy_pack: ProxyPack,
    client: Arc<RwLock<HttpClient>>,
}

impl Proxy {
    pub(crate) fn new(proxy_pack: ProxyPack, client: Arc<RwLock<HttpClient>>) -> Self {
        Self { proxy_pack, client }
    }

    /// Disables the proxy - making all connections running through them fail immediately.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY.find_proxy("socket").unwrap().disable();
    /// ```
    pub async fn disable(&self) -> Result<(), String> {
        let mut payload: HashMap<String, bool> = HashMap::new();
        payload.insert("enabled".into(), false);
        let body = serde_json::to_string(&payload).map_err(|_| ERR_JSON_SERIALIZE)?;

        self.update(body).await
    }

    /// Enables the proxy.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY.find_proxy("socket").unwrap().enable();
    /// ```
    pub async fn enable(&self) -> Result<(), String> {
        let mut payload: HashMap<String, bool> = HashMap::new();
        payload.insert("enabled".into(), true);
        let body = serde_json::to_string(&payload).map_err(|_| ERR_JSON_SERIALIZE)?;

        self.update(body).await
    }

    async fn update(&self, payload: String) -> Result<(), String> {
        let path = format!("proxies/{}", self.proxy_pack.name);

        self.client
            .read()
            .await
            .post_with_data(&path, payload)
            .await
            .map(|_| ())
    }

    /// Removes the proxy and all of its toxics.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY.find_proxy("socket").unwrap().delete();
    /// ```
    pub async fn delete(&self) -> Result<(), String> {
        let path = format!("proxies/{}", self.proxy_pack.name);

        self.client.read().await.delete(&path).await.map(|_| ())
    }

    /// Retrieve all toxics registered on the proxy.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// let toxics = toxiproxy_rust::TOXIPROXY.find_proxy("socket").unwrap().toxics().unwrap();
    /// ```
    pub async fn toxics(&self) -> Result<Vec<ToxicPack>, String> {
        let path = format!("proxies/{}/toxics", self.proxy_pack.name);

        let response = self.client.read().await.get(&path).await?;
        response
            .json()
            .await
            .map_err(|err| format!("json deserialize failed: {}", err))
    }

    /// Registers a [latency] Toxic.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .with_latency("downstream".into(), 2000, 0, 1.0);
    /// ```
    ///
    /// [latency]: https://github.com/Shopify/toxiproxy#latency
    pub async fn with_latency(
        &self,
        stream: String,
        latency: ToxicValueType,
        jitter: ToxicValueType,
        toxicity: f32,
    ) -> &Self {
        let mut attributes = HashMap::new();
        attributes.insert("latency".into(), latency);
        attributes.insert("jitter".into(), jitter);

        self.create_toxic(ToxicPack::new(
            "latency".into(),
            stream,
            toxicity,
            attributes,
        ))
        .await
    }

    /// Registers a [bandwith] Toxic.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .with_bandwidth("downstream".into(), 500, 1.0);
    /// ```
    ///
    /// [bandwith]: https://github.com/Shopify/toxiproxy#bandwith
    pub async fn with_bandwidth(
        &self,
        stream: String,
        rate: ToxicValueType,
        toxicity: f32,
    ) -> &Self {
        let mut attributes = HashMap::new();
        attributes.insert("rate".into(), rate);

        self.create_toxic(ToxicPack::new(
            "bandwidth".into(),
            stream,
            toxicity,
            attributes,
        ))
        .await
    }

    /// Registers a [slow_close] Toxic.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .with_slow_close("downstream".into(), 500, 1.0);
    /// ```
    ///
    /// [slow_close]: https://github.com/Shopify/toxiproxy#slow_close
    pub async fn with_slow_close(
        &self,
        stream: String,
        delay: ToxicValueType,
        toxicity: f32,
    ) -> &Self {
        let mut attributes = HashMap::new();
        attributes.insert("delay".into(), delay);

        self.create_toxic(ToxicPack::new(
            "slow_close".into(),
            stream,
            toxicity,
            attributes,
        ))
        .await
    }

    /// Registers a [timeout] Toxic.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .with_timeout("downstream".into(), 5000, 1.0);
    /// ```
    ///
    /// [timeout]: https://github.com/Shopify/toxiproxy#timeout
    pub async fn with_timeout(
        &self,
        stream: String,
        timeout: ToxicValueType,
        toxicity: f32,
    ) -> &Self {
        let mut attributes = HashMap::new();
        attributes.insert("timeout".into(), timeout);

        self.create_toxic(ToxicPack::new(
            "timeout".into(),
            stream,
            toxicity,
            attributes,
        ))
        .await
    }

    /// Registers a [slicer] Toxic.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .with_slicer("downstream".into(), 1024, 128, 500, 1.0);
    /// ```
    ///
    /// [slicer]: https://github.com/Shopify/toxiproxy#slicer
    pub async fn with_slicer(
        &self,
        stream: String,
        average_size: ToxicValueType,
        size_variation: ToxicValueType,
        delay: ToxicValueType,
        toxicity: f32,
    ) -> &Self {
        let mut attributes = HashMap::new();
        attributes.insert("average_size".into(), average_size);
        attributes.insert("size_variation".into(), size_variation);
        attributes.insert("delay".into(), delay);

        self.create_toxic(ToxicPack::new(
            "slicer".into(),
            stream,
            toxicity,
            attributes,
        ))
        .await
    }

    /// Registers a [limit_data] Toxic.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .with_limit_data("downstream".into(), 2048, 1.0);
    /// ```
    ///
    /// [limit_data]: https://github.com/Shopify/toxiproxy#limit_data
    pub async fn with_limit_data(
        &self,
        stream: String,
        bytes: ToxicValueType,
        toxicity: f32,
    ) -> &Self {
        let mut attributes = HashMap::new();
        attributes.insert("bytes".into(), bytes);

        self.create_toxic(ToxicPack::new(
            "limit_data".into(),
            stream,
            toxicity,
            attributes,
        ))
        .await
    }

    pub async fn create_toxic(&self, toxic: ToxicPack) -> &Self {
        let body = serde_json::to_string(&toxic).expect(ERR_JSON_SERIALIZE);
        let path = format!("proxies/{}/toxics", self.proxy_pack.name);

        let _ = self
            .client
            .read()
            .await
            .post_with_data(&path, body)
            .await
            .map_err(|err| {
                panic!("<proxies>.<toxics> creation has failed: {}", err);
            });

        self
    }

    /// Runs a call as if the proxy was [disabled].
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .with_down(|| {
    ///     /* Example test:
    ///        let service_result = MyService::Server::call(params);
    ///        assert!(service_result.is_err());
    ///     */
    ///   });
    /// ```
    ///
    /// [disabled]: https://github.com/Shopify/toxiproxy#down
    pub async fn with_down<F>(&self, closure: F) -> Result<(), String>
    where
        F: FnOnce(),
    {
        self.disable().await?;
        closure();
        self.enable().await
    }

    /// Runs a call with the current Toxic setup for the proxy.
    /// It restores proxy state after the call.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .with_limit_data("downstream".into(), 2048, 1.0)
    ///   .apply(|| {
    ///     /* Example test:
    ///        let service_result = MyService::Server::call(giant_payload);
    ///        assert!(service_result.is_err());
    ///
    ///        let service_result = MyService::Server::call(small_payload);
    ///        assert!(service_result.is_ok());
    ///     */
    ///   });
    /// ```
    pub async fn apply<F>(&self, closure: F) -> Result<(), String>
    where
        F: FnOnce(),
    {
        closure();
        self.delete_all_toxics().await
    }

    /// Deletes all toxics on the proxy.
    ///
    /// # Examples
    ///
    /// ```
    /// # toxiproxy_rust::TOXIPROXY.populate(vec![toxiproxy_rust::proxy::ProxyPack::new(
    /// #    "socket".into(),
    /// #    "localhost:2001".into(),
    /// #    "localhost:2000".into(),
    /// # )]);
    /// toxiproxy_rust::TOXIPROXY
    ///   .find_proxy("socket")
    ///   .unwrap()
    ///   .delete_all_toxics();
    /// ```
    pub async fn delete_all_toxics(&self) -> Result<(), String> {
        match self.toxics().await {
            Ok(toxic_list) => {
                for toxic in toxic_list {
                    let path = format!("proxies/{}/toxics/{}", self.proxy_pack.name, toxic.name);
                    self.client.read().await.delete(&path).await?;
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    }
}
