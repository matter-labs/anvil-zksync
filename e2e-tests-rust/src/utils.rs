// FIXME: Copy-pasted from spec-tests, we need to restructure the crates
use anyhow::Context;
use fs2::FileExt;
use std::{
    fs::File,
    net::{Ipv4Addr, SocketAddrV4},
};
use tokio::net::TcpListener;

pub struct LockedPort {
    pub port: u16,
    lockfile: File,
}

impl LockedPort {
    /// Checks if the requested port is free.
    /// Returns the unused port (same value as input, except for `0`).
    async fn check_port_is_unused(port: u16) -> anyhow::Result<u16> {
        let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port);
        let listener = TcpListener::bind(addr)
            .await
            .context("failed to bind to random port")?;
        let port = listener
            .local_addr()
            .context("failed to get local address for random port")?
            .port();
        Ok(port)
    }

    /// Request an unused port from the OS.
    async fn pick_unused_port() -> anyhow::Result<u16> {
        // Port 0 means the OS gives us an unused port
        Self::check_port_is_unused(0).await
    }

    /// Acquire an unused port and lock it (meaning no other competing callers of this method can
    /// take this lock). Lock lasts until the returned `LockedPort` instance is dropped.
    pub async fn acquire_unused() -> anyhow::Result<Self> {
        loop {
            let port = Self::pick_unused_port().await?;
            let lockpath = std::env::temp_dir().join(format!("anvil-zksync-port{}.lock", port));
            let lockfile = File::create(lockpath)
                .with_context(|| format!("failed to create lockfile for port={}", port))?;
            if lockfile.try_lock_exclusive().is_ok() {
                break Ok(Self { port, lockfile });
            }
        }
    }

    /// Acquire the requested port and lock it (meaning no other competing callers of this method
    /// can take this lock). Lock lasts until the returned `LockedPort` instance is dropped.
    pub async fn acquire(port: u16) -> anyhow::Result<Self> {
        let port = Self::check_port_is_unused(port).await?;
        let lockpath = std::env::temp_dir().join(format!("anvil-zksync-port{}.lock", port));
        let lockfile = File::create(lockpath)
            .with_context(|| format!("failed to create lockfile for port={}", port))?;
        lockfile
            .try_lock_exclusive()
            .with_context(|| format!("failed to lock the lockfile for port={}", port))?;
        Ok(Self { port, lockfile })
    }
}

/// Dropping `LockedPort` unlocks the port, caller needs to make sure the port is already bound to
/// or is not needed anymore.
impl Drop for LockedPort {
    fn drop(&mut self) {
        self.lockfile
            .unlock()
            .with_context(|| format!("failed to unlock lockfile for port={}", self.port))
            .unwrap();
    }
}
