use self_update::backends::github::Update;
use shared::endpoint::ServerEndpoint;
use tokio::task::spawn_blocking;
use tracing::{info, warn};

const REPO_OWNER: &str = "EliasDerHai";
const REPO_NAME: &str = "rust-file-sync";

/// On startup: ask the server which version it expects (`GET /version`) and, if
/// this client is behind and a release asset exists for the current platform,
/// download the matching GitHub release binary, replace this executable and
/// re-exec into the new version.
///
/// Every failure path is non-fatal
pub async fn check_and_self_update(server_url: &str) {
    let current = env!("CARGO_PKG_VERSION");

    let target = match fetch_server_version(server_url).await {
        Some(v) => v,
        None => return,
    };

    if !is_behind(current, &target) {
        return;
    }

    let asset_target = match asset_target() {
        Some(t) => t,
        None => {
            warn!(
                "client is behind (client={current}, server={target}) but no release asset \
                 exists for this platform - skipping self-update"
            );
            return;
        }
    };

    info!("client is behind (client={current}, server={target}) - attempting self-update");

    // `self_update` must not run on the async runtime thread.
    let target_for_task = target.clone();
    let result = spawn_blocking(move || run_update(asset_target, &target_for_task)).await;

    match result {
        Ok(Ok(true)) => {
            info!("self-update to {target} succeeded - re-executing new binary");
            reexec();
        }
        Ok(Ok(false)) => {
            // Nothing to do (server/GitHub disagreed about the newest version).
        }
        Ok(Err(e)) => warn!("self-update failed: {e} - continuing on current version"),
        Err(e) => warn!("self-update task panicked: {e} - continuing on current version"),
    }
}

/// Fetch the server's own version from `GET /version` (plain text). Returns
/// `None` (with a warning) on any transport/parse error.
async fn fetch_server_version(server_url: &str) -> Option<String> {
    let uri = ServerEndpoint::Version.to_uri(server_url);
    match reqwest::Client::new().get(&uri).send().await {
        Ok(resp) => match resp.text().await {
            Ok(body) => Some(body.trim().to_string()),
            Err(e) => {
                warn!("could not read /version response for update check: {e}");
                None
            }
        },
        Err(e) => {
            warn!("could not reach /version for update check: {e}");
            None
        }
    }
}

fn is_behind(current: &str, target: &str) -> bool {
    match (
        semver::Version::parse(current),
        semver::Version::parse(target),
    ) {
        (Ok(c), Ok(t)) => c < t,
        _ => {
            warn!(
                "could not parse versions for comparison (own={current}, server={target}) - \
                 skipping self-update"
            );
            false
        }
    }
}

/// Maps the running platform to the release asset naming scheme
/// (`client-<os>-<arch>`). `None` for platforms without a published asset
/// (Windows, linux-aarch64).
fn asset_target() -> Option<&'static str> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("linux-x86_64")
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("macos-arm64")
    } else {
        None
    }
}

/// Blocking: pin to the server-reported version tag and replace the running
/// binary. Returns whether the binary was actually replaced.
fn run_update(
    asset_target: &str,
    target_version: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let status = Update::configure()
        .repo_owner(REPO_OWNER)
        .repo_name(REPO_NAME)
        .bin_name("client")
        .target(asset_target)
        .current_version(env!("CARGO_PKG_VERSION"))
        .target_version_tag(&format!("v{target_version}"))
        .no_confirm(true)
        .build()?
        .update()?;
    Ok(status.updated())
}

/// Replace the current process image with the freshly installed binary. Keeps
/// the same PID so launchd/systemd supervision stays valid, avoiding any
/// dependence on the service manager's restart policy. Only returns if `exec`
/// fails, in which case we exit and let the service manager restart us.
#[cfg(unix)]
fn reexec() -> ! {
    use std::os::unix::process::CommandExt;

    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            warn!("could not resolve current_exe after self-update: {e} - exiting to restart");
            std::process::exit(0);
        }
    };
    let err = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .exec();
    warn!(
        "failed to re-exec after self-update: {err} - exiting so the service manager restarts us"
    );
    std::process::exit(0);
}

/// Non-unix fallback (unreachable in practice: no self-update asset ships for
/// these platforms). Exit and let the service manager restart into the new
/// binary.
#[cfg(not(unix))]
fn reexec() -> ! {
    std::process::exit(0);
}
