//! Security checks always run on generated data, with no external corpus.
#[path = "../../../tests/support/fixtures.rs"]
mod fixtures;
use arrow_flight::flight_service_client::FlightServiceClient;
use arrow_flight::flight_service_server::{FlightService, FlightServiceServer};
use arrow_flight::{Criteria, FlightClient, FlightDescriptor, Ticket};
use futures::{StreamExt, TryStreamExt};
use std::sync::Arc;
use tonic::transport::{Channel, Server};
use tonic::{Code, Request};
use uniformat_sas::Compression;
use uniformat_sas_flight::{
    bearer_interceptor, Command, ReadSpec, SasFlightService, ServiceLimits, TicketBody,
};

fn descriptor(path: &str, spec: ReadSpec) -> FlightDescriptor {
    FlightDescriptor::new_cmd(
        serde_json::to_vec(&Command {
            path: path.into(),
            partitions: Some(1),
            spec,
        })
        .unwrap(),
    )
}

struct RunningServer {
    dir: uniformat_testkit::TempDir,
    channel: Channel,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve(token: Option<&str>) -> RunningServer {
    let dir = uniformat_testkit::tempdir().unwrap();
    std::fs::write(
        dir.path().join("table.sas7bdat"),
        fixtures::table(100, Compression::None),
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let service = SasFlightService::new(dir.path()).unwrap();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let token = token.map(str::to_owned);
    let task = tokio::spawn(async move {
        let mut server = Server::builder();
        match token {
            Some(token) => server
                .add_service(FlightServiceServer::with_interceptor(
                    service,
                    bearer_interceptor(token),
                ))
                .serve_with_incoming(incoming)
                .await
                .unwrap(),
            None => server
                .add_service(FlightServiceServer::new(service))
                .serve_with_incoming(incoming)
                .await
                .unwrap(),
        }
    });
    let channel = Channel::from_shared(format!("http://{addr}"))
        .unwrap()
        .connect()
        .await
        .unwrap();
    RunningServer { dir, channel, task }
}

#[tokio::test]
async fn real_rpc_rejects_repeated_columns_and_oversized_payloads() {
    let server = serve(None).await;
    let mut client = FlightServiceClient::new(server.channel.clone());
    let repeated = ReadSpec {
        columns: Some(vec!["x".into(); 64]),
        batch_rows: Some(usize::MAX),
        ..ReadSpec::default()
    };
    let error = client
        .get_flight_info(descriptor("table.sas7bdat", repeated.clone()))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::InvalidArgument);
    let ticket = TicketBody {
        path: "table.sas7bdat".into(),
        pages: (0, 1),
        spec: repeated,
    };
    let error = client
        .do_get(Ticket::new(serde_json::to_vec(&ticket).unwrap()))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::InvalidArgument);
    let error = client
        .do_get(Ticket::new(vec![b' '; 65_537]))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::ResourceExhausted);
    let error = client
        .get_schema(FlightDescriptor::new_cmd(vec![b' '; 65_537]))
        .await
        .unwrap_err();
    assert_eq!(error.code(), Code::ResourceExhausted);
}

