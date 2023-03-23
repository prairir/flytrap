use tokio::sync::mpsc;

pub async fn dispatcher(
    crawler_q_rx: &mut mpsc::Receiver<String>,
    iden_q_tx: mpsc::Sender<(String, String)>,
) {
    while let Some(url) = crawler_q_rx.recv().await {
        let id_clone = iden_q_tx.clone();
        tokio::spawn(async move { worker(id_clone, url).await });
    }
}

pub async fn worker(iden_q_rx: mpsc::Sender<(String, String)>, url: String) {
    println!("{}", url);
    iden_q_rx
        .send((String::from("Hello,"), String::from(" World")))
        .await;
}
