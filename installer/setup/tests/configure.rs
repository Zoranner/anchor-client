use std::fs;

use anchor_setup::write_service_config;
use tempfile::tempdir;

#[test]
fn writes_server_url_without_leaving_a_temporary_file() {
    let directory = tempdir().unwrap();
    let config = directory.path().join("service.toml");
    let database = directory.path().join("audit.db");

    write_service_config(&config, &database, "anchor.example.com", "8443").unwrap();

    let configured: toml::Value = fs::read_to_string(&config).unwrap().parse().unwrap();
    assert_eq!(
        configured["database"].as_str(),
        Some(database.to_str().unwrap())
    );
    assert_eq!(
        configured["server_url"].as_str(),
        Some("https://anchor.example.com:8443")
    );
    assert!(!directory.path().join("service.toml.tmp").exists());
}

#[test]
fn accepts_an_explicit_scheme_and_loopback_http() {
    let directory = tempdir().unwrap();
    let config = directory.path().join("service.toml");

    write_service_config(
        &config,
        &directory.path().join("audit.db"),
        "http://127.0.0.1",
        "8080",
    )
    .unwrap();

    let configured: toml::Value = fs::read_to_string(&config).unwrap().parse().unwrap();
    assert_eq!(
        configured["server_url"].as_str(),
        Some("http://127.0.0.1:8080")
    );
}

#[test]
fn rejects_invalid_input_without_replacing_existing_config() {
    let directory = tempdir().unwrap();
    let config = directory.path().join("service.toml");
    fs::write(&config, "previous configuration\n").unwrap();

    let error = write_service_config(
        &config,
        &directory.path().join("audit.db"),
        "https://anchor.example.com",
        "70000",
    )
    .unwrap_err();

    assert!(error.to_string().contains("端口"));
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        "previous configuration\n"
    );
}

#[test]
fn rejects_plain_http_for_non_loopback_hosts() {
    let directory = tempdir().unwrap();
    let error = write_service_config(
        &directory.path().join("service.toml"),
        &directory.path().join("audit.db"),
        "http://anchor.example.com",
        "8080",
    )
    .unwrap_err();
    assert!(error.to_string().contains("HTTPS"));
}
