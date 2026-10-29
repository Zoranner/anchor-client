pub struct PendingPolicy {
    pub id: String,
    pub package_json: String,
}

#[derive(Default)]
pub struct NativeState {
    pub pending: std::sync::Mutex<Option<PendingPolicy>>,
}
