use anyhow::Context;
use fs2::FileExt;
use std::fs::File;
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::net::TcpListener;

/// Request an unused port from the OS.
async fn pick_unused_port() -> anyhow::Result<u16> {
    // Port 0 means the OS gives us an unused port
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0);
    let listener = TcpListener::bind(addr)
        .await
        .context("failed to bind to random port")?;
    let port = listener
        .local_addr()
        .context("failed to get local address for random port")?
        .port();
    Ok(port)
}

/// Acquire an unused port and lock it for the duration until the era-test-node server has
/// been started.
pub async fn acquire_unused_port() -> anyhow::Result<(u16, File)> {
    loop {
        let port = pick_unused_port().await?;
        let lockpath = std::env::temp_dir().join(format!("era-test-node-port{}.lock", port));
        let lockfile = File::create(lockpath)
            .with_context(|| format!("failed to create lockfile for port {}", port))?;
        if lockfile.try_lock_exclusive().is_ok() {
            break Ok((port, lockfile));
        }
    }
}
