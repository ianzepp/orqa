use std::{ffi::OsString, path::PathBuf};

use super::*;

fn spec() -> ServiceSpec {
    ServiceSpec {
        label: "com.ianzepp.orqa.sample-pod.abc123".to_string(),
        unit: "orqa-sample-pod-abc123.service".to_string(),
        exe: PathBuf::from("/usr/local/bin/orqa"),
        home: PathBuf::from("/tmp/orqa home"),
        pod: "sample-pod".to_string(),
    }
}

fn install_args() -> ServiceInstallArgs {
    ServiceInstallArgs {
        pod: Some("sample-pod".to_string()),
        interval: 30,
        force: true,
        framework: Some(OsString::from("/bin/echo")),
        args: vec![OsString::from("handle work")],
    }
}

#[test]
fn macos_plist_runs_service_loop() {
    let plist = macos_plist(&spec(), &install_args());

    assert!(plist.contains("<string>com.ianzepp.orqa.sample-pod.abc123</string>"));
    assert!(plist.contains("<string>service</string>"));
    assert!(plist.contains("<string>run</string>"));
    assert!(plist.contains("<string>--interval</string>"));
    assert!(plist.contains("<string>30</string>"));
    assert!(plist.contains("<string>--framework</string>"));
    assert!(plist.contains("<string>handle work</string>"));
}

#[test]
fn linux_unit_runs_service_loop() {
    let unit = linux_unit(&spec(), &install_args());

    assert!(unit.contains("Description=Orqa wake-loop service for pod sample-pod"));
    assert!(unit.contains("ExecStart=/bin/sh -lc"));
    assert!(unit.contains("service"));
    assert!(unit.contains("run"));
    assert!(unit.contains("sample-pod"));
    assert!(unit.contains("--interval"));
    assert!(unit.contains("30"));
    assert!(unit.contains("Restart=always"));
}

#[test]
fn rejects_zero_interval() {
    assert!(validate_interval(0).is_err());
    assert!(validate_interval(1).is_ok());
}
