use chrono::prelude::*;
use std::{
    env, 
    fs::File,
    io::BufReader,
    collections::BTreeMap
};
use web3::{
    transports::WebSocket,
    contract::{Contract, Options}, 
    types::{
        BlockId,
        BlockNumber,
        TransactionId,
        U64,
        H160,
        U256
    },
    helpers as w3h,
};

#[tokio::main]
async fn main() {
    // read env vars
    dotenv::dotenv().ok();

    let file = File::open("src/signatures.json").unwrap();
    let reader = BufReader::new(file);
    let code_sig_lookup: BTreeMap<String, Vec<String>> = serde_json::from_reader(reader).unwrap();

    // connect to Eth node
    let websocket = WebSocket::new(&env::var("GORLI")
                    .unwrap())
                    .await
                    .unwrap();
    let w3sock = web3::Web3::new(websocket);

    // read the latest block info
    let latest_block = w3sock
                    .eth()
                    .block(BlockId::Number(BlockNumber::Latest))
                    .await
                    .unwrap()
                    .unwrap();

    let timestamp = latest_block.timestamp.as_u64() as i64;
    let naive = NaiveDateTime::from_timestamp_opt(timestamp, 0).unwrap();
    let utc_dt: DateTime<Utc> = DateTime::from_utc(naive, Utc);

    println!(
        "UTC: [{}], block number: {}, parent hash: {}, transactions: {}, gas used: {}, gas limit: {}, base fee: {}, difficulty: {}, total difficulty: {} ",
        utc_dt.format("%Y-%m-%d %H:%M:%S"),
        latest_block.number.unwrap(),
        latest_block.parent_hash,
        latest_block.transactions.len(),
        latest_block.gas_used,
        latest_block.gas_limit,
        latest_block.base_fee_per_gas.unwrap(),
        latest_block.difficulty,
        latest_block.total_difficulty.unwrap()
    );

    // loop thr trans-ion hash-s & get data
    for tx_hash in latest_block.transactions {
        let tx = match w3sock
                        .eth()
                        .transaction(TransactionId::Hash(tx_hash))
                        .await
                    {
                        Ok(Some(tx)) => tx,
                        _ => {
                            println!("Err occurred");
                            continue;
                        }
                    };
        let from_addr = tx.from.unwrap_or(H160::zero());
        let to_addr = tx.to.unwrap_or(H160::zero());
        let eth_val = wei_to_eth(tx.value);
        println!(
            " [{}] from: {}, to: {}, value: {}, gas: {}, gas price: {:?} ",
            tx.transaction_index.unwrap_or(U64::from(0)),
            w3h::to_string(&from_addr),
            w3h::to_string(&to_addr),
            eth_val,
            tx.gas,
            tx.gas_price,
        );

        let smart_contr_addr = match tx.to {
            Some(addr) => match w3sock.eth().code(addr, None).await {
                Ok(code) => {
                    if code == web3::types::Bytes::from([]) {
                        println!("Empty code, skipping... ");
                        continue;
                    } else {
                        println!("Non-empty code, returning address... ");
                        addr
                    }
                }
                _ => {
                    println!("Unable to get data, skipping... ");
                    continue;
                }
            },
            _ => {
                println!("To address is not a valid address, skipping... ");
                continue;
            }
        };

        let smart_contr = match Contract::from_json(
            w3sock.eth(),
            smart_contr_addr,
            include_bytes!("erc_abi20.json")
        ) {
            Ok(contract) => contract,
            _ => {
                println!("Failed to init contract, skipping... ");
                continue;
            }
        };

        let token_name: String = match smart_contr
            .query("name", (), None, Options::default(), None)
            .await
        {
            Ok(result) => result,
            Err(err) => {
                println!("Err: {:?}", err);
                continue;
            }
        };

        println!(
            " [{}] ({}) from {}, to {}, value {}, gas {}, gas price {:?} ",
            tx.transaction_index.unwrap_or(U64::from(0)),
            &token_name,
            w3h::to_string(&from_addr),
            w3h::to_string(&to_addr),
            eth_val,
            tx.gas,
            tx.gas_price,
        );

        let input_str: String = w3h::to_string(&tx.input);
        if input_str.len() < 12 {
            continue;
        }
        let func_code = input_str[3..11].to_string();
        let func_sig: String = match code_sig_lookup.get(&func_code) {
            Some(func_sig) => format!("{:?}", func_sig),
            _ => {
                println!("Function not found.");
                "[unknown]".to_string()
            }
        };

        println!(
            " [{}] ({} -> {}) from {}, to {}, value {}, gas {}, gas price {:?} ",
            tx.transaction_index.unwrap_or(U64::from(0)),
            &token_name,
            &func_sig,
            w3h::to_string(&from_addr),
            w3h::to_string(&to_addr),
            eth_val,
            tx.gas,
            tx.gas_price,
        );
    }
}

// helper fn - converter 
fn wei_to_eth(wei_val: U256) -> f64 {
    let result = wei_val.as_u128() as f64;
    result / 1_000_000_000_000_000_000.0
}