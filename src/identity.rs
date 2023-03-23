use std::collections::HashMap;
use std::io::Write;
use tokio::sync::mpsc;

pub async fn writer<W: Write + Send + 'static>(
    crawler_q_tx: mpsc::Sender<String>,
    iden_q_rx: &mut mpsc::Receiver<(String, String)>,
    mut out: W,
) {
    let mut identity_map = HashMap::new();
    let mut id_count: u32 = 0;

    // TODO: add buffered writing
    while let Some((from, to)) = iden_q_rx.recv().await {
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

        let to_bytes = to.as_bytes();
        let to_id = match identity_map.get(to_bytes) {
            Some(id) => *id,
            None => {
                identity_map.insert(to_bytes.to_vec(), id_count);

                crawler_q_tx.send(to).await;

                let id = id_count;
                id_count += 1;
                id
            }
        };

        out.write(format!("{} {}\n", from_id, to_id).as_bytes())
            .expect("whoops");
    }
}
