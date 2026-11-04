//! Serveur HTTP de test (HEAD et GET avec `Range`) qui compte les requêtes : partagé par les tests `object_store` des
//! crates `uniformat-sas-arrow` et `uniformat-sas-datafusion`.
#![allow(dead_code, missing_docs)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Sert les fichiers de `root` ; `requests` compte les requêtes reçues.
pub(crate) async fn serve(
    root: std::path::PathBuf,
    requests: Arc<AtomicU64>,
) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                break;
            };
            let (root, requests) = (root.clone(), Arc::clone(&requests));
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                requests.fetch_add(1, Ordering::SeqCst);
                let text = String::from_utf8_lossy(&buf).into_owned();
                let mut lines = text.split("\r\n");
                let first = lines.next().unwrap_or_default().to_owned();
                let mut parts = first.split(' ');
                let method = parts.next().unwrap_or_default().to_owned();
                let target = parts
                    .next()
                    .unwrap_or_default()
                    .trim_start_matches('/')
                    .to_owned();
                let range = lines
                    .find_map(|l| {
                        l.strip_prefix("range: ")
                            .or_else(|| l.strip_prefix("Range: "))
                    })
                    .map(str::to_owned);
                let Ok(data) = std::fs::read(root.join(&target)) else {
                    let _ = sock
                        .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                        .await;
                    return;
                };
                let total = data.len() as u64;
                let common = "Last-Modified: Wed, 21 Oct 2015 07:28:00 GMT\r\nETag: \"x\"\r\nAccept-Ranges: bytes\r\nConnection: close\r\n";
                let response: Vec<u8> = if method == "HEAD" {
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\n{common}\r\n")
                        .into_bytes()
                } else if let Some(r) = range {
                    let spec = r.trim().trim_start_matches("bytes=");
                    let (a, b) = spec.split_once('-').unwrap_or(("0", ""));
                    let start: u64 = a.parse().unwrap_or(0);
                    let end: u64 = b.parse().map_or(total - 1, |e: u64| e.min(total - 1));
                    let body = &data
                        [usize::try_from(start).unwrap_or(0)..=usize::try_from(end).unwrap_or(0)];
                    let mut out = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {start}-{end}/{total}\r\nContent-Length: {}\r\n{common}\r\n",
                        body.len()
                    )
                    .into_bytes();
                    out.extend_from_slice(body);
                    out
                } else {
                    let mut out =
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\n{common}\r\n")
                            .into_bytes();
                    out.extend_from_slice(&data);
                    out
                };
                let _ = sock.write_all(&response).await;
                let _ = sock.shutdown().await;
            });
        }
    });
    addr
}
