//! Click-to-focus terminal (lane L4, D7). FOUNDATION STUB.

use nudge_proto::ProcInfo;

/// Bring the terminal window hosting this session to the foreground.
/// `ancestors` = the hook process's ancestors, nearest parent first.
pub fn focus_session(_ancestors: &[ProcInfo]) -> Result<(), String> {
    Err("focus not implemented yet".into())
}
