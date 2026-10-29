use anchor_core::{
    crypto, ActionPolicy, BurningRules, EventType, NetworkRules, PolicyPackage, PolicyPayload,
    ScanRules, UsbRules,
};
use anchor_ipc::{AuditFilter, ImportResponse};
use anchor_service::Service;
use anchor_store::Store;
use chrono::{Duration, TimeZone, Utc};

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 24, 8, 0, 0).unwrap()
}

fn payload(version: u64) -> PolicyPayload {
    PolicyPayload {
        version,
        expires_at: now() + Duration::days(30),
        usb: UsbRules {
            read: ActionPolicy::Deny,
            write: ActionPolicy::Deny,
            allowed: vec![],
        },
        network: NetworkRules {
            policy: ActionPolicy::Deny,
            allowed: vec![],
        },
        burning: BurningRules {
            policy: ActionPolicy::Deny,
            allowed: vec![],
        },
        scan: ScanRules {
            controlled_dirs: vec![],
            file_types: vec![],
            rules: vec![],
        },
    }
}

fn service_with_policy(version: u64) -> (Service, ed25519_dalek::SigningKey) {
    let (signing_key, verifying_key) = crypto::generate_keypair();
    let service = Service::new(Store::in_memory().unwrap(), verifying_key, "laptop-1");
    service
        .import_policy_json(
            &serde_json::to_string(&PolicyPackage::sign(payload(version), &signing_key).unwrap())
                .unwrap(),
            "admin",
            now(),
        )
        .unwrap();
    (service, signing_key)
}

#[test]
fn inspect_does_not_change_active_policy_or_audit() {
    let (service, key) = service_with_policy(1);
    let before = service
        .query_audit(AuditFilter::default(), None, 100, now())
        .unwrap();
    let candidate = serde_json::to_string(&PolicyPackage::sign(payload(2), &key).unwrap()).unwrap();

    let inspection = service.inspect_policy(&candidate, now()).unwrap();

    assert!(inspection.accepted);
    assert_eq!(inspection.current.unwrap().payload.version, 1);
    assert_eq!(
        service
            .desktop_status(now())
            .unwrap()
            .policy
            .unwrap()
            .payload
            .version,
        1
    );
    let after = service
        .query_audit(AuditFilter::default(), None, 100, now())
        .unwrap();
    assert_eq!(before.events.len(), after.events.len());
}

#[test]
fn import_rejects_older_version_without_replacing_policy() {
    let (service, key) = service_with_policy(2);
    let candidate = serde_json::to_string(&PolicyPackage::sign(payload(1), &key).unwrap()).unwrap();

    let response: ImportResponse = service
        .import_policy_json(&candidate, "alice", now())
        .unwrap();

    assert!(!response.accepted);
    assert_eq!(response.reason, "version_not_newer");
    assert_eq!(
        service
            .desktop_status(now())
            .unwrap()
            .policy
            .unwrap()
            .payload
            .version,
        2
    );
}

#[test]
fn audit_query_is_paged_and_read_only() {
    let (service, _) = service_with_policy(1);
    service
        .record_lifecycle_event(EventType::ServiceStart, "service_start", "running", now())
        .unwrap();
    service
        .record_lifecycle_event(EventType::AuditVerify, "audit_chain", "valid", now())
        .unwrap();

    let first = service
        .query_audit(AuditFilter::default(), None, 2, now())
        .unwrap();
    let second = service
        .query_audit(AuditFilter::default(), first.next_after_sequence, 2, now())
        .unwrap();

    assert_eq!(first.events.len(), 2);
    assert!(first.chain_healthy);
    assert!(second.events.len() <= 2);
    assert!(second.events.iter().all(|event| first
        .events
        .last()
        .is_none_or(|last| event.sequence > last.sequence)));
}
