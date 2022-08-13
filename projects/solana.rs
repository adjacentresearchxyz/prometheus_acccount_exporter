use clap::{crate_authors, crate_name, crate_version, Arg};
use log::{info, trace};
use prometheus_exporter::prelude::*;
use std::{
    fs::read_dir,
    env,
    io::{
        self, 
        Read
    },
};
use hyper::body;
use hyper::{Body, Client, Response};
use hyper_tls::HttpsConnector;
use serde::{
    self,
    Deserialize, 
    Serialize,
    de::DeserializeOwned,
};
use serde_json::{
    self,
    from_str,
    Deserializer,
};

// Structs for defining JSON objects
#[derive(Debug, Clone, Default)]
struct MyOptions { }

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct SolanaAccount {
    pub lamports: u64,
    pub ownerProgram: String,
    pub rentEpoch: u64,
    pub executable: bool,
    pub account: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct SolanaTokenAccount {
    pub tokens: Vec<Token>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct Token {
    pub tokenAddress: String,
    pub tokenAmount: TokenAmount,
    pub tokenAccount: String,
    pub tokenName: String,
    pub tokenIcon: String,
    pub rentEpoch: u64,
    pub lamports: u64,
    // pub tokenSymbol: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
struct TokenAmount {
    pub amount: String,
    pub decimals: u8,
    pub uiAmount: f64,
    pub uiAmountString: String,
}

// Implementing a custom deserializer for the JSON response from the API
// https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=0ecac4807f16d5563db32c4f3f6a9c65
fn read_skipping_ws(mut reader: impl Read) -> io::Result<u8> {
    loop {
        let mut byte = 0u8;
        reader.read_exact(std::slice::from_mut(&mut byte))?;
        if !byte.is_ascii_whitespace() {
            return Ok(byte);
        }
    }
}

fn invalid_data(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn deserialize_single<T: DeserializeOwned, R: Read>(reader: R) -> io::Result<T> {
    let next_obj = Deserializer::from_reader(reader).into_iter::<T>().next();
    match next_obj {
        Some(result) => result.map_err(Into::into),
        None => Err(invalid_data("premature EOF")),
    }
}

fn yield_next_obj<T: DeserializeOwned, R: Read>(
    mut reader: R,
    at_start: &mut bool,
) -> io::Result<Option<T>> {
    if !*at_start {
        *at_start = true;
        if read_skipping_ws(&mut reader)? == b'[' {
            // read the next char to see if the array is empty
            let peek = read_skipping_ws(&mut reader)?;
            if peek == b']' {
                Ok(None)
            } else {
                deserialize_single(io::Cursor::new([peek]).chain(reader)).map(Some)
            }
        } else {
            Err(invalid_data("`[` not found"))
        }
    } else {
        match read_skipping_ws(&mut reader)? {
            b',' => deserialize_single(reader).map(Some),
            b']' => Ok(None),
            _ => Err(invalid_data("`,` or `]` not found")),
        }
    }
}

pub fn iter_json_array<T: DeserializeOwned, R: Read>(
    mut reader: R,
) -> impl Iterator<Item = Result<T, io::Error>> {
    let mut at_start = false;
    std::iter::from_fn(move || yield_next_obj(&mut reader, &mut at_start).transpose())
}

// Function to get the balance of a solana account
async fn get_solana_balance(address: &str) -> Result<SolanaAccount, std::io::Error> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, Body>(https);

    let url = format!("https://public-api.solscan.io/account/{}", address);
    let uri = match url.parse::<hyper::Uri>() {
        Ok(uri) => uri,
        Err(e) => {
            println!("{}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Invalid URL"));
        }
    };

    let mut res = match client.get(uri).await 
    {
        Ok(res) => res,
        Err(e) => {
            println!("{}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Request failed"));
        }
    };
    
    let body_bytes = match body::to_bytes(res.into_body()).await {
        Ok(bytes) => bytes,
        Err(e) => {
            println!("{}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to read body"));
        }
    };
    let body = String::from_utf8(body_bytes.to_vec()).expect("response was not valid utf-8");
    let solana_account: SolanaAccount = serde_json::from_str::<SolanaAccount>(&body).unwrap();

    Ok(solana_account)
}

// Function to get the token balance of a Solana account
async fn get_token_balances(address: &str) -> Result<SolanaTokenAccount, std::io::Error> {
    let https = HttpsConnector::new();
    let client = Client::builder().build::<_, Body>(https);

    let url = format!("https://public-api.solscan.io/account/tokens?account={}", address);
    let uri = match url.parse::<hyper::Uri>() {
        Ok(uri) => uri,
        Err(e) => {
            println!("{}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Invalid URL"));
        }
    };

    let mut res = match client.get(uri).await 
    {
        Ok(res) => res,
        Err(e) => {
            println!("{}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Request failed"));
        }
    };
    
    let body_bytes = match body::to_bytes(res.into_body()).await {
        Ok(bytes) => bytes,
        Err(e) => {
            println!("{}", e);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "Failed to read body"));
        }
    };

    let body = String::from_utf8(body_bytes.to_vec()).expect("response was not valid utf-8");
    let mut tokens = vec![];
    for user in iter_json_array(io::Cursor::new(&body)) {
        let user: Token = user.unwrap();
        tokens.push(user);
    }
    
     let solana_token_accounts = SolanaTokenAccount {
        tokens: tokens,
    };

    Ok(solana_token_accounts)
}

#[tokio::main]
async fn main() {
    // Define addresses here 
    let addresses = vec![
        "<address>",
    ];

    let matches = clap::App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .arg(
            Arg::with_name("port")
                .short("p")
                .help("exporter port")
                .default_value("32148")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("verbose")
                .short("v")
                .help("verbose logging")
                .takes_value(false),
        )
        .get_matches();

    if matches.is_present("verbose") {
        env::set_var(
            "RUST_LOG",
            format!("folder_size=trace,{}=trace", crate_name!()),
        );
    } else {
        env::set_var(
            "RUST_LOG",
            format!("folder_size=info,{}=info", crate_name!()),
        );
    }
    env_logger::init();

    info!("using matches: {:?}", matches);

    let bind = matches.value_of("port").unwrap();
    let bind = u16::from_str_radix(&bind, 10).expect("port must be a valid number");
    let addr = ([0, 0, 0, 0], bind).into();

    info!("starting exporter on {}", addr);

    render_prometheus(addr, MyOptions::default(), |request, options| async move {
        trace!(
            "in our render_prometheus(request == {:?}, options == {:?})",
            request,
            options
        );

        let mut pc = PrometheusMetric::build()
            .with_name("account_balance")
            .with_metric_type(MetricType::Counter)
            .with_help("Solana account balance")
            .build();

        for address in addresses {
            pc.render_and_append_instance(
                &PrometheusInstance::new()
                    .with_label("address", address.as_ref())
                    .with_value(get_solana_balance(address).await.unwrap().lamports as f64 / 1e9) // returning the balance of the account
                    .with_current_timestamp()
                    .expect("error getting the current UNIX epoch"),
            );
            
            let solana_token_accounts = get_token_balances(address).await.unwrap();
            for token_account in solana_token_accounts.tokens {
                pc.render_and_append_instance(
                    &PrometheusInstance::new()
                        .with_label("address", token_account.tokenAddress.as_ref())
                        .with_label("token", token_account.tokenName.as_ref())
                        .with_value(token_account.tokenAmount.uiAmount) // returning the balance of the token account
                        .with_current_timestamp()
                        .expect("error getting the current UNIX epoch"),
                );
            }
        }

        Ok(pc.render())
    })
    .await;
}
