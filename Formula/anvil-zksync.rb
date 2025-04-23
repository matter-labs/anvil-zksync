class AnvilZksync < Formula
    desc "An in-memory ZKSync node for fast Elastic Network ZK chain development"
    homepage "https://github.com/matter-labs/anvil-zksync"
    version "0.5.0"
  
    on_macos do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-apple-darwin.tar.gz"
        sha256 "a4ad1652175be943aeac78a9c3d82eb3e4d27ddf7ed4544dbc8069d38351fbf5"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-apple-darwin.tar.gz"
        sha256 "9f705173914ce6f8426fb344463a7aaa1a5dae8836618388c3a79e098cd7bb60"
      end
    end
  
    on_linux do
      if Hardware::CPU.arm?
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-aarch64-unknown-linux-gnu.tar.gz"
        sha256 "c6a58484d403696d2cc1d250e0ca126eccc13ea4910105822a5bbaf4c2ab229a"
      else
        url "https://github.com/matter-labs/anvil-zksync/releases/download/v#{version}/anvil-zksync-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
        sha256 "064327e8dfa58f47d6bb5723a9b193e8eeda37319c7fc8a4041ea2564766a3d1"
      end
    end
  
    def install
      bin.install "anvil-zksync"
    end
  
    test do
      assert_match version, shell_output("#{bin}/anvil-zksync --version")
    end
  
    livecheck do
      url :stable
      regex(/^v?(\d+(?:\.\d+)+)$/i)
    end
  end
  