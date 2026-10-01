use std::process::Command;

fn timezone(tz: Option<&str>, override_tz: Option<&str>) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_obscura"));
    command
        .args([
            "fetch", "data:text/html,<html></html>", "--wait", "0", "--quiet", "--eval",
            "(function(){return {zone:Intl.DateTimeFormat().resolvedOptions().timeZone,winter:new Date('2026-01-15T12:00:00Z').getTimezoneOffset(),summer:new Date('2026-07-15T12:00:00Z').getTimezoneOffset(),winterHour:new Date('2026-01-15T12:00:00Z').getHours(),summerHour:new Date('2026-07-15T12:00:00Z').getHours()};})()",
        ])
        .env_remove("TZ")
        .env_remove("OBSCURA_TIMEZONE");
    if let Some(tz) = tz { command.env("TZ", tz); }
    if let Some(tz) = override_tz { command.env("OBSCURA_TIMEZONE", tz); }
    let output = command.output().expect("run timezone probe");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).expect("timezone JSON")
}

fn berlin(value: serde_json::Value) {
    assert_eq!(value, serde_json::json!({
        "zone": "Europe/Berlin", "winter": -60, "summer": -120,
        "winterHour": 13, "summerHour": 14,
    }));
}

#[test]
fn default_timezone_keeps_date_and_intl_coherent_across_dst() {
    berlin(timezone(None, None));
}

#[test]
fn inherited_timezone_keeps_date_and_intl_coherent_across_dst() {
    assert_eq!(timezone(Some("America/New_York"), None), serde_json::json!({
        "zone": "America/New_York", "winter": 300, "summer": 240,
        "winterHour": 7, "summerHour": 8,
    }));
}

#[test]
fn explicit_timezone_overrides_the_inherited_zone() {
    berlin(timezone(Some("America/New_York"), Some("Europe/Berlin")));
}
