use log::info;
use std::collections::HashMap;
use std::io::Write;
use tokio::sync::mpsc;

pub async fn writer<W: Write + Send + 'static>(
    crawler_q_tx: mpsc::Sender<String>,
    iden_q_rx: &mut mpsc::Receiver<(bool, String, String)>,
    mut out: W,
) {
    let mut active_map = HashMap::new();

    let mut identity_map = HashMap::new();
    let mut id_count: u32 = 0;

    let mut remove_count = 0;
    let mut total_count = 0;

    // TODO: add buffered writing
    while let Some((over, from, to)) = iden_q_rx.recv().await {
        let from_bytes = from.as_bytes();
        let from_id = match identity_map.get(from_bytes) {
            Some(id) => *id,
            None => {
                identity_map.insert(from_bytes.to_vec(), id_count);
                let id = id_count;
                id_count += 1;
                id
            }
        };

        if over {
            remove_count += 1;
            match active_map.get(&from_bytes.to_vec()) {
                None => {
                    active_map.remove(&from_bytes.to_vec());
                }
                Some(_) => {
                    active_map.remove(&from_bytes.to_vec());
                }
            }
        }

        let to_bytes = to.as_bytes();
        let to_id = match identity_map.get(to_bytes) {
            Some(id) => *id,
            None => {
                identity_map.insert(to_bytes.to_vec(), id_count);

                let crawler_q_tx_clone = crawler_q_tx.clone();

                active_map.insert(to_bytes.to_vec(), ());

                // error should crash program
                tokio::spawn(async move {
                    crawler_q_tx_clone.send(to).await.unwrap();
                });
                total_count += 1;

                let id = id_count;
                id_count += 1;
                id
            }
        };

        out.write(format!("{} {}\n", from_id, to_id).as_bytes())
            .expect("whoops");

        info!(
            "total: {}, removed: {}, activity map: {:?}",
            total_count,
            remove_count,
            active_map.len()
        );

        if active_map.is_empty() {
            info!("closing");
            crawler_q_tx.closed().await;
            break;
        }
    }
}
