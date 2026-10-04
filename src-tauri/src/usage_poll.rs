//! D3 opt-in live usage poller (lane L4). FOUNDATION STUB.

use crate::hub::Core;
use std::sync::Arc;

/// Start the background poller. It must do nothing (and read no credentials)
/// while `config.usage.live_when_idle` is false.
pub fn spawn(_core: Arc<Core>) {}
