mod helpers;

use std::fs;

#[test]
fn test_fixtures_exist_and_readable() {
    let config = helpers::fixture_path("sample-config.toml");
    let session = helpers::fixture_path("sample-session.json");

    assert!(config.exists());
    assert!(session.exists());

    let config_contents = fs::read_to_string(config).expect("read config fixture");
    let session_contents = fs::read_to_string(session).expect("read session fixture");

    assert!(config_contents.contains("[provider]"));
    assert!(session_contents.contains("\"messages\""));
}
