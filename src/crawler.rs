use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use log::info;
use reqwest_middleware::{self, ClientBuilder};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use scraper::{Html, Selector};
use tokio::{sync::mpsc, task::JoinSet};
use url::Url;

pub async fn dispatcher(
    crawler_q_rx: &mut mpsc::Receiver<String>,
    iden_q_tx: mpsc::Sender<(bool, String, String)>,
    limit: u32,
    delay: Duration,
) -> Result<()> {
    let mut count: u32 = 0;
    let mut waitset = JoinSet::new();

    let retry_police = ExponentialBackoff::builder().build_with_max_retries(3);
    let req_client = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let client = Arc::new(
        ClientBuilder::new(req_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_police))
            .build(),
    );

    while let Some(url) = crawler_q_rx.recv().await {
        // delay stops websites from shutting us down
        let s = tokio::time::sleep(delay);
        s.await;

        count += 1;
        let id_clone = iden_q_tx.clone();
        let client_clone = client.clone();
        waitset.spawn(async move { worker(id_clone, client_clone, url.clone()).await });
        if count >= limit {
            break;
        }
    }

    // await till all tasks are complete
    while let Some(w) = waitset.join_next().await {
        match w {
            Ok(_) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    iden_q_tx.closed().await;
    Ok(())
}

// TODO: maybe pass in selector with ARC to save on allocations
pub async fn worker(
    iden_q_rx: mpsc::Sender<(bool, String, String)>,
    client: Arc<reqwest_middleware::ClientWithMiddleware>,
    url: String,
) -> Result<()> {
    // this error should propegate up
    let url_parsed = match Url::parse(url.as_str()) {
        Ok(u) => u,
        Err(_) => {
            tokio::spawn(async move { iden_q_rx.send((true, url, String::from(""))).await });
            return Ok(());
        }
    };

    let resp = match client.get(url.as_str()).send().await {
        Ok(response) => response,
        Err(e) => {
            info!("couldnt get url \"{}\": {}", url, e);
            tokio::spawn(async move { iden_q_rx.send((true, url, String::from(""))).await });
            return Ok(());
        }
    };

    let mut links = Vec::new();

    let body = match resp.text().await {
        Ok(response) => response,
        Err(e) => {
            info!("no body at url \"{}\": {}", url, e);
            tokio::spawn(async move { iden_q_rx.send((true, url, String::from(""))).await });
            return Ok(());
        }
    };

    {
        let doc = Html::parse_document(&body);
        let a_selector = match Selector::parse(r#"a[href]"#) {
            Ok(selector) => selector,
            Err(_) => {
                tokio::spawn(async move { iden_q_rx.send((true, url, String::from(""))).await });
                return Ok(());
            }
        };

        // find all links on page
        // make relative ones absolute
        // add a scheme to ones without a scheme

        for element in doc.select(&a_selector) {
            let link_str = match element.value().attr("href") {
                Some(l) => l,
                None => continue,
            };

            let mut link = match Url::parse(link_str) {
                Ok(l) => l,
                Err(_) => match url_parsed.join(link_str) {
                    Ok(l) => l,
                    Err(_) => continue,
                },
            };

            link = match link.scheme() {
                "" => {
                    let new = url_parsed.scheme().to_owned() + &link.to_string();
                    Url::parse(new.as_str()).unwrap()
                }
                "http" | "https" => link,
                _ => continue,
            };

            // yes i know that excludes things that use those as routers but i dont care
            // i dont even think most of those routers are ssr so doesnt matter
            // this also removes a lot of duplicates
            link = match link.fragment() {
                None => link,
                Some(_) => {
                    link.set_fragment(None);
                    link
                }
            };

            link = match link.query() {
                None => link,
                Some(_) => {
                    link.set_query(None);
                    link
                }
            };

            let iden_clone = iden_q_rx.clone();
            let url_clone = url.clone();

            // TODO: change to &str so less heap copying
            let t = tokio::spawn(async move {
                iden_clone
                    .send((false, url_clone, link.to_string()))
                    .await
                    .unwrap()
            });

            links.push(t);
        }
    }

    // if cannot send on channel, error should propegate
    for link in links {
        match link.await {
            Ok(_) => {}
            Err(e) => {
                tokio::spawn(async move {
                    iden_q_rx.send((true, url, String::from(""))).await.unwrap()
                });
                return Err(e.into());
            }
        };
    }

    tokio::spawn(async move { iden_q_rx.send((true, url, String::from(""))).await });

    Ok(())
}
