use std::{
    collections::{BTreeSet, HashMap},
    path::PathBuf,
    sync::{Arc, Weak},
};

use anyhow::{bail, Result};
use balius_runtime::{ledgers::Ledger, Response, Runtime, Store};
use dashmap::DashMap;
use ledger::BlockfrostLedger;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::{
    fs,
    sync::{Mutex, RwLock},
};
use tracing::error;
use u5c::convert_block;

use crate::{
    blockfrost::BlockfrostClient,
    streams::{BlockInfo, BlockReference, Event, Listener, ListenerFilter, ListenerId},
};

mod ledger;
mod u5c;

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractsConfig {
    pub components_path: PathBuf,
    pub stores_path: PathBuf,
    pub cache_size: Option<usize>,
}

pub struct ContractManager {
    config: Option<ContractsConfig>,
    ledger: Option<Ledger>,
    invoke_runtime: RwLock<Option<Runtime>>,
    listener_runtimes: DashMap<ListenerId, ContractListenerHandle>,
}

impl ContractManager {
    pub async fn new(
        config: &ContractsConfig,
        blockfrost: Option<BlockfrostClient>,
    ) -> Result<Self> {
        fs::create_dir_all(&config.components_path).await?;
        let ledger = blockfrost.map(|client| {
            let ledger = BlockfrostLedger::new(client);
            Ledger::Custom(Arc::new(Mutex::new(ledger)))
        });
        let runtime = Self::new_invoke_runtime(config, ledger.clone()).await?;
        Ok(Self {
            config: Some(config.clone()),
            ledger,
            invoke_runtime: RwLock::new(Some(runtime)),
            listener_runtimes: DashMap::new(),
        })
    }

    pub fn none() -> Self {
        Self {
            config: None,
            ledger: None,
            invoke_runtime: RwLock::new(None),
            listener_runtimes: DashMap::new(),
        }
    }

    pub async fn deploy(&self, id: &str, contract: &[u8]) -> Result<()> {
        let Some(config) = self.config.as_ref() else {
            bail!("No contract directory configured");
        };
        let path = config.components_path.join(format!("{id}.wasm"));
        fs::write(&path, contract).await?;

        // update the invoke runtime
        let mut rt_lock = self.invoke_runtime.write().await;
        *rt_lock = None; // drop the old runtime before creating the new
        *rt_lock = Some(Self::new_invoke_runtime(config, self.ledger.clone()).await?);
        drop(rt_lock);

        // update any listener runtimes
        for entry in self.listener_runtimes.iter() {
            let Some(runtime) = entry.runtime.upgrade() else {
                let key = entry.key().clone();
                self.listener_runtimes.remove(&key);
                continue;
            };
            if !entry.contracts.iter().any(|it| it == id) {
                continue;
            }
            let mut rt_lock = runtime.lock().await;
            *rt_lock = None; // drop the old runtime before creating the new
            *rt_lock = Some(
                Self::new_listen_runtime(
                    config,
                    self.ledger.clone(),
                    entry.key(),
                    &entry.contracts,
                )
                .await?,
            );
        }

        Ok(())
    }

    pub async fn invoke(
        &self,
        contract: &str,
        method: &str,
        params: Value,
    ) -> Result<Option<Vec<u8>>> {
        let params = serde_json::to_vec(&params)?;
        let rt_lock = self.invoke_runtime.read().await;
        let Some(runtime) = rt_lock.as_ref() else {
            bail!("Contract manager not configured")
        };
        let response = runtime.handle_request(contract, method, params).await?;
        match response {
            Response::PartialTx(bytes) => Ok(Some(bytes)),
            _ => Ok(None),
        }
    }

    pub async fn listen(&self, listener: &Listener) -> ContractListener {
        let contracts = find_contract_names(&listener.filters);
        let runtime = match self.config.as_ref() {
            Some(config) => {
                Self::new_listen_runtime(config, self.ledger.clone(), &listener.id, &contracts)
                    .await
                    .ok()
            }
            None => None,
        };
        let runtime = Arc::new(Mutex::new(runtime));

        let handle = ContractListenerHandle {
            contracts,
            runtime: Arc::downgrade(&runtime),
        };

        self.listener_runtimes.insert(listener.id.clone(), handle);
        ContractListener::new(runtime)
    }

    async fn new_invoke_runtime(
        config: &ContractsConfig,
        ledger: Option<Ledger>,
    ) -> Result<Runtime> {
        let mut runtime = Self::new_runtime(config, ledger, "invoke")?;
        let mut entries = fs::read_dir(&config.components_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let extless = entry.path().with_extension("");
            let Some(id) = extless.file_name().and_then(|s| s.to_str()) else {
                bail!("invalid file name {:?}", entry.file_name().into_string());
            };
            let wasm_path = entry.path();

            runtime.register_worker(id, wasm_path, json!(null)).await?;
        }
        Ok(runtime)
    }

    async fn new_listen_runtime(
        config: &ContractsConfig,
        ledger: Option<Ledger>,
        listener: &ListenerId,
        contracts: &[String],
    ) -> Result<Runtime> {
        let mut runtime = Self::new_runtime(config, ledger, listener.to_string())?;
        for contract in contracts {
            let wasm_path = config.components_path.join(contract).with_extension("wasm");
            runtime
                .register_worker(contract, wasm_path, json!(null))
                .await?;
        }
        Ok(runtime)
    }

    fn new_runtime(
        config: &ContractsConfig,
        ledger: Option<Ledger>,
        store_name: impl AsRef<str>,
    ) -> Result<Runtime> {
        let store_path = config
            .stores_path
            .join(store_name.as_ref())
            .with_extension("wasm");
        let store = Store::open(&store_path, config.cache_size)?;
        let mut runtime_builder = Runtime::builder(store);
        if let Some(ledger) = ledger {
            runtime_builder = runtime_builder.with_ledger(ledger)
        }
        Ok(runtime_builder.build()?)
    }
}

pub struct ContractListener {
    runtime: Arc<Mutex<Option<Runtime>>>,
    cache: HashMap<BlockReference, Vec<Event>>,
}

impl ContractListener {
    fn new(runtime: Arc<Mutex<Option<Runtime>>>) -> Self {
        Self {
            runtime,
            cache: HashMap::new(),
        }
    }

    pub async fn gather_events(&self, rollbacks: &[BlockInfo], block: &BlockInfo) {
        let mut lock = self.runtime.lock().await;
        let runtime = lock.as_mut().unwrap();
        let undo_blocks = rollbacks.into_iter().map(|b| convert_block(b)).collect();
        let next_block = convert_block(block);
        if let Err(error) = runtime.handle_chain(&undo_blocks, &next_block).await {
            error!("could not gather events for new blocks: {error}");
        }

        // TODO: actually gather events
    }

    pub async fn events_for(&self, block_ref: &BlockReference) -> Vec<Event> {
        self.cache.get(block_ref).cloned().unwrap_or_default()
    }
}

struct ContractListenerHandle {
    contracts: Vec<String>,
    runtime: Weak<Mutex<Option<Runtime>>>,
}

fn find_contract_names(filters: &[ListenerFilter]) -> Vec<String> {
    let mut result = BTreeSet::new();
    for filter in filters {
        if let ListenerFilter::Event { contract, .. } = filter {
            result.insert(contract.clone());
        }
    }
    result.into_iter().collect()
}
