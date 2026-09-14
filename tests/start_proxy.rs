mod support;

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use hickory_proto::rr::RecordType;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

use support::{
    DohResponse, RunningSplitHello, TEST_TIMEOUT, TestConfig, TestDoh, TestOrigin,
    build_client_hello, connect_via_proxy, decode_handshake_records, fetch_pac, request_via_proxy,
};

#[tokio::test]
async fn kernel_assigned_proxy_pac_and_origin_ports_are_distinct_and_held() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    for _ in 0..8 {
        let origin = TestOrigin::start_ipv4(Vec::new()).await;
        assert_ne!(
            origin.addr.port(),
            8443,
            "origin must not require global port 8443"
        );
        let mut process = RunningSplitHello::start(TestConfig {
            doh: &doh,
            origin_port: origin.addr.port(),
            target_host: "selected.test",
            with_pac: true,
            max_connections: 2,
            shutdown_seconds: 1,
            injected_secret: None,
        });
        let pac = process.pac_addr.expect("PAC enabled");
        assert_ne!(process.proxy_addr, pac);
        assert!(std::net::TcpListener::bind(process.proxy_addr).is_err());
        assert!(std::net::TcpListener::bind(pac).is_err());
        assert!(std::net::TcpListener::bind(origin.addr).is_err());
        process.signal("INT");
        assert!(process.wait_for_exit(TEST_TIMEOUT).success());
    }
}

#[tokio::test]
async fn ephemeral_listener_runtime_enforces_exact_dynamic_allowed_port() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let origin = TestOrigin::start_ipv4(Vec::new()).await;
    let mut process = RunningSplitHello::start(TestConfig {
        doh: &doh,
        origin_port: origin.addr.port(),
        target_host: "selected.test",
        with_pac: false,
        max_connections: 2,
        shutdown_seconds: 1,
        injected_secret: None,
    });

    let unconfigured_port = if origin.addr.port() == 1 { 2 } else { 1 };
    let response = request_via_proxy(
        process.proxy_addr,
        &format!("selected.test:{unconfigured_port}"),
    )
    .await;
    assert!(response.starts_with(b"HTTP/1.1 403 Forbidden\r\n"));
    assert_eq!(
        origin
            .wait_for_connections_count(Duration::from_millis(100))
            .await,
        0
    );
    assert!(doh.queries().is_empty());

    process.signal("INT");
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
}

#[tokio::test]
async fn v14_direct_ipv4_and_available_ipv6_relay_are_byte_identical() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let ipv4 = TestOrigin::start_ipv4(b"ipv4-reply".to_vec()).await;
    let mut process = RunningSplitHello::start(TestConfig {
        doh: &doh,
        origin_port: ipv4.addr.port(),
        target_host: "selected.test",
        with_pac: false,
        max_connections: 2,
        shutdown_seconds: 1,
        injected_secret: None,
    });

    let mut client = connect_via_proxy(
        process.proxy_addr,
        &format!("localhost:{}", ipv4.addr.port()),
    )
    .await;
    client.write_all(b"direct-ipv4").await.unwrap();
    client.shutdown().await.unwrap();
    let mut reply = Vec::new();
    timeout(TEST_TIMEOUT, client.read_to_end(&mut reply))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reply, b"ipv4-reply");
    assert_eq!(
        ipv4.wait_for_connections(1).await,
        vec![b"direct-ipv4".to_vec()]
    );
    assert!(
        doh.queries().is_empty(),
        "direct IPv4 must not use target DoH"
    );

    if let Some(ipv6) = TestOrigin::start_ipv6(0, b"ipv6-reply".to_vec()).await {
        let mut ipv6_process = RunningSplitHello::start(TestConfig {
            doh: &doh,
            origin_port: ipv6.addr.port(),
            target_host: "selected.test",
            with_pac: false,
            max_connections: 2,
            shutdown_seconds: 1,
            injected_secret: None,
        });
        let mut client = connect_via_proxy(
            ipv6_process.proxy_addr,
            &format!("localhost:{}", ipv6.addr.port()),
        )
        .await;
        client.write_all(b"direct-ipv6").await.unwrap();
        client.shutdown().await.unwrap();
        let mut reply = Vec::new();
        timeout(TEST_TIMEOUT, client.read_to_end(&mut reply))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reply, b"ipv6-reply");
        assert_eq!(
            ipv6.wait_for_connections(1).await,
            vec![b"direct-ipv6".to_vec()]
        );
        assert!(
            doh.queries().is_empty(),
            "direct IPv6 must not use target DoH"
        );
        ipv6_process.signal("INT");
        assert!(ipv6_process.wait_for_exit(TEST_TIMEOUT).success());
    }

    process.signal("INT");
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
}