#[tokio::test]
async fn real_rpc_requires_the_configured_bearer_token() {
    let server = serve(Some("synthetic-test-token")).await;
    let mut client = FlightServiceClient::new(server.channel.clone());
    let cmd = descriptor("table.sas7bdat", ReadSpec::default());
    assert_eq!(
        client
            .get_flight_info(cmd.clone())
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    for token in [
        "Bearer incorrect",
        "Basic synthetic-test-token",
        "Bearer synthetic-test-token-extra",
    ] {
        let mut req = Request::new(cmd.clone());
        req.metadata_mut()
            .insert("authorization", token.parse().unwrap());
        assert_eq!(
            client.get_flight_info(req).await.unwrap_err().code(),
            Code::Unauthenticated
        );
    }
    let mut req = Request::new(cmd);
    req.metadata_mut().insert(
        "authorization",
        "Bearer synthetic-test-token".parse().unwrap(),
    );
    assert_eq!(
        client
            .get_flight_info(req)
            .await
            .unwrap()
            .into_inner()
            .total_records,
        100
    );
    assert!(bearer_interceptor(String::new())(Request::new(())).is_err());
}

#[tokio::test]
async fn real_rpc_validates_paths_page_ranges_and_compression() {
    let server = serve(None).await;
    std::fs::write(
        server.dir.path().join("compressed.sas7bdat"),
        fixtures::table(100, Compression::Char),
    )
    .unwrap();
    let mut client = FlightServiceClient::new(server.channel.clone());
    for path in [
        "../table.sas7bdat",
        "/etc/passwd",
        "./../table.sas7bdat",
        "table.csv",
        "",
        "missing.sas7bdat",
    ] {
        assert!(
            client
                .get_flight_info(descriptor(path, ReadSpec::default()))
                .await
                .is_err(),
            "{path}"
        );
    }
    for (path, pages) in [
        ("table.sas7bdat", (0, u64::MAX)),
        ("table.sas7bdat", (5, 2)),
        ("compressed.sas7bdat", (1, 2)),
        ("../table.sas7bdat", (0, 1)),
    ] {
        let body = TicketBody {
            path: path.into(),
            pages,
            spec: ReadSpec::default(),
        };
        assert!(client
            .do_get(Ticket::new(serde_json::to_vec(&body).unwrap()))
            .await
            .is_err());
    }
    assert_eq!(
        client
            .do_get(Ticket::new(b"invalid json".to_vec()))
            .await
            .unwrap_err()
            .code(),
        Code::InvalidArgument
    );
    #[cfg(unix)]
    {
        let outside = uniformat_testkit::tempdir().unwrap();
        let path = outside.path().join("outside.sas7bdat");
        std::fs::write(&path, fixtures::table(1, Compression::None)).unwrap();
        std::os::unix::fs::symlink(&path, server.dir.path().join("link.sas7bdat")).unwrap();
        assert!(client
            .get_flight_info(descriptor("link.sas7bdat", ReadSpec::default()))
            .await
            .is_err());
    }
}

#[tokio::test]
async fn synthetic_projection_limit_and_listing_still_work_over_rpc() {
    let server = serve(None).await;
    let mut client = FlightClient::new(server.channel.clone());
    let info = client
        .get_flight_info(descriptor(
            "table.sas7bdat",
            ReadSpec {
                columns: Some(vec!["text".into(), "x".into()]),
                limit: Some(7),
                ..ReadSpec::default()
            },
        ))
        .await
        .unwrap();
    assert_eq!(info.total_records, 7);
    let ticket = info.endpoint[0].ticket.clone().unwrap();
    let batches: Vec<_> = client
        .do_get(ticket)
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(batches.iter().map(|b| b.num_rows()).sum::<usize>(), 7);
    assert_eq!(batches[0].schema().field(0).name(), "text");
    let infos: Vec<_> = client
        .list_flights(Vec::<u8>::new())
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(infos.len(), 1);
}

#[tokio::test]
async fn admission_covers_unconsumed_responses_and_listing_work_is_bounded() {
    let dir = uniformat_testkit::tempdir().unwrap();
    std::fs::write(dir.path().join("table.sas7bdat"), fixtures::numeric(1)).unwrap();
    let service = SasFlightService::new(dir.path())
        .unwrap()
        .with_limits(ServiceLimits {
            concurrent_requests: 1,
            listing_entries: 2,
            ..ServiceLimits::default()
        })
        .unwrap();
    let response = service
        .list_flights(Request::new(Criteria::default()))
        .await
        .unwrap();
    let err = service
        .get_flight_info(Request::new(descriptor(
            "table.sas7bdat",
            ReadSpec::default(),
        )))
        .await
        .unwrap_err();
    assert_eq!(err.code(), Code::ResourceExhausted);
    drop(response);
    assert!(service
        .get_flight_info(Request::new(descriptor(
            "table.sas7bdat",
            ReadSpec::default()
        )))
        .await
        .is_ok());
    for name in ["unrelated-1", "unrelated-2"] {
        std::fs::write(dir.path().join(name), b"").unwrap();
    }
    let error = service
        .list_flights(Request::new(Criteria::default()))
        .await
        .err()
        .unwrap();
    assert_eq!(error.code(), Code::ResourceExhausted);
}

#[tokio::test]
async fn dropping_a_data_stream_releases_the_worker_slot() {
    let dir = uniformat_testkit::tempdir().unwrap();
    let bytes = fixtures::numeric(50_000);
    let file = uniformat_sas_io::SasFile::open(bytes.clone(), Default::default()).unwrap();
    let body = TicketBody {
        path: "table.sas7bdat".into(),
        pages: (0, file.meta().header.page_count),
        spec: ReadSpec {
            batch_rows: Some(1),
            ..ReadSpec::default()
        },
    };
    std::fs::write(dir.path().join("table.sas7bdat"), bytes).unwrap();
    let service = Arc::new(
        SasFlightService::new(dir.path())
            .unwrap()
            .with_limits(ServiceLimits {
                concurrent_requests: 1,
                ..ServiceLimits::default()
            })
            .unwrap(),
    );
    let response = service
        .do_get(Request::new(Ticket::new(
            serde_json::to_vec(&body).unwrap(),
        )))
        .await
        .unwrap();
    assert_eq!(
        service
            .get_flight_info(Request::new(descriptor(
                "table.sas7bdat",
                ReadSpec::default()
            )))
            .await
            .unwrap_err()
            .code(),
        Code::ResourceExhausted
    );
    drop(response);
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if service
                .get_flight_info(Request::new(descriptor(
                    "table.sas7bdat",
                    ReadSpec::default(),
                )))
                .await
                .is_ok()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    // A following request remains usable, including its response body.
    let mut listing = service
        .list_flights(Request::new(Criteria::default()))
        .await
        .unwrap()
        .into_inner();
    assert!(listing.next().await.unwrap().is_ok());
}

#[tokio::test]
async fn listing_limits_the_sum_of_metadata_bytes() {
    use prost::Message;
    let dir = uniformat_testkit::tempdir().unwrap();
    for i in 0..3 {
        std::fs::write(
            dir.path().join(format!("file{i}.sas7bdat")),
            fixtures::numeric(1),
        )
        .unwrap();
    }
    let service = SasFlightService::new(dir.path()).unwrap();
    let mut one = service
        .get_flight_info(Request::new(FlightDescriptor::new_path(vec![
            "file0.sas7bdat".into(),
        ])))
        .await
        .unwrap()
        .into_inner();
    one.endpoint.clear();
    let limit = 2 * one.encoded_len();
    let service = service
        .with_limits(ServiceLimits {
            response_bytes: limit,
            ..ServiceLimits::default()
        })
        .unwrap();
    let error = service
        .list_flights(Request::new(Criteria::default()))
        .await
        .err()
        .unwrap();
    assert_eq!(error.code(), Code::ResourceExhausted);
}
