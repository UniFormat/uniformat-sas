//! Serveur Arrow Flight sur un dossier de fichiers `.sas7bdat`.
use std::net::SocketAddr;
use std::path::PathBuf;

use arrow_flight::flight_service_server::FlightServiceServer;
use clap::Parser;
use uniformat_sas_flight::{bearer_interceptor, SasFlightService};

/// Sert les fichiers `.sas7bdat` d'un dossier en Arrow Flight (lecture seule).
#[derive(Parser, Debug)]
#[command(name = "uniformat-sas-flight-server", version)]
struct Args {
    /// Dossier racine : seuls les fichiers qu'il contient sont servis.
    root: PathBuf,
    /// Adresse d'écoute.
    #[arg(long, default_value = "127.0.0.1:50051")]
    addr: SocketAddr,
    /// Nombre d'endpoints par défaut d'une table non compressée.
    #[arg(long)]
    partitions: Option<usize>,
    /// Jeton porteur exigé (sinon variable `UNIFORMAT_FLIGHT_TOKEN` ; sans jeton, pas d'authentification).
    #[arg(long, env = "UNIFORMAT_FLIGHT_TOKEN", hide_env_values = true)]
    token: Option<String>,
    /// Authorize a non-loopback listener only behind an authenticated TLS reverse proxy on a private network.
    #[arg(long)]
    trusted_proxy: bool,
}

impl Args {
    fn validate(&self) -> std::io::Result<()> {
        if !self.addr.ip().is_loopback() && !self.trusted_proxy {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,
                "non-loopback Flight requires --trusted-proxy and an authenticated TLS proxy on a private network"));
        }
        if self
            .token
            .as_ref()
            .is_some_and(|t| t.is_empty() || t.chars().any(char::is_control))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "empty or invalid bearer token",
            ));
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    args.validate()?;
    let mut service = SasFlightService::new(&args.root)?;
    if let Some(n) = args.partitions {
        service = service.with_default_partitions(n);
    }
    let listener = tokio::net::TcpListener::bind(args.addr).await?;
    let local = listener.local_addr()?;
    // première ligne de la sortie standard : adresse effective (utile avec le port 0)
    {
        use std::io::Write;
        let mut out = std::io::stdout().lock();
        writeln!(out, "listening {local}")?;
        out.flush()?;
    }
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let mut server = tonic::transport::Server::builder();
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    match args.token {
        Some(t) => {
            server
                .add_service(FlightServiceServer::with_interceptor(
                    service,
                    bearer_interceptor(t),
                ))
                .serve_with_incoming_shutdown(incoming, shutdown)
                .await?;
        }
        None => {
            server
                .add_service(FlightServiceServer::new(service))
                .serve_with_incoming_shutdown(incoming, shutdown)
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_listener_requires_explicit_trusted_proxy_configuration() {
        let args = Args::try_parse_from(["server", ".", "--addr", "0.0.0.0:50051"]).unwrap();
        assert!(args.validate().is_err());
        let args =
            Args::try_parse_from(["server", ".", "--addr", "0.0.0.0:50051", "--trusted-proxy"])
                .unwrap();
        assert!(args.validate().is_ok());
        let args = Args::try_parse_from(["server", "."]).unwrap();
        assert!(args.validate().is_ok());
    }
}