#[tokio::test]
async fn v15_selected_target_uses_authenticated_doh_and_preserves_client_hello() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let origin = TestOrigin::start_ipv4(b"origin-reply".to_vec()).await;
    let mut process = RunningSplitHello::start(TestConfig {
        doh: &doh,
        origin_port: origin.addr.port(),
        target_host: "selected.test",
        with_pac: false,
        max_connections: 2,
        shutdown_seconds: 1,
        injected_secret: None,
    });

    let input = build_client_hello("selected.test", &[2, 19]);
    let (input_handshake, input_consumed) = decode_handshake_records(&input, 3);
    assert_eq!(input_consumed, input.len());
    let mut client = connect_via_proxy(
        process.proxy_addr,
        &format!("selected.test:{}", origin.addr.port()),
    )
    .await;
    client.write_all(&input).await.unwrap();
    client.write_all(b"opaque-tail").await.unwrap();
    client.shutdown().await.unwrap();
    let mut reply = Vec::new();
    timeout(TEST_TIMEOUT, client.read_to_end(&mut reply))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reply, b"origin-reply");

    let queries = doh.wait_for_queries(1).await;
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].queries.len(), 1);
    assert_eq!(queries[0].queries[0].query_type(), RecordType::A);
    assert_eq!(queries[0].queries[0].name().to_ascii(), "selected.test.");

    let received = origin.wait_for_connections(1).await;
    let (output_handshake, consumed) = decode_handshake_records(&received[0], 2);
    assert_eq!(output_handshake, input_handshake);
    assert_eq!(&received[0][consumed..], b"opaque-tail");
    process.wait_for_output("outcome=target-transformed");

    process.signal("INT");
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
}

#[tokio::test]
async fn v16_malformed_selected_tunnel_isolated_from_direct_success_and_bound() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let origin = TestOrigin::start_ipv4(b"direct-success".to_vec()).await;
    let mut process = RunningSplitHello::start(TestConfig {
        doh: &doh,
        origin_port: origin.addr.port(),
        target_host: "selected.test",
        with_pac: false,
        max_connections: 1,
        shutdown_seconds: 1,
        injected_secret: None,
    });

    let mut selected = connect_via_proxy(
        process.proxy_addr,
        &format!("selected.test:{}", origin.addr.port()),
    )
    .await;
    let direct_authority = format!("localhost:{}", origin.addr.port());
    let mut direct_connect = Box::pin(connect_via_proxy(process.proxy_addr, &direct_authority));
    assert!(
        timeout(Duration::from_millis(100), &mut direct_connect)
            .await
            .is_err(),
        "the configured one-handler bound must hold while selected tunnel is active"
    );

    selected
        .write_all(&build_client_hello("mismatched.test", &[]))
        .await
        .unwrap();
    selected.shutdown().await.unwrap();
    let mut selected_rest = Vec::new();
    timeout(TEST_TIMEOUT, selected.read_to_end(&mut selected_rest))
        .await
        .unwrap()
        .unwrap();

    let mut direct = timeout(TEST_TIMEOUT, &mut direct_connect)
        .await
        .expect("direct CONNECT proceeds after selected failure");
    direct.write_all(b"direct-request").await.unwrap();
    direct.shutdown().await.unwrap();
    let mut reply = Vec::new();
    timeout(TEST_TIMEOUT, direct.read_to_end(&mut reply))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reply, b"direct-success");

    let connections = origin.wait_for_connections(2).await;
    assert!(connections.iter().any(Vec::is_empty));
    assert!(connections.iter().any(|bytes| bytes == b"direct-request"));
    process.wait_for_output("outcome=tls-error");
    process.wait_for_output("outcome=direct-relay");

    let mut survivor = connect_via_proxy(
        process.proxy_addr,
        &format!("localhost:{}", origin.addr.port()),
    )
    .await;
    survivor.write_all(b"listener-survived").await.unwrap();
    survivor.shutdown().await.unwrap();
    let mut reply = Vec::new();
    timeout(TEST_TIMEOUT, survivor.read_to_end(&mut reply))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reply, b"direct-success");
    assert!(origin.peak_active() <= 1);

    process.signal("INT");
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
}

