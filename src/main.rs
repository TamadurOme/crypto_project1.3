use reqwest;
use std::collections::HashMap;
use hmac::{Hmac, Mac, NewMac};
use sha2::{Sha256, Sha512};
use base64::{decode, encode};
use serde::Deserialize;
use chrono::Utc;
use tokio;
use std::sync::Arc;
use sha2::Digest;

type HmacSha512 = Hmac<Sha512>;

// Data structure for API response for balance
#[derive(Deserialize, Debug)]
struct BalanceResponse {
    error: Vec<String>,
    result: Option<HashMap<String, String>>,
}

// Data structure for API response for order placement
#[derive(Deserialize, Debug)]
struct OrderResponse {
    error: Vec<String>,
    result: Option<HashMap<String, String>>,
}

// Function to generate signature for API requests
fn generate_signature(api_secret: &str, nonce: &str, endpoint: &str, post_data: &str) -> String {
    // Decode API secret from base64
    let api_secret_decoded = decode(api_secret).expect("Invalid base64 API secret");
    
    // Create SHA256 hash
    let mut sha256 = Sha256::new();
    sha256.update(format!("{}{}", nonce, post_data).as_bytes());
    let hash = sha256.finalize();
    
    // Create HMAC-SHA512 signature
    let mut mac = HmacSha512::new_from_slice(&api_secret_decoded).expect("HMAC can take key of any size");
    mac.update(endpoint.as_bytes());
    mac.update(&hash);
    
    // Encode signature to base64
    encode(mac.finalize().into_bytes())
}

// Function to fetch account balance
async fn fetch_balance(api_key: &str, api_secret: &str) -> Result<Option<HashMap<String, String>>, Box<dyn std::error::Error + Send + Sync>> {
    let url = "https://api.kraken.com/0/private/Balance";
    let endpoint = "/0/private/Balance";
    let client = reqwest::Client::new();
    
    // Generate nonce using current timestamp in milliseconds
    let nonce = format!("{}", Utc::now().timestamp_millis());
    let mut params = HashMap::new();
    params.insert("nonce", nonce.clone());
    
    // Create post data string
    let post_data = format!("nonce={}", nonce);
    
    // Generate API signature
    let api_sign = generate_signature(api_secret, &nonce, endpoint, &post_data);
    
    // Send POST request to fetch balance
    let response = client
        .post(url)
        .header("API-Key", api_key)
        .header("API-Sign", api_sign)
        .form(&params)
        .send()
        .await?;
    
    // Print response status and body for debugging
    let status = response.status();
    let body = response.text().await?;
    println!("Response status: {}", status);
    println!("Response body: {}", body);

    // Process the response if successful
    if status.is_success() {
        let balance: BalanceResponse = serde_json::from_str(&body)?;
        if balance.error.is_empty() {
            if let Some(result) = balance.result {
                println!("Balance fetched successfully");
                for (currency, amount) in result.iter() {
                    println!("{}: {}", currency, amount);
                }
                return Ok(Some(result));
            } else {
                println!("No balance data found.");
            }
        } else {
            println!("API returned errors: {:?}", balance.error);
        }
    } else {
        println!("Request failed with status code: {}", status);
    }
    Ok(None)
}

// Function to place a market sell order for USD
async fn place_market_order_usd(api_key: Arc<String>, api_secret: Arc<String>, volume: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = "https://api.kraken.com/0/private/AddOrder";
    let endpoint = "/0/private/AddOrder";
    
    let client = reqwest::Client::new();
    let nonce = format!("{}", Utc::now().timestamp_millis());
    let mut params = HashMap::new();
    params.insert("nonce", nonce.clone());
    params.insert("ordertype", "market".to_string());
    params.insert("type", "sell".to_string());
    params.insert("volume", volume.clone());
    params.insert("pair", "USDCUSD".to_string()); // Set trading pair to USDCUSD
    
    // Create post data string
    let post_data = format!("nonce={}&ordertype=market&type=sell&volume={}&pair=USDCUSD", nonce, volume);
    
    // Generate API signature
    let api_sign = generate_signature(&api_secret, &nonce, endpoint, &post_data);

    // Print request details for debugging
    println!("API Key: {}", api_key);
    println!("API Sign: {}", api_sign);
    println!("Post Data: {}", post_data);

    // Send POST request to place market order
    let response = client
        .post(url)
        .header("API-Key", api_key.as_str())
        .header("API-Sign", api_sign)
        .form(&params)
        .send()
        .await?;

    // Print response status and body for debugging
    let status = response.status();
    let body = response.text().await?;
    println!("Response status: {}", status);
    println!("Response body: {}", body);

    // Process the response if successful
    if status.is_success() {
        let order_response: OrderResponse = serde_json::from_str(&body)?;
        if order_response.error.is_empty() {
            println!("Market order placed successfully");
        } else {
            println!("API returned errors: {:?}", order_response.error);
        }
    } else {
        println!("Request failed with status code: {}", status);
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let api_key = Arc::new("your_api_key".to_string());
    let api_secret = Arc::new("your_api_secret".to_string());

    // Fetch balance task
    let balance_task = {
        let api_key = Arc::clone(&api_key);
        let api_secret = Arc::clone(&api_secret);
        tokio::spawn(async move {
            fetch_balance(&api_key, &api_secret).await
        })
    };

    // Wait for the balance to be fetched
    if let Ok(Ok(Some(balance))) = balance_task.await {
        if let Some(usdc_balance) = balance.get("USDC") {
            println!("USDC Balance: {}", usdc_balance);

            // If there is enough balance, place a market sell order
            if usdc_balance.parse::<f64>().unwrap() > 0.0 {
                let volume = usdc_balance.clone();
                let order_task = {
                    let api_key = Arc::clone(&api_key);
                    let api_secret = Arc::clone(&api_secret);
                    tokio::spawn(async move {
                        place_market_order_usd(api_key, api_secret, volume).await
                    })
                };

                if let Err(e) = order_task.await {
                    eprintln!("Error placing market order: {:?}", e);
                }
            } else {
                println!("No USDC balance to sell.");
            }
        } else {
            println!("No USDC balance found.");
        }
    } else {
        eprintln!("Error fetching balance or no balance available.");
    }
}
