use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use reqwest_middleware::{self, ClientBuilder};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use scraper::{Html, Selector};
use tokio::{sync::mpsc, task::JoinSet};
use url::Url;

pub async fn dispatcher(
    crawler_q_rx: &mut mpsc::Receiver<String>,
    iden_q_tx: mpsc::Sender<(String, String)>,
    limit: u32,
) {
    let mut count: u32 = 0;
    let mut waitset = JoinSet::new();

    let retry_police = ExponentialBackoff::builder().build_with_max_retries(5);
    let req_client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("whoops"); // TODO: error propegation
    let client = Arc::new(
        ClientBuilder::new(req_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_police))
            .build(),
    );

    while let Some(url) = crawler_q_rx.recv().await {
        count += 1;
        let id_clone = iden_q_tx.clone();
        let client_clone = client.clone();
        waitset.spawn(async move { worker(id_clone, client_clone, url.clone()).await });
        if count >= limit {
            break;
        }
    }

    // await till all tasks are complete
    // TODO: error propegation
    while let Some(_) = waitset.join_next().await {}

    iden_q_tx.closed();
}

// TODO: error propegation
// TODO: maybe pass in selector with ARC to save on allocations
pub async fn worker(
    iden_q_rx: mpsc::Sender<(String, String)>,
    client: Arc<reqwest_middleware::ClientWithMiddleware>,
    url: String,
) -> Result<()> {
    // uri parsing is much more flexible
    // we need to be able to handle relative links
    // and links without schemes
    let url_parsed = Url::parse(url.as_str()).unwrap();

    let resp = client.get(url.as_str()).send().await;
    let a = match resp {
        Err(e) => {
            println!("{}", e);
            panic!("whoops");
        }
        Ok(s) => s,
    };

    let mut links = Vec::new();

    let body = a.text().await.unwrap(); // TODO: do that cleaner and DONT PANIC JUST MOVE ON

    {
        let doc = Html::parse_document(&body);
        let a_selector = Selector::parse(r#"a[href]"#).unwrap();

        // find all links on page
        // make relative ones absolute
        // add a scheme to ones without a scheme

        // TODO: dont give a shit about `#` id locators
        // yes i know that excludes things that use those as routers but i dont care
        // i dont even think most of those routers are ssr so doesnt matter

        for element in doc.select(&a_selector) {
            let link_str = match element.value().attr("href") {
                Some(l) => l,
                None => continue,
            };

            let link = match Url::parse(link_str) {
                Ok(l) => l,
                Err(_) => match url_parsed.join(link_str) {
                    Ok(l) => l,
                    Err(_) => continue,
                },
            };

            let link_scheme = match link.scheme() {
                "" => {
                    let new = url_parsed.scheme().to_owned() + &link.to_string();
                    Url::parse(new.as_str()).unwrap()
                }
                "http" | "https" => link,
                _ => continue,
            };

            links.push(link_scheme.to_string());
        }
    }

    // TODO: change to &str so less heap copying
    for link in links {
        iden_q_rx.send((url.clone(), link)).await?;
    }
    Ok(())
}