#[tokio::test]
async fn v17_signal_shutdown_closes_active_tunnel_and_listeners_before_deadline() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let origin = TestOrigin::start_ipv4(Vec::new()).await;
    let mut process = RunningSplitHello::start(TestConfig {
        doh: &doh,
        origin_port: origin.addr.port(),
        target_host: "selected.test",
        with_pac: true,
        max_connections: 2,
        shutdown_seconds: 1,
        injected_secret: None,
    });
    let mut active = connect_via_proxy(
        process.proxy_addr,
        &format!("localhost:{}", origin.addr.port()),
    )
    .await;

    let started = Instant::now();
    process.signal("TERM");
    let status = process.wait_for_exit(Duration::from_secs(3));
    assert!(status.success());
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(TcpStream::connect(process.proxy_addr).await.is_err());
    assert!(TcpStream::connect(process.pac_addr.unwrap()).await.is_err());
    let mut closed = Vec::new();
    timeout(TEST_TIMEOUT, active.read_to_end(&mut closed))
        .await
        .expect("active tunnel closes during shutdown")
        .expect("read active tunnel closure");
    assert!(!process.directory_path().join("state").exists());
}

#[tokio::test]
async fn v18_exact_start_command_reaches_ready_and_serves_target_only_pac() {
    let doh = TestDoh::start(DohResponse::Addresses(vec![Ipv4Addr::LOCALHOST])).await;
    let origin = TestOrigin::start_ipv4(Vec::new()).await;
    let mut process = RunningSplitHello::start(TestConfig {
        doh: &doh,
        origin_port: origin.addr.port(),
        target_host: "selected.test",
        with_pac: true,
        max_connections: 2,
        shutdown_seconds: 1,
        injected_secret: None,
    });
    let output = process.output();
    assert!(output.contains("outcome=ready"));
    assert!(output.contains(&format!("proxy_addr={}", process.proxy_addr)));
    assert!(output.contains("target_count=1"));
    assert!(output.contains("doh_host=localhost"));
    assert!(output.contains("transport=\"IPv4 TCP only for selected targets\""));
    assert!(output.contains("scope=\"explicit loopback proxy; not a VPN\""));

    let response = fetch_pac(process.pac_addr.unwrap()).await;
    let text = String::from_utf8(response).unwrap();
    assert!(text.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(text.contains("host === \"selected.test\""));
    assert!(text.contains("dnsDomainIs(host, \".selected.test\")"));
    assert!(text.contains(&format!("PROXY {}", process.proxy_addr)));
    assert!(text.contains("return \"DIRECT\""));

    process.signal("INT");
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
}

#[tokio::test]
async fn v19_doh_failure_emits_stable_secret_safe_diagnostic() {
    const SECRET: &str = "INJECTED-SECRET-COOKIE";
    const RAW_REQUEST: &str = "X-Secret: INJECTED-SECRET-HEADER";
    let doh = TestDoh::start(DohResponse::Raw(vec![0xde, 0xad, 0xbe, 0xef])).await;
    let origin = TestOrigin::start_ipv4(Vec::new()).await;
    let mut process = RunningSplitHello::start(TestConfig {
        doh: &doh,
        origin_port: origin.addr.port(),
        target_host: "SeLeCtEd.TeSt.",
        with_pac: false,
        max_connections: 2,
        shutdown_seconds: 1,
        injected_secret: Some(SECRET),
    });

    let mut client = TcpStream::connect(process.proxy_addr).await.unwrap();
    let raw = format!(
        "CONNECT SeLeCtEd.TeSt.:{} HTTP/1.1\r\nHost: SeLeCtEd.TeSt.:{}\r\n{RAW_REQUEST}\r\n\r\n",
        origin.addr.port(),
        origin.addr.port()
    );
    client.write_all(raw.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    timeout(TEST_TIMEOUT, client.read_to_end(&mut response))
        .await
        .unwrap()
        .unwrap();
    assert!(response.starts_with(b"HTTP/1.1 502"));

    let output = process.wait_for_output("outcome=dns-error");
    assert!(output.contains("host=selected.test"));
    assert!(!output.contains(SECRET));
    assert!(!output.contains("INJECTED-SECRET-HEADER"));
    assert!(!output.contains("SeLeCtEd.TeSt."));
    assert!(!output.contains("deadbeef"));
    let hello = build_client_hello("selected.test", &[]);
    assert!(
        !output
            .as_bytes()
            .windows(hello.len())
            .any(|part| part == hello)
    );
    assert!(
        origin
            .wait_for_connections_count(Duration::from_millis(100))
            .await
            == 0
    );

    process.signal("INT");
    assert!(process.wait_for_exit(TEST_TIMEOUT).success());
}
