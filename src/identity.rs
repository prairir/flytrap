use std::io::Write;
use tokio::sync::mpsc;

pub async fn writer<W: Write + Send + 'static>(
    _crawler_q_tx: mpsc::Sender<String>,
    iden_q_rx: &mut mpsc::Receiver<(String, String)>,
    mut out: W,
) {
    // TODO: add buffered writing
    while let Some((from, to)) = iden_q_rx.recv().await {
        out.write(format!("{} {}\n", from, to).as_bytes())
            .expect("whoops");
    }
}
