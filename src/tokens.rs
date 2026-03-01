use std::{
    collections::BTreeMap,
    fs::OpenOptions,
    io::Write,
    path::Path,
};
use alloy::{
    primitives::Address,
    providers::Provider,
};
use indicatif::{ProgressBar, ProgressStyle};
use anyhow::{anyhow, Result};
use log::info;
use serde::{Serialize, Deserialize};

use crate::{interfaces::IERC20, pools::Pool};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub address: Address,
    pub name: String,
    pub symbol: String,
    pub decimals: u8
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokensToml {
    token: Vec<Token>,
}

pub async fn load_tokens(
    provider: impl Provider + 'static + Clone,
    path: &Path,
    pools: &BTreeMap<Address, Pool>,
    parallel: u64,
    last_pool_id: i64,
) -> Result<BTreeMap<Address, Token>> {

    info!("Loading tokens...");

    let mut tokens = match load_tokens_from_file(path) {
        Ok(t) => t,
        Err(e) => {
            info!("Failed to load tokens from file: {:?}. Starting with empty token list.", e);
            BTreeMap::new()
        }
    };


    let pb = ProgressBar::new(pools.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );

    let mut count = 1;
    let mut requests = Vec::new();

    for (_, pool) in pools.into_iter() {
        if pool.id <= last_pool_id {
            pb.inc(1);
            continue;
        }
        let token0 = pool.token0;
        let token1 = pool.token1;
        for token in [token0, token1] {
            if !tokens.contains_key(&token) {
                requests.push(
                    tokio::task::spawn(
                        get_token_data(
                            provider.clone(),
                            token,
                        )
                    )
                );
                count += 1 ;
            }
            if count == parallel {
                let results = futures::future::join_all(requests).await;
                for result in results {
                    match result {
                        Ok(r) => match r {
                            Ok(t) => {
                                tokens.insert(
                                    t.address,
                                    Token {
                                        address: t.address,
                                        name: t.name,
                                        symbol: t.symbol,
                                        decimals: t.decimals
                                    }
                                );
                            }
                            Err(e) => { info!("Error getting token data {:?}", e) }
                        }
                        Err(e) => { info!("Error getting token data {:?}", e) }
                    }
                }
                requests = Vec::new();
                count = 0;
                pb.inc(parallel);
            }
        }
    }

    write_tokens_to_toml(&tokens, path)?;

    Ok(tokens)
}

async fn get_token_data(
    provider: impl Provider,
    token: Address,
) -> Result<Token> {

    let interface = IERC20::new(token, &provider);

    let multicall = provider
        .multicall()
        .add(interface.decimals())
        .add(interface.name())
        .add(interface.symbol());

    let (decimals_result, name_result, symbol_result) = match multicall.aggregate().await {
        Ok(r) => r,
        Err(e) => { return Err(anyhow!("Multicall for token data failed {:?}", e )) }
    };

    Ok(Token {
        address: token,
        name: name_result,
        symbol: symbol_result,
        decimals: decimals_result,
    })
}

pub fn load_tokens_from_file(
    path: &Path,
) -> Result<BTreeMap<Address, Token>> {
    let mut tokens = BTreeMap::new();

    if path.exists() {
        let reader = match std::fs::read_to_string(path) {
            Ok(r) => r,
            Err(e) => { return Err(anyhow!("Failed to read tokens TOML file: {:?}", e)) }
        };
        let tokens_toml: TokensToml = match toml::from_str(&reader) {
            Ok(t) => t,
            Err(e) => { return Err(anyhow!("Failed to parse tokens TOML file: {:?}", e)) }
        };
        for token in tokens_toml.token {
            tokens.insert(token.address, token);
        }
    }

    Ok(tokens)
}

pub fn write_tokens_to_toml(
    tokens: &BTreeMap<Address, Token>,
    path: &Path,
) -> Result<()> {
    let tokens_vec: Vec<Token> = tokens.values().cloned().collect();
    let tokens_toml = TokensToml { token: tokens_vec };

    let toml_string = match toml::to_string_pretty(&tokens_toml) {
        Ok(s) => s,
        Err(e) => { return Err(anyhow!("Failed to serialize tokens to TOML: {:?}", e)) }
    };

    let mut file = match OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path) {
            Ok(f) => f,
            Err(e) => { return Err(anyhow!("Failed to open file for writing tokens: {:?}", e)) }
        };

    match file.write_all(toml_string.as_bytes()) {
        Ok(_) => (),
        Err(e) => { return Err(anyhow!("Failed to write tokens to file: {:?}", e)) }
    };

    Ok(())
}
